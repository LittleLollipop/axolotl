// src/graph_db.rs
// 统一图数据库接口（内存模式 / mmap 模式切换）
//
// 使用方式：
//
// // 内存模式（完整加载，支持读写）
// let db = GraphDB::open("graph.bin", GraphMode::InMemory)?;
//
// // mmap 模式（只读，支持大于内存的图）
// let db = GraphDB::open("graph.bin", GraphMode::Mmap)?;
// // 或指定 mmap 文件路径
// let db = GraphDB::open_with_mmap("graph.bin", "graph.mmap")?;
//
// // 统一 API（两种模式都支持）
// db.get_vertex(id)
// db.out_neighbors(id)
// db.to_csr()  // 转 CSR 供 GPU 算法

use std::collections::HashMap;
use std::path::Path;

use crate::{PropertyValue, PersistentGraph, CSRGraph};
use crate::mmap_graph::MmapGraph;

// ── 模式选择 ─────────────────────────

/// 图数据库打开模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMode {
    /// 完整加载到内存（支持读写，需要足够内存）
    InMemory,
    /// mmap 只读模式（支持大于内存的图，只读）
    Mmap,
}

// ── 统一错误类型 ─────────────────────────

#[derive(Debug)]
pub enum GraphDBError {
    Io(String),
    NotSupported(String),
    NotFound(String),
}

impl From<std::io::Error> for GraphDBError {
    fn from(e: std::io::Error) -> Self {
        GraphDBError::Io(e.to_string())
    }
}

// ── 统一图数据库句柄 ─────────────────────────

/// 统一图数据库句柄
///
/// 支持两种模式：
/// - InMemory：完整加载到内存，支持读写
/// - Mmap：mmap 只读，支持大于内存的图
///
/// 两种模式对外暴露同一套只读 API；
/// 写操作仅在 InMemory 模式下可用。
pub struct GraphDB {
    mode: GraphMode,

    // ---- InMemory 模式 ----
    persistent: Option<PersistentGraph>,

    // ---- Mmap 模式 ----
    mmap: Option<MmapGraph>,
}

impl GraphDB {
    // ── 打开数据库 ─────────────────────────

    /// 打开图数据库（自动选择模式）
    ///
    /// - InMemory：从二进制文件完整加载到内存
    /// - Mmap：打开 mmap 文件（如果不存在则自动从二进制文件生成）
    pub fn open<P: AsRef<Path>>(path: P, mode: GraphMode) -> Result<Self, GraphDBError> {
        match mode {
            GraphMode::InMemory => {
                let g = PersistentGraph::open(path.as_ref().to_str().unwrap())?;
                Ok(GraphDB {
                    mode,
                    persistent: Some(g),
                    mmap: None,
                })
            }
            GraphMode::Mmap => {
                // 尝试直接打开 .mmap 文件；如果不存在，自动从二进制文件生成
                let path_str = path.as_ref().to_str().unwrap();
                let mmap_path = format!("{}.mmap", path_str);

                if !std::path::Path::new(&mmap_path).exists() {
                    // 需要先从 PersistentGraph 生成 mmap 文件
                    if !std::path::Path::new(path_str).exists() {
                        return Err(GraphDBError::Io(format!("neither {} nor {} exists", path_str, mmap_path)));
                    }
                    let src = PersistentGraph::open(path_str)?;
                    MmapGraph::convert_from(&src, &mmap_path)
                        .map_err(|e| GraphDBError::Io(e.to_string()))?;
                }

                let mmap_g = MmapGraph::open(&mmap_path)
                    .map_err(|e| GraphDBError::Io(e.to_string()))?;
                Ok(GraphDB {
                    mode,
                    persistent: None,
                    mmap: Some(mmap_g),
                })
            }
        }
    }

