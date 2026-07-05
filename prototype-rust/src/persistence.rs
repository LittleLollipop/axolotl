// src/persistence.rs
// 二进制持久化存储（与 Swift 版本格式对齐）

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use crate::PropertyValue;
use crate::CSRGraph;
// ── 二进制格式常量 ─────────────────────────

const MAGIC: [u8; 4] = *b"AXOL";
const VERSION: u16 = 1;

#[repr(u8)]
enum ValueType {
    String = 0,
    Int = 1,
    Double = 2,
    Bool = 3,
    Null = 4,
}

// ── 写入辅助 ─────────────────────────

fn write_u16<W: Write>(w: &mut W, v: u16) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

fn write_u32<W: Write>(w: &mut W, v: u32) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

fn write_u64<W: Write>(w: &mut W, v: u64) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

fn write_f64<W: Write>(w: &mut W, v: f64) -> io::Result<()> {
    w.write_all(&v.to_be_bytes())
}

fn write_string<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    write_u32(w, bytes.len() as u32)?;
    w.write_all(bytes)
}

fn write_property_value<W: Write>(w: &mut W, pv: &PropertyValue) -> io::Result<()> {
    match pv {
        PropertyValue::String(s) => {
            w.write_all(&[ValueType::String as u8])?;
            write_string(w, s)
        }
        PropertyValue::Int(i) => {
            w.write_all(&[ValueType::Int as u8])?;
            write_u64(w, *i as u64)
        }
        PropertyValue::Double(d) => {
            w.write_all(&[ValueType::Double as u8])?;
            write_f64(w, *d)
        }
        PropertyValue::Bool(b) => {
            w.write_all(&[ValueType::Bool as u8])?;
            w.write_all(&[*b as u8])
        }
        PropertyValue::Null => {
            w.write_all(&[ValueType::Null as u8])
        }
    }
}

// ── 读取辅助 ─────────────────────────

fn read_u16<R: Read>(r: &mut R) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

fn read_u64<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_be_bytes(buf))
}

fn read_f64<R: Read>(r: &mut R) -> io::Result<f64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_be_bytes(buf))
}

fn read_string<R: Read>(r: &mut R) -> io::Result<String> {
    let len = read_u32(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?)
}

fn read_property_value<R: Read>(r: &mut R) -> io::Result<PropertyValue> {
    let mut type_tag = [0u8; 1];
    r.read_exact(&mut type_tag)?;
    match type_tag[0] {
        t if t == ValueType::String as u8 => {
            let s = read_string(r)?;
            Ok(PropertyValue::String(s))
        }
        t if t == ValueType::Int as u8 => {
            let v = read_u64(r)? as i64;
            Ok(PropertyValue::Int(v))
        }
        t if t == ValueType::Double as u8 => {
            let v = read_f64(r)?;
            Ok(PropertyValue::Double(v))
        }
        t if t == ValueType::Bool as u8 => {
            let mut b = [0u8; 1];
            r.read_exact(&mut b)?;
            Ok(PropertyValue::Bool(b[0] != 0))
        }
        t if t == ValueType::Null as u8 => {
            Ok(PropertyValue::Null)
        }
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown property value type")),
    }
}

// ── 图数据库（带持久化的完整封装）─────────────────

/// 带持久化的属性图数据库
///
/// 设计：
/// - 二进制存储（快，紧凑）
/// - 顶点/边支持属性
/// - 加载后自动构建 CSR + EdgeBlock（供 GPU 算法使用）
/// - 增量变更后支持 save() 写回
pub struct PersistentGraph {
    /// 顶点: id → (properties, out_degree 在构建 CSR 时用)
    pub vertices: HashMap<u64, VertexRecord>,
    /// 边: (from, to) → EdgeRecord
    pub edges: HashMap<(u64, u64), EdgeRecord>,
    /// 文件路径（用于 save()）
    pub file_path: String,
    /// 索引管理器（可选，默认不启用）
    pub index_manager: crate::index::IndexManager,
}

#[derive(Debug, Clone)]
pub struct VertexRecord {
    pub properties: HashMap<String, PropertyValue>,
}

