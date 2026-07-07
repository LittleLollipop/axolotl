// src/recovery.rs
// Crash Recovery —— 重启时自动重放 WAL 日志
//
// 算法：
// 1. 加载基础图文件（AXEB 或 AXOL 格式）
// 2. 扫描 WAL 目录，找到所有 wal_*.log 文件
// 3. 每个 WAL 文件：
//    - 有 Commit 条目 → 重放到图中
//    - 无 Commit 条目 → 丢弃（事务未提交）
// 4. 清理已处理的 WAL 文件
// 5. 返回恢复后的图

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};

use crate::persistence::PersistentGraph;

// ── WAL 条目（与 transaction.rs 一致）─────────────────

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum OpType {
    Begin = 0,
    AddVertex = 1,
    AddEdge = 2,
    DeleteVertex = 3,
    DeleteEdge = 4,
    Commit = 5,
    Rollback = 6,
}

impl OpType {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(OpType::Begin),
            1 => Some(OpType::AddVertex),
            2 => Some(OpType::AddEdge),
            3 => Some(OpType::DeleteVertex),
            4 => Some(OpType::DeleteEdge),
            5 => Some(OpType::Commit),
            6 => Some(OpType::Rollback),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct WalEntry {
    tx_id: u64,
    op_type: OpType,
    _timestamp: u64,
    data: Vec<u8>,
}

fn parse_wal_entry(buf: &[u8]) -> Option<(WalEntry, usize)> {
    if buf.len() < 21 { return None; }

    let tx_id = u64::from_be_bytes(buf[0..8].try_into().ok()?);
    let op_type = OpType::from_u8(buf[8])?;
    let timestamp = u64::from_be_bytes(buf[9..17].try_into().ok()?);
    let data_len = u32::from_be_bytes(buf[17..21].try_into().ok()?) as usize;

    let total = 21 + data_len;
    if buf.len() < total { return None; }

    let data = buf[21..total].to_vec();
    Some((WalEntry { tx_id, op_type, _timestamp: timestamp, data }, total))
}

// ── 恢复逻辑 ─────────────────────────

/// WAL 恢复结果
#[derive(Debug)]
pub struct RecoveryResult {
    /// 处理的 WAL 文件数
    pub wal_files_found: usize,
    /// 成功重放的事务数
    pub transactions_replayed: usize,
    /// 跳过的事务数（未提交）
    pub transactions_skipped: usize,
    /// 添加的顶点数
    pub vertices_added: usize,
    /// 添加的边数
    pub edges_added: usize,
}

/// 扫描 WAL 目录并恢复
///
/// `graph` 应该已经从磁盘加载（PersistentGraph 格式）。
/// 返回恢复后的图（原地修改了 `graph`）和恢复统计。
pub fn recover_wal(
    graph: &mut PersistentGraph,
    wal_dir: &str,
) -> io::Result<RecoveryResult> {
    let mut result = RecoveryResult {
        wal_files_found: 0,
        transactions_replayed: 0,
        transactions_skipped: 0,
        vertices_added: 0,
        edges_added: 0,
    };

    // 收集 WAL 文件
    let mut wal_files: Vec<_> = match fs::read_dir(wal_dir) {
        Ok(dir) => dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("wal_") && n.ends_with(".log"))
                    .unwrap_or(false)
            })
            .collect(),
        Err(_) => {
            // WAL 目录不存在 = 没有待恢复的日志
            return Ok(result);
        }
    };

    wal_files.sort();
    result.wal_files_found = wal_files.len();

    for path in &wal_files {
        let mut f = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;

        // 解析所有条目
        let mut entries = Vec::new();
        let mut pos = 0;
        while pos < buf.len() {
            if let Some((entry, len)) = parse_wal_entry(&buf[pos..]) {
                entries.push(entry);
                pos += len;
            } else {
                break; // 截断的 WAL（crash 中）
            }
        }

        if entries.is_empty() { continue; }

        // 判断是否已提交
        let has_commit = entries.iter().any(|e| e.op_type == OpType::Commit);
        let has_rollback = entries.iter().any(|e| e.op_type == OpType::Rollback);

        if has_commit && !has_rollback {
            // 已提交 → 重放
            for entry in &entries {
                replay_entry(graph, entry);
            }
            result.transactions_replayed += 1;

            // 统计数据
            let v_add = entries.iter().filter(|e| e.op_type == OpType::AddVertex).count();
            let e_add = entries.iter().filter(|e| e.op_type == OpType::AddEdge).count();
            result.vertices_added += v_add;
            result.edges_added += e_add;
        } else {
            // 未提交或已回滚 → 跳过
            result.transactions_skipped += 1;
        }

        // 清理 WAL 文件
        let _ = fs::remove_file(path);
    }

    Ok(result)
}

