// src/lib.rs
// Axolotl-RS: 高性能图数据库库

pub mod graph;
pub mod algorithms;
pub mod persistence;
// pub mod visualization; // 暂时禁用
pub mod edge_block; // EdgeBlock 数据结构（优化缓存利用率）
pub mod csr_graph; // CSR 格式图（用于对比）
pub mod incremental_pagerank; // 正确的增量 PageRank 实现（CPU/GPU 协同）

pub use graph::GraphDB;
pub use algorithms::*;
pub use persistence::*;
// pub use visualization::*;
pub use edge_block::*; // 导出 EdgeBlock
pub use csr_graph::*; // 导出 CSRGraph
pub use incremental_pagerank::IncrementalPageRank; // 导出正确的增量 PageRank

use thiserror::Error;

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