    /// 打开数据库（显式指定 mmap 文件路径）
    pub fn open_with_mmap<P: AsRef<Path>>(
        bin_path:  P,
        mmap_path: P,
        mode:      GraphMode,
    ) -> Result<Self, GraphDBError> {
        match mode {
            GraphMode::InMemory => Self::open(bin_path, mode),
            GraphMode::Mmap => {
                let mmap_path_str = mmap_path.as_ref().to_str().unwrap();
                if !std::path::Path::new(mmap_path_str).exists() {
                    let src = PersistentGraph::open(bin_path.as_ref().to_str().unwrap())?;
                    MmapGraph::convert_from(&src, mmap_path_str)
                        .map_err(|e| GraphDBError::Io(e.to_string()))?;
                }
                let mmap_g = MmapGraph::open(mmap_path_str)
                    .map_err(|e| GraphDBError::Io(e.to_string()))?;
                Ok(GraphDB {
                    mode,
                    persistent: None,
                    mmap: Some(mmap_g),
                })
            }
        }
    }

    /// 创建内存模式数据库（新建，不加载文件）
    pub fn create_in_memory(file_path: &str) -> Result<Self, GraphDBError> {
        let g = PersistentGraph {
            vertices:      HashMap::new(),
            edges:         HashMap::new(),
            file_path:     file_path.to_string(),
            index_manager:  crate::index::IndexManager::new(),
        };
        Ok(GraphDB {
            mode: GraphMode::InMemory,
            persistent: Some(g),
            mmap: None,
        })
    }

    // ── 只读 API（两种模式都支持）─────────────────────────

    /// 顶点数量
    pub fn vertex_count(&self) -> usize {
        match self.mode {
            GraphMode::InMemory => self.persistent.as_ref().unwrap().vertices.len(),
            GraphMode::Mmap    => self.mmap.as_ref().unwrap().vertex_count(),
        }
    }

    /// 边数量
    pub fn edge_count(&self) -> usize {
        match self.mode {
            GraphMode::InMemory => self.persistent.as_ref().unwrap().edges.len(),
            GraphMode::Mmap    => self.mmap.as_ref().unwrap().edge_count(),
        }
    }

    /// 获取顶点属性
    pub fn get_vertex(&self, id: u64) -> Option<HashMap<String, PropertyValue>> {
        match self.mode {
            GraphMode::InMemory => {
                self.persistent.as_ref().unwrap().vertices.get(&id)
                    .map(|vr| vr.properties.clone())
            }
            GraphMode::Mmap => {
                self.mmap.as_ref().unwrap().get_vertex(id)
            }
        }
    }

    /// 获取边（权重 + 属性）
    pub fn get_edge(&self, from: u64, to: u64) -> Option<(f64, HashMap<String, PropertyValue>)> {
        match self.mode {
            GraphMode::InMemory => {
                self.persistent.as_ref().unwrap().edges.get(&(from, to))
                    .map(|er| (er.weight, er.properties.clone()))
            }
            GraphMode::Mmap => {
                self.mmap.as_ref().unwrap().get_edge(from, to)
            }
        }
    }

    /// 获取出边邻居列表
    pub fn out_neighbors(&self, id: u64) -> Vec<u64> {
        match self.mode {
            GraphMode::InMemory => {
                // 扫描 edges HashMap
                let g = self.persistent.as_ref().unwrap();
                g.edges.keys()
                    .filter(|(f, _)| *f == id)
                    .map(|(_, t)| *t)
                    .collect()
            }
            GraphMode::Mmap => {
                self.mmap.as_ref().unwrap().out_neighbors(id)
            }
        }
    }

    /// 转换为 CSR（供 GPU 算法使用）
    pub fn to_csr(&self) -> CSRGraph {
        match self.mode {
            GraphMode::InMemory => {
                self.persistent.as_ref().unwrap().to_csr()
            }
            GraphMode::Mmap => {
                self.mmap.as_ref().unwrap().to_csr()
            }
        }
    }

