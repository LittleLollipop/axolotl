// src/mvcc.rs
// MVCC 快照隔离实现
//
// 设计：
// - 用 Arc<VertexRecord> 和 Arc<EdgeRecord> 实现 Copy-on-Write
// - 快照：克隆 HashMap（只复制 Arc 指针，非常快）
// - 修改：创建新记录，替换 HashMap 中的 Arc
// - 旧快照仍然引用旧记录，实现隔离
//
// 线程安全：
// - MvccGraphHandle 包装 Arc<RwLock<InnerMvccGraph>>
// - 支持多线程并发读写
// - 快照隔离：读不阻塞写，写不阻塞读

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::io;
use crate::PropertyValue;
use crate::persistence::{PersistentGraph, VertexRecord, EdgeRecord};

// ── 内部 MVCC 图（非线程安全）─────────────────────

struct InnerMvccGraph {
    vertices: HashMap<u64, Arc<VertexRecord>>,
    edges: HashMap<(u64, u64), Arc<EdgeRecord>>,
    file_path: String,
}

// ── 线程安全的 MVCC 图句柄 ─────────────────────

/// MVCC 图的线程安全句柄
///
/// 用法：
/// ```rust
/// let graph = MvccGraphHandle::open("graph.bin")?;
///
/// // 创建快照（非常快，O(1)）
/// let snapshot = graph.snapshot();
///
/// // 在快照上跑算法（看到一致的图）
/// let pr = pagerank(&snapshot.to_csr(), 20, 0.85);
///
/// // 并发写入
/// graph.add_vertex(1, props)?;
/// ```
pub struct MvccGraphHandle {
    inner: Arc<RwLock<InnerMvccGraph>>,
}

impl MvccGraphHandle {
    /// 打开数据库（不存在则创建）
    pub fn open(path: &str) -> io::Result<Self> {
        let inner = InnerMvccGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: path.to_string(),
        };

        let handle = MvccGraphHandle {
            inner: Arc::new(RwLock::new(inner)),
        };

        // 如果文件存在，加载
        if std::fs::metadata(path).is_ok() {
            handle.load()?;
        }

        Ok(handle)
    }

    /// 创建快照（非常快，只克隆 Arc 指针）
    ///
    /// 快照是只读的，看到的是创建快照时的图状态
    /// 后续对原图的修改不会影响快照
    pub fn snapshot(&self) -> MvccSnapshot {
        let inner = self.inner.read().unwrap();
        MvccSnapshot {
            vertices: inner.vertices.clone(), // 只克隆 Arc，非常快
            edges: inner.edges.clone(),       // 只克隆 Arc，非常快
        }
    }

    /// 添加顶点
    pub fn add_vertex(&self, id: u64, properties: HashMap<String, PropertyValue>) -> io::Result<()> {
        let mut inner = self.inner.write().unwrap();
        let record = Arc::new(VertexRecord { properties });
        inner.vertices.insert(id, record);
        Ok(())
    }

    /// 添加边
    pub fn add_edge(&self, from: u64, to: u64, weight: f64, properties: HashMap<String, PropertyValue>) -> io::Result<()> {
        let mut inner = self.inner.write().unwrap();

        // 自动创建不存在的顶点（无属性）
        if !inner.vertices.contains_key(&from) {
            inner.vertices.insert(from, Arc::new(VertexRecord { properties: HashMap::new() }));
        }
        if !inner.vertices.contains_key(&to) {
            inner.vertices.insert(to, Arc::new(VertexRecord { properties: HashMap::new() }));
        }

        let record = Arc::new(EdgeRecord { properties, weight });
        inner.edges.insert((from, to), record);

        Ok(())
    }

    /// 更新顶点属性（Copy-on-Write）
    pub fn update_vertex(&self, id: u64, properties: HashMap<String, PropertyValue>) -> io::Result<()> {
        let mut inner = self.inner.write().unwrap();
        let record = Arc::new(VertexRecord { properties });
        inner.vertices.insert(id, record);
        Ok(())
    }

    /// 更新边属性（Copy-on-Write）
    pub fn update_edge(&self, from: u64, to: u64, weight: f64, properties: HashMap<String, PropertyValue>) -> io::Result<()> {
        let mut inner = self.inner.write().unwrap();
        let record = Arc::new(EdgeRecord { properties, weight });
        inner.edges.insert((from, to), record);
        Ok(())
    }

    /// 删除顶点
    pub fn delete_vertex(&self, id: u64) -> io::Result<()> {
        let mut inner = self.inner.write().unwrap();
        inner.vertices.remove(&id);
        // 删除关联的边
        inner.edges.retain(|(f, t), _| *f != id && *t != id);
        Ok(())
    }

    /// 删除边
    pub fn delete_edge(&self, from: u64, to: u64) -> io::Result<()> {
        let mut inner = self.inner.write().unwrap();
        inner.edges.remove(&(from, to));
        Ok(())
    }

    /// 保存到二进制文件
    pub fn save(&self) -> io::Result<()> {
        let inner = self.inner.read().unwrap();
        let pg = Self::inner_to_persistent(&inner);
        pg.save()
    }

    /// 从二进制文件加载
    pub fn load(&self) -> io::Result<()> {
        let mut inner = self.inner.write().unwrap();
        let pg = PersistentGraph::open(&inner.file_path)?;

        inner.vertices.clear();
        inner.edges.clear();

        for (id, vr) in &pg.vertices {
            inner.vertices.insert(*id, Arc::new(vr.clone()));
        }

        for (key, er) in &pg.edges {
            inner.edges.insert(*key, Arc::new(er.clone()));
        }

        Ok(())
    }

    /// 转换为 PersistentGraph
    fn inner_to_persistent(inner: &InnerMvccGraph) -> PersistentGraph {
        let mut pg = PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: inner.file_path.clone(),
        };

        for (id, vr) in &inner.vertices {
            pg.vertices.insert(*id, vr.as_ref().clone());
        }

        for (key, er) in &inner.edges {
            pg.edges.insert(*key, er.as_ref().clone());
        }

        pg
    }

    /// 顶点数量
    pub fn vertex_count(&self) -> usize {
        let inner = self.inner.read().unwrap();
        inner.vertices.len()
    }

    /// 边数量
    pub fn edge_count(&self) -> usize {
        let inner = self.inner.read().unwrap();
        inner.edges.len()
    }
}

