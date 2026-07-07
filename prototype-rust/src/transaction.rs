// src/transaction.rs
// 事务支持模块
//
// 实现：
// - WAL（Write-Ahead Log）
// - 回滚
// - 快照隔离（简化版）

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::PropertyValue;
use crate::persistence::{PersistentGraph, VertexRecord, EdgeRecord};

// ── WAL 日志条目 ─────────────────────────

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum OpType {
    Begin = 0,
    AddVertex = 1,
    AddEdge = 2,
    DeleteVertex = 3,
    DeleteEdge = 4,
    Commit = 5,
    Rollback = 6,
}

/// WAL 日志条目
#[derive(Debug, Clone)]
struct WalEntry {
    tx_id: u64,
    op_type: u8,
    timestamp: u64,
    data: Vec<u8>,
}

impl WalEntry {
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // tx_id (8 bytes)
        buf.extend_from_slice(&self.tx_id.to_be_bytes());

        // op_type (1 byte)
        buf.push(self.op_type);

        // timestamp (8 bytes)
        buf.extend_from_slice(&self.timestamp.to_be_bytes());

        // data_len (4 bytes)
        let data_len = self.data.len() as u32;
        buf.extend_from_slice(&data_len.to_be_bytes());

        // data
        buf.extend_from_slice(&self.data);

        buf
    }

    fn deserialize(buf: &[u8]) -> Option<Self> {
        if buf.len() < 21 {
            return None;
        }

        let tx_id = u64::from_be_bytes(buf[0..8].try_into().ok()?);
        let op_type = buf[8];
        let timestamp = u64::from_be_bytes(buf[9..17].try_into().ok()?);
        let data_len = u32::from_be_bytes(buf[17..21].try_into().ok()?);

        if buf.len() < 21 + data_len as usize {
            return None;
        }

        let data = buf[21..21 + data_len as usize].to_vec();

        Some(WalEntry {
            tx_id,
            op_type,
            timestamp,
            data,
        })
    }
}

// ── 事务 ID 生成 ─────────────────────────

fn generate_tx_id() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

// ── 事务 ─────────────────────────

/// 事务状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TxState {
    Active,
    Committed,
    RolledBack,
}

/// 写操作记录（用于回滚）
#[derive(Debug, Clone)]
struct WriteOp {
    op_type: u8,
    vertex_id: Option<u64>,
    edge_key: Option<(u64, u64)>,
    old_vertex: Option<VertexRecord>,  // 用于回滚
    old_edge: Option<EdgeRecord>,      // 用于回滚
}

/// 事务
///
/// 用法：
/// ```rust
/// let mut tx = graph.begin_tx();
/// tx.add_vertex(1, props).unwrap();
/// tx.add_edge(1, 2, 1.0, edge_props).unwrap();
/// tx.commit().unwrap();
/// ```
pub struct Transaction {
    graph: Arc<RwLock<PersistentGraph>>,
    tx_id: u64,
    state: TxState,
    /// 写操作记录（用于回滚）
    write_ops: Vec<WriteOp>,
    /// WAL 文件句柄
    wal_file: Option<File>,
    wal_path: String,
}

impl Transaction {
    /// 开始新事务
    pub fn begin(graph: Arc<RwLock<PersistentGraph>>, wal_dir: &str) -> io::Result<Self> {
        let tx_id = generate_tx_id();
        let wal_path = format!("{}/wal_{}.log", wal_dir, tx_id);

        let mut wal_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wal_path)?;

        // 写入 Begin 日志
        let entry = WalEntry {
            tx_id,
            op_type: OpType::Begin as u8,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            data: Vec::new(),
        };
        wal_file.write_all(&entry.serialize())?;

