// src/graph.rs
// Axolotl-RS: 核心图数据库实现

use super::*;
use petgraph::graph::{Graph, NodeIndex};
use std::collections::{HashMap, HashSet};

/// 高性能图数据库（支持增量算法）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphDB {
    /// 底层图结构（使用 petgraph）
    #[serde(skip)]
    pub(crate) graph: Graph<VertexId, (f64, HashMap<String, PropertyValue>), petgraph::Undirected>,
    
    /// 顶点 ID 映射（VertexId -> NodeIndex）
    #[serde(skip)]
    pub(crate) vertex_map: HashMap<VertexId, NodeIndex>,
    
    /// 反向映射（NodeIndex -> VertexId）
    #[serde(skip)]
    pub(crate) reverse_map: HashMap<NodeIndex, VertexId>,
    
    /// 顶点属性存储
    pub(crate) vertices: HashMap<VertexId, Vertex>,
    
    /// 边属性存储
    pub(crate) edges: HashMap<EdgeId, Edge>,
    
    /// 邻接表（快速邻居查询）
    pub(crate) adjacency_list: HashMap<VertexId, HashSet<VertexId>>,
    
    /// 属性索引（property_name -> (value -> vertex_ids)）
    pub(crate) property_index: HashMap<String, HashMap<PropertyValue, HashSet<VertexId>>>,
    
    /// 增量算法缓存（PageRank 等）
    #[serde(skip)]
    pub(crate) pagerank_cache: Option<HashMap<VertexId, f64>>,
    
    /// 图修改计数器（用于增量更新）
    #[serde(skip)]
    pub(crate) modification_count: u64,
}

impl GraphDB {
    /// 创建新图数据库
    pub fn new() -> Self {
        Self {
            graph: Graph::new_undirected(),
            vertex_map: HashMap::new(),
            reverse_map: HashMap::new(),
            vertices: HashMap::new(),
            edges: HashMap::new(),
            adjacency_list: HashMap::new(),
            property_index: HashMap::new(),
            pagerank_cache: None,
            modification_count: 0,
        }
    }
    
    /// 添加顶点
    pub fn add_vertex(&mut self, properties: HashMap<String, PropertyValue>) -> Result<VertexId, GraphError> {
        // 生成顶点 ID
        let vertex_id = self.modification_count;
        self.modification_count += 1;
        
        // 添加到 petgraph
        let node_idx = self.graph.add_node(vertex_id);
        self.vertex_map.insert(vertex_id, node_idx);
        self.reverse_map.insert(node_idx, vertex_id);
        
        // 创建顶点对象
        let vertex = Vertex {
            id: vertex_id,
            properties,
        };
        
        // 存储顶点
        self.vertices.insert(vertex_id, vertex.clone());
        
        // 初始化邻接表
        self.adjacency_list.insert(vertex_id, HashSet::new());
        
        // 更新属性索引
        self.update_property_index(&vertex);
        
        // 清除缓存（图已修改）
        self.invalidate_cache();
        
        Ok(vertex_id)
    }
    
    /// 添加顶点（指定 ID）
    pub fn add_vertex_with_id(&mut self, id: VertexId, properties: HashMap<String, PropertyValue>) -> Result<(), GraphError> {
        // 检查顶点是否已存在
        if self.vertices.contains_key(&id) {
            return Err(GraphError::VertexAlreadyExists(id));
        }
        
        // 添加到 petgraph
        let node_idx = self.graph.add_node(id);
        self.vertex_map.insert(id, node_idx);
        self.reverse_map.insert(node_idx, id);
        
        // 创建顶点对象
        let vertex = Vertex {
            id,
            properties,
        };
        
        // 存储顶点
        self.vertices.insert(id, vertex.clone());
        
        // 初始化邻接表
        self.adjacency_list.insert(id, HashSet::new());
        
        // 更新属性索引
        self.update_property_index(&vertex);
        
        // 清除缓存（图已修改）
        self.invalidate_cache();
        
        Ok(())
    }
    