// ── 快照 ─────────────────────────

/// 图的只读快照
///
/// 创建快照非常快（只克隆 Arc 指针）
/// 快照看到的是创建时的图状态
/// 后续对原图的修改不会影响快照
///
/// 用法：
/// ```rust
/// let snapshot = graph.snapshot();
///
/// // 在快照上跑算法（看到一致的图）
/// let csr = snapshot.to_csr();
/// let pr = pagerank(&csr, 20, 0.85);
///
/// // 快照可以跨线程传递
/// std::thread::spawn(move || {
///     let csr = snapshot.to_csr();
///     // 使用 csr...
/// });
/// ```
#[derive(Clone)]
pub struct MvccSnapshot {
    vertices: HashMap<u64, Arc<VertexRecord>>,
    edges: HashMap<(u64, u64), Arc<EdgeRecord>>,
}

impl MvccSnapshot {
    /// 获取顶点属性
    pub fn get_vertex(&self, id: u64) -> Option<&VertexRecord> {
        self.vertices.get(&id).map(|arc| arc.as_ref())
    }

    /// 获取边属性
    pub fn get_edge(&self, from: u64, to: u64) -> Option<&EdgeRecord> {
        self.edges.get(&(from, to)).map(|arc| arc.as_ref())
    }

    /// 导出为 CSRGraph（供 GPU 算法使用）
    pub fn to_csr(&self) -> crate::csr_graph::CSRGraph {
        let mut graph = crate::csr_graph::CSRGraph::new();

        // 添加顶点
        let mut sorted_ids: Vec<u64> = self.vertices.keys().copied().collect();
        sorted_ids.sort_unstable();
        for id in &sorted_ids {
            let vr = &self.vertices[id];
            graph.add_vertex(*id, vr.properties.clone());
        }

        // 添加边
        let mut edges: Vec<(u64, u64)> = Vec::new();
        let mut sorted_edges: Vec<(u64, u64)> = self.edges.keys().copied().collect();
        sorted_edges.sort_unstable();
        for (from, to) in &sorted_edges {
            edges.push((*from, *to));
        }

        graph.build_csr(&edges);

        // 填充权重
        graph.weights = Vec::with_capacity(sorted_edges.len());
        for (from, to) in &sorted_edges {
            let er = &self.edges[&(*from, *to)];
            graph.weights.push(er.weight as f32);
        }

        graph
    }

    /// 顶点数量
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// 边数量
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// 获取所有顶点 ID
    pub fn vertex_ids(&self) -> Vec<u64> {
        self.vertices.keys().copied().collect()
    }

    /// 获取所有边
    pub fn edges(&self) -> Vec<((u64, u64), &EdgeRecord)> {
        self.edges.iter().map(|(k, v)| (*k, v.as_ref())).collect()
    }
}

