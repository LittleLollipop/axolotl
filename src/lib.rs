// src/lib.rs
// Axolotl-RS: 高性能图数据库库

use thiserror::Error;

pub mod graph;
pub mod algorithms;
pub mod persistence;
// pub mod visualization; // 暂时禁用
pub mod edge_block; // EdgeBlock 数据结构（优化缓存利用率）
pub mod csr_graph; // CSR 格式图（用于 GPU 加速）
pub mod incremental_pagerank; // CPU/GPU 协同的增量 PageRank
pub mod incremental_bfs; // CPU/GPU 协同的增量 BFS

// GPU 模块（仅在 macOS 上编译）
#[cfg(target_os = "macos")]
pub mod gpu; // GPU 加速模块（使用 Metal）

pub use graph::GraphDB;
pub use algorithms::*;
pub use persistence::*;
// pub use visualization::*;
pub use edge_block::*; // 导出 EdgeBlock
pub use csr_graph::*; // 导出 CSRGraph
pub use incremental_pagerank::IncrementalPageRank; // 导出增量 PageRank
pub use incremental_bfs::IncrementalBFS; // 导出增量 BFS

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
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum PropertyValue {
    Int(i64),
    String(String),
    Bool(bool),
    Null,
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
