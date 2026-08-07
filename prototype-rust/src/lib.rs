// src/lib.rs
// Axolotl-RS: 高性能图数据库库

#![allow(ambiguous_glob_reexports)]
// Allow pyo3 SIGNATURE deprecated when python-bindings is enabled
#[cfg_attr(feature = "python-bindings", allow(deprecated))]

use thiserror::Error;

pub mod graph;
pub mod graph_db; // 统一图数据库接口（内存模式 / mmap 模式切换）
pub mod algorithms;
pub mod persistence;
pub mod query; // Fluent Builder 查询 API
pub mod transaction; // 事务支持（WAL + 回滚 + 快照隔离）
pub mod mvcc; // MVCC 快照隔离（Copy-on-Write）
pub mod index; // 哈希索引（精确匹配）
pub mod compact_storage; // 紧凑存储（减少内存占用 2-3x）
pub mod mmap_graph; // mmap 友好格式（支持大于内存的图）
// pub mod visualization; // 暂时禁用
pub mod edge_block; // EdgeBlock 数据结构（CPU 版本，优化缓存利用率）
pub mod csr_graph; // CSR 格式图（用于 GPU 加速）
pub mod gpu_edge_block; // GPU EdgeBlock 格式（扁平化数组，适配 GPU）
pub mod incremental_pagerank; // CPU/GPU 协同的增量 PageRank
pub mod incremental_bfs; // CPU/GPU 协同的增量 BFS
pub mod incremental_sssp; // CPU/GPU 协同的增量 SSSP
pub mod incremental_ssspv2; // 增量 SSSP v2（差分 BFS，纯 CPU）
pub mod incremental_sssp_edgeblock; // CPU/GPU 协同的增量 SSSP（使用 EdgeBlock 格式）
pub mod pagerank_correct; // PageRank CPU 正确实现
pub mod incremental_cc; // 增量 Connected Components（使用 Union-Find）
pub mod incremental_tc; // 增量 Triangle Counting（三角形计数）
pub mod server; // REST API 网络层
pub mod recovery; // Crash Recovery（WAL 重放）
#[cfg(feature = "python-bindings")]
pub mod py_bindings; // Python bindings (PyO3)

// GPU 模块（仅在 macOS 上编译）
#[cfg(target_os = "macos")]
pub mod gpu; // GPU 加速模块（使用 Metal）

pub mod wave_core; // Wave Core 分解 — CPU/GPU 协同的批量剥皮
pub mod scc; // Tarjan 强连通分量 — 有向图 SCC 分解
pub mod betweenness; // Brandes 介数中心性 — 采样近似, EdgeBlock BFS
pub mod louvain; // Louvain 社群检测 — 模块度优化, 分层折叠
pub use graph::GraphDB; // 旧版本
pub use algorithms::*;
pub use persistence::*;
// pub use visualization::*;
pub use edge_block::*; // 导出 EdgeBlock
pub use csr_graph::*; // 导出 CSRGraph
pub use incremental_pagerank::IncrementalPageRankEdgeBlock; // 导出增量 PageRank (EdgeBlock)
pub use incremental_bfs::IncrementalBFS; // 导出增量 BFS
pub use incremental_sssp::IncrementalSSSP; // 导出增量 SSSP
pub use incremental_sssp_edgeblock::IncrementalSsspEdgeBlock; // 导出增量 SSSP (EdgeBlock)
pub use incremental_cc::IncrementalCC; // 导出增量 Connected ComponentsP

// 导出 GPU 模块（仅在 macOS 上）
#[cfg(target_os = "macos")]
pub use gpu::*;

/// 图数据库错误类型
#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Vertex {0} not found")]
    VertexNotFound(u64),
    
    #[error("Edge ({0}, {1}) not found")]
    EdgeNotFound(u64, u64),
    
    #[error("Vertex {0} already exists")]
    VertexAlreadyExists(u64),
    
    #[error("Edge ({0}, {1}) already exists")]
    EdgeAlreadyExists(u64, u64),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// 属性值类型（支持多种数据类型）
///
/// Double 使用 f64::to_bits() 转成 u64 来实现 Hash + Eq
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum PropertyValue {
    Int(i64),
    String(String),
    Double(f64),
    Bool(bool),
    Null,
}

// ── Hash + Eq 手动实现（处理 f64）────────────────────

impl PartialEq for PropertyValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PropertyValue::Int(a), PropertyValue::Int(b)) => a == b,
            (PropertyValue::String(a), PropertyValue::String(b)) => a == b,
            (PropertyValue::Double(a), PropertyValue::Double(b)) => a.to_bits() == b.to_bits(),
            (PropertyValue::Bool(a), PropertyValue::Bool(b)) => a == b,
            (PropertyValue::Null, PropertyValue::Null) => true,
            _ => false,
        }
    }
}

impl Eq for PropertyValue {}

use std::hash::{Hash, Hasher};

impl Hash for PropertyValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            PropertyValue::Int(i) => { 0u8.hash(state); i.hash(state); }
            PropertyValue::String(s) => { 1u8.hash(state); s.hash(state); }
            PropertyValue::Double(d) => { 2u8.hash(state); d.to_bits().hash(state); }
            PropertyValue::Bool(b) => { 3u8.hash(state); b.hash(state); }
            PropertyValue::Null => { 4u8.hash(state); }
        }
    }
}

impl PropertyValue {
    /// 转换为 JSON 字符串
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "null".to_string())
    }
}

/// 顶点 ID 类型
pub type VertexId = u64;

/// 边 ID（元组）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EdgeId {
    pub from: VertexId,
    pub to: VertexId,
}

impl EdgeId {
    pub fn new(from: VertexId, to: VertexId) -> Self {
        Self { from, to }
    }
}

/// 顶点数据结构
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vertex {
    pub id: VertexId,
    pub properties: std::collections::HashMap<String, PropertyValue>,
}

/// 边数据结构
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub properties: std::collections::HashMap<String, PropertyValue>,
    pub weight: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_property_value() {
        let pv = PropertyValue::String("test".to_string());
        assert_eq!(pv.to_json(), "\"test\"");
    }
    
    #[test]
    fn test_edge_id() {
        let edge_id = EdgeId::new(1, 2);
        assert_eq!(edge_id.from, 1);
        assert_eq!(edge_id.to, 2);
    }
}