// ── 测试 ───────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;

    #[test]
    fn test_mvcc_snapshot_isolation() {
        let test_path = "/tmp/test_mvcc.bin";

        // 删除旧文件
        let _ = fs::remove_file(test_path);

        let g = MvccGraphHandle::open(test_path).unwrap();

        // 添加顶点
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        g.add_vertex(1, props).unwrap();

        // 创建快照
        let snapshot1 = g.snapshot();

        // 修改图（添加 Bob）
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Bob".to_string()));
        g.add_vertex(2, props).unwrap();

        // 快照 1 应该看不到 Bob
        assert_eq!(snapshot1.vertex_count(), 1);
        assert!(snapshot1.get_vertex(2).is_none());

        // 新快照应该看到 Bob
        let snapshot2 = g.snapshot();
        assert_eq!(snapshot2.vertex_count(), 2);
        assert!(snapshot2.get_vertex(2).is_some());

        // 清理
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_mvcc_concurrent_read_write() {
        let test_path = "/tmp/test_mvcc_concurrent.bin";

        // 删除旧文件
        let _ = fs::remove_file(test_path);

        let g = MvccGraphHandle::open(test_path).unwrap();

        // 添加初始顶点
        for i in 0..100u64 {
            let mut props = HashMap::new();
            props.insert("id".to_string(), PropertyValue::Int(i as i64));
            g.add_vertex(i, props).unwrap();
        }

        // 创建快照
        let snapshot = g.snapshot();

        // 在另一个线程修改图
        let g_clone = MvccGraphHandle {
            inner: g.inner.clone(),
        };
        let handle = thread::spawn(move || {
            let mut props = HashMap::new();
            props.insert("name".to_string(), PropertyValue::String("New".to_string()));
            g_clone.add_vertex(100, props).unwrap();
        });

        handle.join().unwrap();

        // 快照看到的是一致的（100 个顶点）
        assert_eq!(snapshot.vertex_count(), 100);

        // 新快照看到 101 个顶点
        let snapshot2 = g.snapshot();
        assert_eq!(snapshot2.vertex_count(), 101);

        // 清理
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_mvcc_snapshot_to_csr() {
        let test_path = "/tmp/test_mvcc_csr.bin";

        // 删除旧文件
        let _ = fs::remove_file(test_path);

        let g = MvccGraphHandle::open(test_path).unwrap();

        // 添加顶点和边
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("A".to_string()));
        g.add_vertex(0, props.clone()).unwrap();

        props.insert("name".to_string(), PropertyValue::String("B".to_string()));
        g.add_vertex(1, props).unwrap();

        g.add_edge(0, 1, 1.0, HashMap::new()).unwrap();

        // 创建快照
        let snapshot = g.snapshot();

        // 快照转 CSR
        let csr = snapshot.to_csr();
        assert_eq!(csr.vertex_count, 2);
        assert_eq!(csr.total_edges, 1);

        // 清理
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_mvcc_copy_on_write() {
        let test_path = "/tmp/test_mvcc_cow.bin";

        // 删除旧文件
        let _ = fs::remove_file(test_path);

        let g = MvccGraphHandle::open(test_path).unwrap();

        // 添加顶点
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        g.add_vertex(1, props.clone()).unwrap();

        // 创建快照
        let snapshot = g.snapshot();

        // 修改顶点（Copy-on-Write）
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        props.insert("age".to_string(), PropertyValue::Int(30));
        g.update_vertex(1, props).unwrap();

        // 快照看到的是旧值
        let old_vertex = snapshot.get_vertex(1).unwrap();
        assert!(old_vertex.properties.get("age").is_none());

        // 新快照看到的是新值
        let snapshot2 = g.snapshot();
        let new_vertex = snapshot2.get_vertex(1).unwrap();
        assert_eq!(new_vertex.properties["age"], PropertyValue::Int(30));

        // 清理
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_mvcc_snapshot_cross_thread() {
        let test_path = "/tmp/test_mvcc_cross_thread.bin";

        // 删除旧文件
        let _ = fs::remove_file(test_path);

        let g = MvccGraphHandle::open(test_path).unwrap();

        // 添加顶点
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        g.add_vertex(1, props).unwrap();

        // 创建快照
        let snapshot = g.snapshot();

        // 快照可以跨线程传递
        let handle = thread::spawn(move || {
            // 在另一个线程使用快照
            assert_eq!(snapshot.vertex_count(), 1);
            let v = snapshot.get_vertex(1).unwrap();
            assert_eq!(v.properties["name"], PropertyValue::String("Alice".to_string()));
        });

        handle.join().unwrap();

        // 清理
        let _ = fs::remove_file(test_path);
    }
}