    /// 迭代所有顶点
    pub fn iter_vertices(&self) -> Box<dyn Iterator<Item = (u64, HashMap<String, PropertyValue>)> + '_> {
        match self.mode {
            GraphMode::InMemory => {
                let g = self.persistent.as_ref().unwrap();
                Box::new(g.vertices.iter().map(|(id, vr)| (*id, vr.properties.clone())))
            }
            GraphMode::Mmap => {
                Box::new(self.mmap.as_ref().unwrap().iter_vertices())
            }
        }
    }

    /// 迭代所有边
    pub fn iter_edges(&self) -> Box<dyn Iterator<Item = (u64, u64, f64, HashMap<String, PropertyValue>)> + '_> {
        match self.mode {
            GraphMode::InMemory => {
                let g = self.persistent.as_ref().unwrap();
                Box::new(g.edges.iter().map(|((from, to), er)| (*from, *to, er.weight, er.properties.clone())))
            }
            GraphMode::Mmap => {
                Box::new(
                    self.mmap.as_ref().unwrap().iter_edges()
                        .map(|(from, to, weight, props)| (from, to, weight, props))
                )
            }
        }
    }

    /// 获取当前模式
    pub fn mode(&self) -> GraphMode {
        self.mode
    }

    // ── 写操作 API（仅 InMemory 模式）─────────────────────────

    /// 添加顶点（仅 InMemory 模式）
    pub fn add_vertex(&mut self, id: u64, properties: HashMap<String, PropertyValue>) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                self.persistent.as_mut().unwrap().add_vertex(id, properties);
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("add_vertex not supported in Mmap mode. Switch to InMemory mode for write operations.".to_string()))
            }
        }
    }

    /// 添加边（仅 InMemory 模式）
    pub fn add_edge(
        &mut self,
        from:      u64,
        to:        u64,
        weight:    f64,
        properties: HashMap<String, PropertyValue>,
    ) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                self.persistent.as_mut().unwrap().add_edge(from, to, weight, properties);
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("add_edge not supported in Mmap mode. Switch to InMemory mode for write operations.".to_string()))
            }
        }
    }

    /// 删除顶点（仅 InMemory 模式）
    /// 返回被删除顶点的属性（如果存在）
    pub fn delete_vertex(&mut self, id: u64) -> Result<Option<HashMap<String, PropertyValue>>, GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                let result = self.persistent.as_mut().unwrap().delete_vertex(id);
                Ok(result)
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("delete_vertex not supported in Mmap mode.".to_string()))
            }
        }
    }

    /// 删除边（仅 InMemory 模式）
    pub fn delete_edge(&mut self, from: u64, to: u64) -> Result<Option<(f64, HashMap<String, PropertyValue>)>, GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                let result = self.persistent.as_mut().unwrap().delete_edge(from, to);
                Ok(result)
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("delete_edge not supported in Mmap mode.".to_string()))
            }
        }
    }

    /// 保存（仅 InMemory 模式）
    pub fn save(&self) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                self.persistent.as_ref().unwrap().save()
                    .map_err(|e| GraphDBError::Io(e.to_string()))?;
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("save not supported in Mmap mode. Data is read-only.".to_string()))
            }
        }
    }

    /// 生成 mmap 文件（从当前 InMemory 数据）
    /// 生成后可以用 Mmap 模式打开
    pub fn generate_mmap_file(&self, mmap_path: &str) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                let g = self.persistent.as_ref().unwrap();
                MmapGraph::convert_from(g, mmap_path)
                    .map_err(|e| GraphDBError::Io(e.to_string()))?;
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("generate_mmap_file only works in InMemory mode.".to_string()))
            }
        }
    }

    // ── 索引 API（仅 InMemory 模式）─────────────────────────

    /// 创建索引（仅 InMemory 模式）
    pub fn create_index(&mut self, name: &str, property_key: &str, unique: bool) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                self.persistent.as_mut().unwrap().create_index(name, property_key, unique)
                    .map_err(|e| GraphDBError::Io(e.to_string()))?;
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("create_index not supported in Mmap mode.".to_string()))
            }
        }
    }

    /// 删除索引（仅 InMemory 模式）
    pub fn drop_index(&mut self, name: &str) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                self.persistent.as_mut().unwrap().drop_index(name);
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("drop_index not supported in Mmap mode.".to_string()))
            }
        }
    }
}