        Ok(Transaction {
            graph,
            tx_id,
            state: TxState::Active,
            write_ops: Vec::new(),
            wal_file: Some(wal_file),
            wal_path,
        })
    }

    /// 写入 WAL（内部方法，不需要锁）
    fn log_op(&mut self, op_type: OpType, data: &[u8]) -> io::Result<()> {
        if let Some(ref mut f) = self.wal_file {
            let entry = WalEntry {
                tx_id: self.tx_id,
                op_type: op_type as u8,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                data: data.to_vec(),
            };
            f.write_all(&entry.serialize())?;
        }
        Ok(())
    }

    /// 序列化 add_vertex 操作
    fn serialize_add_vertex(id: u64, properties: &HashMap<String, PropertyValue>) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&id.to_be_bytes());
        // 简化：只序列化 id，properties 用 JSON
        let props_json = serde_json::to_vec(properties).unwrap_or_default();
        buf.extend_from_slice(&(props_json.len() as u32).to_be_bytes());
        buf.extend_from_slice(&props_json);
        buf
    }

    /// 序列化 add_edge 操作
    fn serialize_add_edge(from: u64, to: u64, weight: f64, properties: &HashMap<String, PropertyValue>) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&from.to_be_bytes());
        buf.extend_from_slice(&to.to_be_bytes());
        buf.extend_from_slice(&weight.to_be_bytes());
        let props_json = serde_json::to_vec(properties).unwrap_or_default();
        buf.extend_from_slice(&(props_json.len() as u32).to_be_bytes());
        buf.extend_from_slice(&props_json);
        buf
    }

    /// 序列化边 key
    fn serialize_edge_key(from: u64, to: u64) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&from.to_be_bytes());
        buf.extend_from_slice(&to.to_be_bytes());
        buf
    }

    /// 添加顶点
    pub fn add_vertex(&mut self, id: u64, properties: HashMap<String, PropertyValue>) -> io::Result<()> {
        if self.state != TxState::Active {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Transaction is not active",
            ));
        }

        // 先序列化数据（不需要锁）
        let data = Self::serialize_add_vertex(id, &properties);

        // 写 WAL（不需要锁）
        self.log_op(OpType::AddVertex, &data)?;

        // 获取锁，修改内存
        let mut graph = self.graph.write().unwrap();

        // 记录旧值（用于回滚）
        let old_vertex = graph.vertices.get(&id).cloned();
        self.write_ops.push(WriteOp {
            op_type: OpType::AddVertex as u8,
            vertex_id: Some(id),
            edge_key: None,
            old_vertex,
            old_edge: None,
        });

        // 修改内存
        graph.add_vertex(id, properties);

        Ok(())
    }

    /// 添加边
    pub fn add_edge(
        &mut self,
        from: u64,
        to: u64,
        weight: f64,
        properties: HashMap<String, PropertyValue>,
    ) -> io::Result<()> {
        if self.state != TxState::Active {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Transaction is not active",
            ));
        }

        // 先序列化数据（不需要锁）
        let data = Self::serialize_add_edge(from, to, weight, &properties);

        // 写 WAL（不需要锁）
        self.log_op(OpType::AddEdge, &data)?;

        // 获取锁，修改内存
        let mut graph = self.graph.write().unwrap();

        // 记录旧值（用于回滚）
        let old_edge = graph.edges.get(&(from, to)).cloned();
        self.write_ops.push(WriteOp {
            op_type: OpType::AddEdge as u8,
            vertex_id: None,
            edge_key: Some((from, to)),
            old_vertex: None,
            old_edge,
        });

        // 修改内存
        graph.add_edge(from, to, weight, properties);

        Ok(())
    }

    /// 删除顶点
    pub fn delete_vertex(&mut self, id: u64) -> io::Result<()> {
        if self.state != TxState::Active {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Transaction is not active",
            ));
        }

        // 写 WAL（不需要锁）
        let data = id.to_be_bytes();
        self.log_op(OpType::DeleteVertex, &data)?;

        // 获取锁，修改内存
        let mut graph = self.graph.write().unwrap();

        // 记录旧值（用于回滚）
        let old_vertex = graph.vertices.get(&id).cloned();
        self.write_ops.push(WriteOp {
            op_type: OpType::DeleteVertex as u8,
            vertex_id: Some(id),
            edge_key: None,
            old_vertex,
            old_edge: None,
        });

        // 修改内存
        graph.vertices.remove(&id);

        Ok(())
    }

    /// 删除边
    pub fn delete_edge(&mut self, from: u64, to: u64) -> io::Result<()> {
        if self.state != TxState::Active {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Transaction is not active",
            ));
        }

        // 写 WAL（不需要锁）
        let data = Self::serialize_edge_key(from, to);
        self.log_op(OpType::DeleteEdge, &data)?;

        // 获取锁，修改内存
        let mut graph = self.graph.write().unwrap();

        // 记录旧值（用于回滚）
        let old_edge = graph.edges.get(&(from, to)).cloned();
        self.write_ops.push(WriteOp {
            op_type: OpType::DeleteEdge as u8,
            vertex_id: None,
            edge_key: Some((from, to)),
            old_vertex: None,
            old_edge,
        });

        // 修改内存
        graph.edges.remove(&(from, to));

        Ok(())
    }

    /// 提交事务
    pub fn commit(&mut self) -> io::Result<()> {
        if self.state != TxState::Active {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Transaction is not active",
            ));
        }

        // 写入 Commit 日志
        self.log_op(OpType::Commit, &[])?;

        // 刷新 WAL
        if let Some(ref mut f) = self.wal_file {
            f.sync_all()?;
        }

        // 更新状态
        self.state = TxState::Committed;

        // 清理 WAL 文件（不需要锁）
        self.cleanup_wal()?;

        Ok(())
    }

    /// 回滚事务
    pub fn rollback(&mut self) -> io::Result<()> {
        if self.state != TxState::Active {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Transaction is not active",
            ));
        }

        // 写入 Rollback 日志
        self.log_op(OpType::Rollback, &[])?;

        // 回滚写操作（逆序）
        {
            let mut graph = self.graph.write().unwrap();
            for op in self.write_ops.iter().rev() {
                match op.op_type {
                    t if t == OpType::AddVertex as u8 => {
                        // 回滚：删除添加的顶点
                        if let Some(id) = op.vertex_id {
                            graph.vertices.remove(&id);
                        }
                    }
                    t if t == OpType::AddEdge as u8 => {
                        // 回滚：删除添加的边
                        if let Some((from, to)) = op.edge_key {
                            graph.edges.remove(&(from, to));
                        }
                    }
                    t if t == OpType::DeleteVertex as u8 => {
                        // 回滚：恢复删除的顶点
                        if let Some(id) = op.vertex_id {
                            if let Some(ref vr) = op.old_vertex {
                                graph.vertices.insert(id, vr.clone());
                            }
                        }
                    }
                    t if t == OpType::DeleteEdge as u8 => {
                        // 回滚：恢复删除的边
                        if let Some((from, to)) = op.edge_key {
                            if let Some(ref er) = op.old_edge {
                                graph.edges.insert((from, to), er.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        } // graph 锁在这里释放

        // 更新状态
        self.state = TxState::RolledBack;

        // 清理 WAL 文件（不需要锁）
        self.cleanup_wal()?;

        Ok(())
    }

    /// 清理 WAL 文件
    fn cleanup_wal(&mut self) -> io::Result<()> {
        self.wal_file = None;
        fs::remove_file(&self.wal_path).ok();
        Ok(())
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // 如果事务还在 Active 状态，自动回滚
        if self.state == TxState::Active {
            let _ = self.rollback();
        }
    }
}

// ── 事务管理器 ─────────────────────────

/// 事务管理器
///
/// 管理事务的创建、提交、回滚
pub struct TransactionManager {
    graph: Arc<RwLock<PersistentGraph>>,
    wal_dir: String,
}

impl TransactionManager {
    pub fn new(graph: PersistentGraph, wal_dir: &str) -> Self {
        // 创建 WAL 目录
        fs::create_dir_all(wal_dir).ok();

        TransactionManager {
            graph: Arc::new(RwLock::new(graph)),
            wal_dir: wal_dir.to_string(),
        }
    }

    /// 开始新事务
    pub fn begin_tx(&self) -> io::Result<Transaction> {
        Transaction::begin(self.graph.clone(), &self.wal_dir)
    }

    /// 获取图的只读快照
    pub fn snapshot(&self) -> PersistentGraph {
        let graph = self.graph.read().unwrap();
        PersistentGraph {
            vertices: graph.vertices.clone(),
            edges: graph.edges.clone(),
            file_path: graph.file_path.clone(),
            index_manager: crate::index::IndexManager::new(),  // 快照不继承索引（可以重建）
        }
    }

    /// 获取底层图的引用（用于算法）
    pub fn graph(&self) -> &Arc<RwLock<PersistentGraph>> {
        &self.graph
    }
}

// ── 测试 ───────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_graph() -> PersistentGraph {
        let test_path = "/tmp/test_tx_graph.bin";

        // 删除旧文件
        let _ = fs::remove_file(test_path);

        PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: test_path.to_string(),
            index_manager: crate::index::IndexManager::new(),
        }
    }

    #[test]
    fn test_transaction_commit() {
        let graph = setup_test_graph();
        let tx_mgr = TransactionManager::new(graph, "/tmp/test_wal");

        // 开始事务
        let mut tx = tx_mgr.begin_tx().unwrap();

        // 添加顶点
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        tx.add_vertex(1, props).unwrap();

        // 添加边
        let mut edge_props = HashMap::new();
        edge_props.insert("type".to_string(), PropertyValue::String("knows".to_string()));
        tx.add_edge(1, 2, 1.0, edge_props).unwrap();

        // 提交
        tx.commit().unwrap();

        // 验证
        let graph = tx_mgr.snapshot();
        assert_eq!(graph.vertices.len(), 2); // add_edge 会自动创建顶点 2
        assert_eq!(graph.edges.len(), 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let graph = setup_test_graph();
        let tx_mgr = TransactionManager::new(graph, "/tmp/test_wal");

        // 开始事务
        let mut tx = tx_mgr.begin_tx().unwrap();

        // 添加顶点
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        tx.add_vertex(1, props).unwrap();

        // 回滚
        tx.rollback().unwrap();

        // 验证：顶点不应该存在
        let graph = tx_mgr.snapshot();
        assert_eq!(graph.vertices.len(), 0);
    }

    #[test]
    fn test_transaction_auto_rollback() {
        let graph = setup_test_graph();
        let tx_mgr = TransactionManager::new(graph, "/tmp/test_wal");

        {
            // 开始事务
            let mut tx = tx_mgr.begin_tx().unwrap();

            // 添加顶点
            let mut props = HashMap::new();
            props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
            tx.add_vertex(1, props).unwrap();

            // tx 离开作用域，应该自动回滚
        }

        // 验证：顶点不应该存在
        let graph = tx_mgr.snapshot();
        assert_eq!(graph.vertices.len(), 0);
    }

    #[test]
    fn test_transaction_query_after_commit() {
        let graph = setup_test_graph();
        let tx_mgr = TransactionManager::new(graph, "/tmp/test_wal");

        // 开始事务
        let mut tx = tx_mgr.begin_tx().unwrap();

        // 添加顶点
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        tx.add_vertex(1, props).unwrap();

        // 提交
        tx.commit().unwrap();

        // 用查询 API 验证
        let graph = tx_mgr.snapshot();
        let results = graph.find_vertex()
            .with_property("name", PropertyValue::String("Alice".to_string()))
            .execute();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], 1);
    }
}