/// 重放单条 WAL 条目到 PersistentGraph
fn replay_entry(graph: &mut PersistentGraph, entry: &WalEntry) {
    match entry.op_type {
        OpType::AddVertex => {
            if entry.data.len() >= 8 {
                let id = u64::from_be_bytes(entry.data[0..8].try_into().unwrap());
                let props = if entry.data.len() > 12 {
                    let json_len = u32::from_be_bytes(entry.data[8..12].try_into().unwrap()) as usize;
                    if entry.data.len() >= 12 + json_len {
                        serde_json::from_slice(&entry.data[12..12 + json_len]).unwrap_or_default()
                    } else { HashMap::new() }
                } else { HashMap::new() };
                graph.add_vertex(id, props);
            }
        }
        OpType::AddEdge => {
            if entry.data.len() >= 24 {
                let from = u64::from_be_bytes(entry.data[0..8].try_into().unwrap());
                let to = u64::from_be_bytes(entry.data[8..16].try_into().unwrap());
                let weight = f64::from_be_bytes(entry.data[16..24].try_into().unwrap());
                let props = if entry.data.len() > 28 {
                    let json_len = u32::from_be_bytes(entry.data[24..28].try_into().unwrap()) as usize;
                    if entry.data.len() >= 28 + json_len {
                        serde_json::from_slice(&entry.data[28..28 + json_len]).unwrap_or_default()
                    } else { HashMap::new() }
                } else { HashMap::new() };
                graph.add_edge(from, to, weight, props);
            }
        }
        OpType::DeleteVertex => {
            if entry.data.len() >= 8 {
                let id = u64::from_be_bytes(entry.data[0..8].try_into().unwrap());
                graph.vertices.remove(&id);
            }
        }
        OpType::DeleteEdge => {
            if entry.data.len() >= 16 {
                let from = u64::from_be_bytes(entry.data[0..8].try_into().unwrap());
                let to = u64::from_be_bytes(entry.data[8..16].try_into().unwrap());
                graph.edges.remove(&(from, to));
            }
        }
        _ => {} // Begin/Commit/Rollback 不需要重放
    }
}

// ── 集成：恢复 + 转 EdgeBlock ─────────────────────────

/// 从文件加载图并应用 WAL 恢复，返回 EdgeBlock 图
///
/// 使用场景：GraphDB::open() 时自动调用
pub fn recover_to_edgeblock(
    graph_path: &str,
    wal_dir: &str,
) -> io::Result<(crate::gpu_edge_block::GPUEdgeBlockGraph, RecoveryResult)> {
    use crate::gpu_edge_block::GPUEdgeBlockGraph;

    // 尝试加载基础图（优先 AXEB，回退 AXOL）
    let mut pg = if let Ok(eb) = GPUEdgeBlockGraph::open(graph_path) {
        // 已有 EdgeBlock 文件，转回 PersistentGraph 以便 WAL 重放
        edgeblock_to_persistent(&eb)
    } else if std::path::Path::new(graph_path).exists() {
        PersistentGraph::open(graph_path)?
    } else {
        // 新数据库（无基础文件）
        PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: graph_path.to_string(),
            index_manager: crate::index::IndexManager::new(),
        }
    };

    // 应用 WAL 恢复
    let result = recover_wal(&mut pg, wal_dir)?;

    // 转回 EdgeBlock
    let eb = persistent_to_edgeblock(&pg);

    Ok((eb, result))
}