#[derive(Debug, Clone)]
pub struct EdgeRecord {
    pub properties: HashMap<String, PropertyValue>,
    pub weight: f64,
}

impl PersistentGraph {
    /// 打开数据库（不存在则创建）
    pub fn open(path: &str) -> io::Result<Self> {
        let mut g = PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: path.to_string(),
            index_manager: crate::index::IndexManager::new(),
        };

        if fs::metadata(path).is_ok() {
            g.load()?;
            // 加载后重建索引（如果有预定义的索引）
            g.index_manager.rebuild_all(&g.vertices);
        }

        Ok(g)
    }

    /// 创建索引
    pub fn create_index(&mut self, name: &str, property_key: &str, unique: bool) -> Result<(), String> {
        self.index_manager.create_index(
            name,
            property_key,
            crate::index::IndexType::Hash,
            unique,
        )?;
        // 立即从现有数据构建索引
        self.index_manager.rebuild_property(property_key, &self.vertices);
        Ok(())
    }

    /// 删除索引
    pub fn drop_index(&mut self, name: &str) -> Result<(), String> {
        self.index_manager.drop_index(name)
    }

    /// 列出所有索引
    pub fn list_indexes(&self) -> Vec<crate::index::IndexDef> {
        self.index_manager.list_indexes()
    }

    /// 添加顶点（自动维护索引）
    pub fn add_vertex(&mut self, id: u64, properties: HashMap<String, PropertyValue>) {
        self.vertices.insert(id, VertexRecord { properties: properties.clone() });
        // 更新索引
        self.index_manager.on_vertex_added(id, &properties);
    }

    /// 删除顶点（自动维护索引）
    /// 返回被删除顶点的属性（如果存在）
    pub fn delete_vertex(&mut self, id: u64) -> Option<HashMap<String, PropertyValue>> {
        let mut result = None;
        if let Some(vr) = self.vertices.get(&id) {
            let properties = vr.properties.clone();
            // 更新索引
            self.index_manager.on_vertex_deleted(id, &properties);
            self.vertices.remove(&id);
            result = Some(properties);
        }
        // 删除相关边
        self.edges.retain(|(from, to), _| *from != id && *to != id);
        result
    }

    /// 删除边（自动维护索引）
    /// 返回被删除边的（权重, 属性）（如果存在）
    pub fn delete_edge(&mut self, from: u64, to: u64) -> Option<(f64, HashMap<String, PropertyValue>)> {
        self.edges.remove(&(from, to))
            .map(|er| (er.weight, er.properties))
    }

    /// 更新顶点属性（自动维护索引）
    pub fn update_vertex(&mut self, id: u64, new_properties: HashMap<String, PropertyValue>) {
        if let Some(vr) = self.vertices.get_mut(&id) {
            let old_properties = vr.properties.clone();
            vr.properties = new_properties.clone();
            // 更新索引
            self.index_manager.on_vertex_updated(id, &old_properties, &new_properties);
        }
    }

    /// 添加边
    pub fn add_edge(&mut self, from: u64, to: u64, weight: f64, properties: HashMap<String, PropertyValue>) {
        // 自动创建不存在的顶点（无属性）
        if !self.vertices.contains_key(&from) {
            self.vertices.insert(from, VertexRecord { properties: HashMap::new() });
        }
        if !self.vertices.contains_key(&to) {
            self.vertices.insert(to, VertexRecord { properties: HashMap::new() });
        }
        self.edges.insert((from, to), EdgeRecord { properties, weight });
    }

    /// 保存到二进制文件
    pub fn save(&self) -> io::Result<()> {
        let mut buf: Vec<u8> = Vec::new();

        // ── Header (40 bytes) ──
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&VERSION.to_be_bytes());
        buf.extend_from_slice(&(self.vertices.len() as u64).to_be_bytes());
        buf.extend_from_slice(&(self.edges.len() as u64).to_be_bytes());
        buf.extend_from_slice(&[0u8; 16]);

        // ── Vertices（按 id 排序）──
        let mut vertex_ids: Vec<u64> = self.vertices.keys().copied().collect();
        vertex_ids.sort_unstable();

        for id in &vertex_ids {
            let vr = &self.vertices[id];
            buf.extend_from_slice(&id.to_be_bytes());
            write_u32(&mut buf, vr.properties.len() as u32)?;
            let mut keys: Vec<&String> = vr.properties.keys().collect();
            keys.sort_unstable();
            for key in keys {
                write_string(&mut buf, key)?;
                write_property_value(&mut buf, &vr.properties[key])?;
            }
        }

        // ── Edges（按 (from, to) 排序）──
        let mut edge_keys: Vec<(u64, u64)> = self.edges.keys().copied().collect();
        edge_keys.sort_unstable();

        for (from, to) in &edge_keys {
            let er = &self.edges[&(*from, *to)];
            buf.extend_from_slice(&from.to_be_bytes());
            buf.extend_from_slice(&to.to_be_bytes());
            write_f64(&mut buf, er.weight)?;
            write_u32(&mut buf, er.properties.len() as u32)?;
            let mut keys: Vec<&String> = er.properties.keys().collect();
            keys.sort_unstable();
            for key in keys {
                write_string(&mut buf, key)?;
                write_property_value(&mut buf, &er.properties[key])?;
            }
        }

        // 原子写入
        let tmp_path = format!("{}.tmp", self.file_path);
        {
            let mut f = File::create(&tmp_path)?;
            f.write_all(&buf)?;
            f.sync_all()?;
        }
        fs::rename(&tmp_path, &self.file_path)?;

        Ok(())
    }

    /// 从二进制文件加载
    pub fn load(&mut self) -> io::Result<()> {
        let mut f = File::open(&self.file_path)?;

        // ── Header ──
        let mut magic = [0u8; 4];
        f.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid magic bytes"));
        }

        let version = read_u16(&mut f)?;
        if version != VERSION {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported version"));
        }

        let vertex_count = read_u64(&mut f)? as usize;
        let edge_count = read_u64(&mut f)? as usize;
        let mut reserved = [0u8; 16];
        f.read_exact(&mut reserved)?;

        // ── Vertices ──
        for _ in 0..vertex_count {
            let id = read_u64(&mut f)?;
            let property_count = read_u32(&mut f)? as usize;

            let mut properties = HashMap::new();
            for _ in 0..property_count {
                let key = read_string(&mut f)?;
                let value = read_property_value(&mut f)?;
                properties.insert(key, value);
            }

            self.vertices.insert(id, VertexRecord { properties });
        }

        // ── Edges ──
        for _ in 0..edge_count {
            let from = read_u64(&mut f)?;
            let to = read_u64(&mut f)?;
            let weight = read_f64(&mut f)?;
            let property_count = read_u32(&mut f)? as usize;

            let mut properties = HashMap::new();
            for _ in 0..property_count {
                let key = read_string(&mut f)?;
                let value = read_property_value(&mut f)?;
                properties.insert(key, value);
            }

            self.edges.insert((from, to), EdgeRecord { properties, weight });
        }

        Ok(())
    }

    /// 导出为 CSRGraph（供 GPU 算法使用）
    ///
    /// 每次运行算法前调用，将 PersistentGraph 转换为 CSRGraph。
    /// 后续可以优化为增量更新 CSR（只在边变更时重建）。
    pub fn to_csr(&self) -> CSRGraph {
        let mut graph = CSRGraph::new();

        // 添加顶点
        let mut sorted_ids: Vec<u64> = self.vertices.keys().copied().collect();
        sorted_ids.sort_unstable();
        for id in &sorted_ids {
            let vr = &self.vertices[id];
            graph.add_vertex(*id, vr.properties.clone());
        }

        // 添加边（收集所有边，然后调用 build_csr）
        let mut edges: Vec<(u64, u64)> = Vec::new();
        let mut sorted_edges: Vec<(u64, u64)> = self.edges.keys().copied().collect();
        sorted_edges.sort_unstable();
        for (from, to) in &sorted_edges {
            edges.push((*from, *to));
        }

        graph.build_csr(&edges);

        // 填充权重（build_csr 不处理权重）
        graph.weights = Vec::with_capacity(sorted_edges.len());
        for (from, to) in &sorted_edges {
            let er = &self.edges[&(*from, *to)];
            graph.weights.push(er.weight as f32);
        }

        graph
    }
}