// ── 测试 ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PropertyValue;
    use std::collections::HashMap;

    fn make_test_graph() -> GraphDB {
        // 使用 open（如果文件存在则加载，不存在则创建空数据库）
        let file_path = "/tmp/test_graph_db.bin";
        // 先删除旧文件
        let _ = std::fs::remove_file(file_path);

        let mut db = GraphDB::create_in_memory(file_path).unwrap();

        let mut alice = HashMap::new();
        alice.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        alice.insert("age".to_string(),  PropertyValue::Int(28));
        db.add_vertex(1, alice).unwrap();

        let mut bob = HashMap::new();
        bob.insert("name".to_string(), PropertyValue::String("Bob".to_string()));
        db.add_vertex(2, bob).unwrap();

        db.add_edge(1, 2, 1.0, HashMap::new()).unwrap();

        // 保存（创建二进制文件）
        db.save().unwrap();

        db
    }

    #[test]
    fn test_in_memory_mode() {
        let db = make_test_graph();

        assert_eq!(db.mode(), GraphMode::InMemory);
        assert_eq!(db.vertex_count(), 2);
        assert_eq!(db.edge_count(),   1);

        let alice = db.get_vertex(1).unwrap();
        assert_eq!(alice["name"], PropertyValue::String("Alice".to_string()));

        let neighbors = db.out_neighbors(1);
        assert_eq!(neighbors, vec![2]);
    }

    #[test]
    fn test_mmap_mode() {
        // 先创建并保存
        let db = make_test_graph();
        // 生成 mmap 文件
        db.generate_mmap_file("/tmp/test_graph_db.bin.mmap").unwrap();

        // 用 Mmap 模式打开
        let db = GraphDB::open("/tmp/test_graph_db.bin", GraphMode::Mmap).unwrap();

        assert_eq!(db.mode(), GraphMode::Mmap);
        assert_eq!(db.vertex_count(), 2);
        assert_eq!(db.edge_count(),   1);

        let alice = db.get_vertex(1).unwrap();
        assert_eq!(alice["name"], PropertyValue::String("Alice".to_string()));

        let neighbors = db.out_neighbors(1);
        assert_eq!(neighbors, vec![2]);

        // 清理
        let _ = std::fs::remove_file("/tmp/test_graph_db.bin.mmap");
    }

    #[test]
    fn test_mmap_mode_read_only() {
        // 先准备 mmap 文件
        let db = make_test_graph();
        db.generate_mmap_file("/tmp/test_graph_db.bin.mmap").unwrap();

        let mut db = GraphDB::open("/tmp/test_graph_db.bin", GraphMode::Mmap).unwrap();

        // 写操作应该返回错误
        let result = db.add_vertex(3, HashMap::new());
        assert!(result.is_err());

        let result = db.add_edge(1, 3, 1.0, HashMap::new());
        assert!(result.is_err());

        let result = db.save();
        assert!(result.is_err());

        let _ = std::fs::remove_file("/tmp/test_graph_db.bin.mmap");
    }

    #[test]
    fn test_to_csr_both_modes() {
        // InMemory 模式
        let db = make_test_graph();
        let csr = db.to_csr();
        assert_eq!(csr.vertex_count, 2);
        assert_eq!(csr.total_edges,  1);

        // Mmap 模式
        let db = make_test_graph();
        db.generate_mmap_file("/tmp/test_graph_db.bin.mmap").unwrap();
        let db = GraphDB::open("/tmp/test_graph_db.bin", GraphMode::Mmap).unwrap();
        let csr = db.to_csr();
        assert_eq!(csr.vertex_count, 2);
        assert_eq!(csr.total_edges,  1);

        let _ = std::fs::remove_file("/tmp/test_graph_db.bin.mmap");
    }

    #[test]
    fn test_iter_both_modes() {
        // InMemory 模式
        let db = make_test_graph();
        let vertices: Vec<_> = db.iter_vertices().collect();
        assert_eq!(vertices.len(), 2);

        // Mmap 模式
        let db = make_test_graph();
        db.generate_mmap_file("/tmp/test_graph_db.bin.mmap").unwrap();
        let db = GraphDB::open("/tmp/test_graph_db.bin", GraphMode::Mmap).unwrap();
        let vertices: Vec<_> = db.iter_vertices().collect();
        assert_eq!(vertices.len(), 2);

        let _ = std::fs::remove_file("/tmp/test_graph_db.bin.mmap");
    }
}