fn edgeblock_to_persistent(eb: &crate::gpu_edge_block::GPUEdgeBlockGraph) -> PersistentGraph {
    let mut pg = PersistentGraph {
        vertices: HashMap::new(),
        edges: HashMap::new(),
        file_path: String::new(),
        index_manager: crate::index::IndexManager::new(),
    };
    for i in 0..eb.vertex_count as usize {
        let id = eb.idx_to_id[i];
        if id != u64::MAX {
            pg.add_vertex(id, eb.vertex_props[i].clone());
        }
    }
    for i in 0..eb.vertex_count as usize {
        let from_id = eb.idx_to_id[i];
        if from_id == u64::MAX { continue; }
        for to_id in eb.out_neighbors_by_idx(i) {
            pg.add_edge(from_id, to_id, 1.0, HashMap::new());
        }
    }
    pg
}

fn persistent_to_edgeblock(pg: &PersistentGraph) -> crate::gpu_edge_block::GPUEdgeBlockGraph {
    let mut eb = crate::gpu_edge_block::GPUEdgeBlockGraph::new();
    for (&id, vr) in &pg.vertices {
        eb.add_vertex(id, vr.properties.clone());
    }
    for ((from, to), er) in &pg.edges {
        eb.add_edge(*from, *to, er.weight as f32);
    }
    eb
}