// ── 测试 ───────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_persistent_graph_save_load() {
        let test_path = "/tmp/test_persistent_graph.bin";

        // 创建并保存
        let mut g = PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: test_path.to_string(),
            index_manager: crate::index::IndexManager::new(),
        };

        let mut alice_props = HashMap::new();
        alice_props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        alice_props.insert("age".to_string(), PropertyValue::Int(30));
        alice_props.insert("score".to_string(), PropertyValue::Double(95.5));
        alice_props.insert("active".to_string(), PropertyValue::Bool(true));
        g.add_vertex(1, alice_props);

        let mut bob_props = HashMap::new();
        bob_props.insert("name".to_string(), PropertyValue::String("Bob".to_string()));
        g.add_vertex(2, bob_props);

        let mut edge_props = HashMap::new();
        edge_props.insert("type".to_string(), PropertyValue::String("knows".to_string()));
        g.add_edge(1, 2, 1.0, edge_props);

        g.save().expect("save failed");

        // 加载并验证
        let mut g2 = PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: test_path.to_string(),
            index_manager: crate::index::IndexManager::new(),
        };
        g2.load().expect("load failed");

        assert_eq!(g2.vertices.len(), 2);
        assert_eq!(g2.edges.len(), 1);

        let alice = &g2.vertices[&1];
        assert_eq!(alice.properties["name"], PropertyValue::String("Alice".to_string()));
        assert_eq!(alice.properties["age"], PropertyValue::Int(30));

        // 清理
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_persistent_graph_to_csr_and_pagerank() {
        let test_path = "/tmp/test_persistent_graph_pagerank.bin";

        // 创建一个有 5 个顶点的链状图: 0→1→2→3→4
        let mut g = PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: test_path.to_string(),
            index_manager: crate::index::IndexManager::new(),
        };

        for i in 0..5u64 {
            let mut props = HashMap::new();
            props.insert("label".to_string(), PropertyValue::String(format!("V{}", i)));
            g.add_vertex(i, props);
        }

        // 0→1, 1→2, 2→3, 3→4, 0→4（额外一条跨边）
        let edges = [(0u64, 1u64), (1, 2), (2, 3), (3, 4), (0, 4)];
        for (from, to) in &edges {
            g.add_edge(*from, *to, 1.0, HashMap::new());
        }

        g.save().expect("save failed");

        // 加载
        let mut g2 = PersistentGraph::open(test_path).expect("open failed");
        assert_eq!(g2.vertices.len(), 5);
        assert_eq!(g2.edges.len(), 5);

        // 转 CSR
        let csr = g2.to_csr();
        assert_eq!(csr.vertex_count, 5);
        assert_eq!(csr.total_edges, 5);
        assert_eq!(csr.weights.len(), 5);

        // 验证权重正确
        for w in &csr.weights {
            assert_eq!(*w, 1.0f32);
        }

        // 验证 CSR 结构：顶点 0 有 2 条出边（0→1 和 0→4）
        let v0_idx = csr.vertex_to_idx[&0];
        let out_start = csr.offsets[v0_idx as usize] as usize;
        let out_end = csr.offsets[v0_idx as usize + 1] as usize;
        assert_eq!(out_end - out_start, 2, "vertex 0 should have 2 outgoing edges");

        // 验证顶点 4 有 1 条入边（3→4 或 0→4，实际是 0→4 和 3→4，所以入边数是 2）
        let v4_idx = csr.vertex_to_idx[&4];
        let in_start = csr.reverse_offsets[v4_idx as usize] as usize;
        let in_end = csr.reverse_offsets[v4_idx as usize + 1] as usize;
        assert_eq!(in_end - in_start, 2, "vertex 4 should have 2 incoming edges");

        // 如果能访问 GPU，跑 PageRank 验证数值
        // （CI 环境无 GPU，所以这里只是结构验证）

        // 清理
        let _ = fs::remove_file(test_path);
    }
}