    /// 添加边
    pub fn add_edge(
        &mut self,
        from: VertexId,
        to: VertexId,
        properties: HashMap<String, PropertyValue>,
        weight: f64,
    ) -> Result<EdgeId, GraphError> {
        // 检查顶点是否存在
        if !self.vertices.contains_key(&from) {
            return Err(GraphError::VertexNotFound(from));
        }
        if !self.vertices.contains_key(&to) {
            return Err(GraphError::VertexNotFound(to));
        }
        
        // 检查边是否已存在
        let edge_id = EdgeId::new(from, to);
        if self.edges.contains_key(&edge_id) {
            return Err(GraphError::EdgeAlreadyExists(from, to));
        }
        
        // 添加到 petgraph
        let from_idx = self.vertex_map[&from];
        let to_idx = self.vertex_map[&to];
        self.graph.add_edge(from_idx, to_idx, (weight, properties.clone()));
        
        // 创建边对象
        let edge = Edge {
            id: edge_id,
            properties,
            weight,
        };
        
        // 存储边
        self.edges.insert(edge_id, edge);
        
        // 更新邻接表
        self.adjacency_list.get_mut(&from).unwrap().insert(to);
        self.adjacency_list.get_mut(&to).unwrap().insert(from);
        
        // 清除缓存
        self.invalidate_cache();
        
        Ok(edge_id)
    }
    
    /// 获取顶点
    pub fn get_vertex(&self, id: VertexId) -> Option<&Vertex> {
        self.vertices.get(&id)
    }
    
    /// 获取边
    pub fn get_edge(&self, from: VertexId, to: VertexId) -> Option<&Edge> {
        let edge_id = EdgeId::new(from, to);
        self.edges.get(&edge_id)
    }
    
    /// 获取邻居
    pub fn get_neighbors(&self, vertex_id: VertexId) -> HashSet<VertexId> {
        self.adjacency_list
            .get(&vertex_id)
            .cloned()
            .unwrap_or_default()
    }
    
    /// 获取顶点数量
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }
    
    /// 获取边数量
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
    
    /// 获取所有顶点 ID
    pub fn vertex_ids(&self) -> Vec<VertexId> {
        self.vertices.keys().cloned().collect()
    }
    
    /// 获取 PageRank 缓存（用于增量更新）
    pub fn get_pagerank_cache(&self) -> Option<&HashMap<VertexId, f64>> {
        self.pagerank_cache.as_ref()
    }
    
    /// 设置 PageRank 缓存
    pub fn set_pagerank_cache(&mut self, cache: HashMap<VertexId, f64>) {
        self.pagerank_cache = Some(cache);
    }
    
    /// 更新属性索引
    fn update_property_index(&mut self, vertex: &Vertex) {
        for (key, value) in &vertex.properties {
            self.property_index
                .entry(key.clone())
                .or_insert_with(HashMap::new)
                .entry(value.clone())
                .or_insert_with(HashSet::new)
                .insert(vertex.id);
        }
    }
    
    /// 清除缓存（图被修改时调用）
    fn invalidate_cache(&mut self) {
        self.pagerank_cache = None;
    }
    
    /// 获取图修改计数器（用于增量算法）
    pub fn get_modification_count(&self) -> u64 {
        self.modification_count
    }
}

impl Default for GraphDB {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add_vertex() {
        let mut db = GraphDB::new();
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        
        let vertex_id = db.add_vertex(props).unwrap();
        assert_eq!(db.vertex_count(), 1);
        assert!(db.get_vertex(vertex_id).is_some());
    }
    
    #[test]
    fn test_add_edge() {
        let mut db = GraphDB::new();
        
        let mut props1 = HashMap::new();
        props1.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        let v1 = db.add_vertex(props1).unwrap();
        
        let mut props2 = HashMap::new();
        props2.insert("name".to_string(), PropertyValue::String("Bob".to_string()));
        let v2 = db.add_vertex(props2).unwrap();
        
        let edge_id = db.add_edge(v1, v2, HashMap::new(), 1.0).unwrap();
        assert_eq!(db.edge_count(), 1);
        assert!(db.get_edge(v1, v2).is_some());
        assert_eq!(db.get_neighbors(v1).len(), 1);
    }
}