// ── 测试 ───────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PropertyValue;
    use std::collections::HashMap;
    use std::fs;

    #[test]
    fn test_recover_committed_transaction() {
        let wal_dir = "/tmp/test_recovery_wal";
        let _ = fs::remove_dir_all(wal_dir);
        fs::create_dir_all(wal_dir).unwrap();

        // 创建 WAL 文件模拟一个已提交事务
        use std::io::Write;
        let tx_id = 100u64;
        let ts = 1u64;

        let mut wal_file = fs::File::create(&format!("{}/wal_{}.log", wal_dir, tx_id)).unwrap();

        // Begin
        wal_file.write_all(&tx_id.to_be_bytes()).unwrap();
        wal_file.write_all(&[0u8]).unwrap(); // OpType::Begin
        wal_file.write_all(&ts.to_be_bytes()).unwrap();
        wal_file.write_all(&0u32.to_be_bytes()).unwrap(); // data_len = 0

        // AddVertex: id=1, props={name: "Alice"}
        let mut data = Vec::new();
        data.extend_from_slice(&1u64.to_be_bytes());
        let props_json = br#"{"name":"Alice"}"#;
        data.extend_from_slice(&(props_json.len() as u32).to_be_bytes());
        data.extend_from_slice(props_json);
        wal_file.write_all(&tx_id.to_be_bytes()).unwrap();
        wal_file.write_all(&[1u8]).unwrap(); // OpType::AddVertex
        wal_file.write_all(&ts.to_be_bytes()).unwrap();
        wal_file.write_all(&(data.len() as u32).to_be_bytes()).unwrap();
        wal_file.write_all(&data).unwrap();

        // Commit
        wal_file.write_all(&tx_id.to_be_bytes()).unwrap();
        wal_file.write_all(&[5u8]).unwrap(); // OpType::Commit
        wal_file.write_all(&ts.to_be_bytes()).unwrap();
        wal_file.write_all(&0u32.to_be_bytes()).unwrap(); // data_len = 0

        drop(wal_file);

        // 恢复
        let mut graph = PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: String::new(),
            index_manager: crate::index::IndexManager::new(),
        };
        let result = recover_wal(&mut graph, wal_dir).unwrap();

        assert_eq!(result.wal_files_found, 1);
        assert_eq!(result.transactions_replayed, 1);
        assert_eq!(result.transactions_skipped, 0);

        assert!(graph.vertices.contains_key(&1));
        assert_eq!(graph.vertices[&1].properties["name"], PropertyValue::String("Alice".to_string()));

        // WAL 文件应该被清理
        assert!(!std::path::Path::new(&format!("{}/wal_{}.log", wal_dir, tx_id)).exists());
        let _ = fs::remove_dir_all(wal_dir);
    }

    #[test]
    fn test_skip_uncommitted_transaction() {
        let wal_dir = "/tmp/test_recovery_uncommitted";
        let _ = fs::remove_dir_all(wal_dir);
        fs::create_dir_all(wal_dir).unwrap();

        use std::io::Write;
        let tx_id = 200u64;
        let ts = 1u64;

        let mut wal_file = fs::File::create(&format!("{}/wal_{}.log", wal_dir, tx_id)).unwrap();

        // Begin
        wal_file.write_all(&tx_id.to_be_bytes()).unwrap();
        wal_file.write_all(&[0u8]).unwrap();
        wal_file.write_all(&ts.to_be_bytes()).unwrap();
        wal_file.write_all(&0u32.to_be_bytes()).unwrap();

        // AddVertex (but NO Commit → crash!)
        let mut data = Vec::new();
        data.extend_from_slice(&1u64.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes()); // empty props
        wal_file.write_all(&tx_id.to_be_bytes()).unwrap();
        wal_file.write_all(&[1u8]).unwrap();
        wal_file.write_all(&ts.to_be_bytes()).unwrap();
        wal_file.write_all(&(data.len() as u32).to_be_bytes()).unwrap();
        wal_file.write_all(&data).unwrap();

        // NO Commit → uncommitted transaction
        drop(wal_file);

        let mut graph = PersistentGraph {
            vertices: HashMap::new(), edges: HashMap::new(),
            file_path: String::new(),
            index_manager: crate::index::IndexManager::new(),
        };
        let result = recover_wal(&mut graph, wal_dir).unwrap();

        assert_eq!(result.transactions_replayed, 0);
        assert_eq!(result.transactions_skipped, 1);
        assert!(!graph.vertices.contains_key(&1)); // 未提交，不应出现

        let _ = fs::remove_dir_all(wal_dir);
    }

    #[test]
    fn test_recover_to_edgeblock() {
        let wal_dir = "/tmp/test_recovery_eb_wal";
        let data_path = "/tmp/test_recovery_data.bin";
        let _ = fs::remove_dir_all(wal_dir);
        let _ = fs::remove_file(data_path);
        fs::create_dir_all(wal_dir).unwrap();

        // 创建基础 EdgeBlock 文件
        let mut eb = crate::gpu_edge_block::GPUEdgeBlockGraph::new();
        eb.add_vertex(1, HashMap::new());
        eb.add_vertex(2, HashMap::new());
        eb.add_edge(1, 2, 1.0);
        eb.save(data_path).unwrap();

        // 创建 WAL（已提交事务：添加顶点 3）
        use std::io::Write;
        let tx_id = 300u64;
        let ts = 1u64;
        let mut wal_file = fs::File::create(&format!("{}/wal_{}.log", wal_dir, tx_id)).unwrap();
        wal_file.write_all(&tx_id.to_be_bytes()).unwrap();
        wal_file.write_all(&[0u8]).unwrap(); // Begin
        wal_file.write_all(&ts.to_be_bytes()).unwrap();
        wal_file.write_all(&0u32.to_be_bytes()).unwrap();

        let mut data = Vec::new();
        data.extend_from_slice(&3u64.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        wal_file.write_all(&tx_id.to_be_bytes()).unwrap();
        wal_file.write_all(&[1u8]).unwrap(); // AddVertex
        wal_file.write_all(&ts.to_be_bytes()).unwrap();
        wal_file.write_all(&(data.len() as u32).to_be_bytes()).unwrap();
        wal_file.write_all(&data).unwrap();

        wal_file.write_all(&tx_id.to_be_bytes()).unwrap();
        wal_file.write_all(&[5u8]).unwrap(); // Commit
        wal_file.write_all(&ts.to_be_bytes()).unwrap();
        wal_file.write_all(&0u32.to_be_bytes()).unwrap();
        drop(wal_file);

        // 恢复
        let (eb, result) = recover_to_edgeblock(data_path, wal_dir).unwrap();
        assert_eq!(result.transactions_replayed, 1);
        assert_eq!(eb.vertex_count, 3);
        assert_eq!(eb.total_edges, 1);
        assert!(eb.get_vertex(3).is_some());

        let _ = fs::remove_dir_all(wal_dir);
        let _ = fs::remove_file(data_path);
    }
}
