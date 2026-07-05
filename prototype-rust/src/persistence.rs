// src/persistence.rs
// Axolotl-RS: 持久化（JSON + 二进制）

use super::*;
use petgraph::graph::Graph;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// 持久化管理器
pub trait PersistenceManager {
    /// 保存到 JSON 文件
    fn save_json(&self, path: &Path) -> Result<(), GraphError>;
    
    /// 从 JSON 文件加载
    fn load_json(path: &Path) -> Result<Self, GraphError>
    where
        Self: Sized;
    
    /// 保存到二进制文件
    fn save_binary(&self, path: &Path) -> Result<(), GraphError>;
    
    /// 从二进制文件加载
    fn load_binary(path: &Path) -> Result<Self, GraphError>
    where
        Self: Sized;
}

impl PersistenceManager for GraphDB {
    /// 保存到 JSON 文件
    fn save_json(&self, path: &Path) -> Result<(), GraphError> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        
        serde_json::to_writer(writer, self)?;
        
        Ok(())
    }
    
    /// 从 JSON 文件加载
    fn load_json(path: &Path) -> Result<Self, GraphError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        
        let mut db: Self = serde_json::from_reader(reader)?;
        
        // 重建 petgraph 和映射
        db.rebuild_graph()?;
        
        Ok(db)
    }
    
    /// 保存到二进制文件（简化版：使用 bincode）
    fn save_binary(&self, path: &Path) -> Result<(), GraphError> {
        // 注意：这里使用 JSON 作为后备（bincode 需要额外依赖）
        // 生产环境应该使用真正的二进制格式
        self.save_json(path)
    }
    
    /// 从二进制文件加载
    fn load_binary(path: &Path) -> Result<Self, GraphError> {
        // 注意：这里使用 JSON 作为后备
        Self::load_json(path)
    }
}

impl GraphDB {
    /// 重建 petgraph 和映射（从 JSON 加载后调用）
    fn rebuild_graph(&mut self) -> Result<(), GraphError> {
        // 清空现有图
        self.graph = Graph::new_undirected();
        self.vertex_map.clear();
        self.reverse_map.clear();
        
        // 重新添加顶点
        for (vertex_id, _) in &self.vertices {
            let node_idx = self.graph.add_node(*vertex_id);
            self.vertex_map.insert(*vertex_id, node_idx);
            self.reverse_map.insert(node_idx, *vertex_id);
        }
        
        // 重新添加边
        for (edge_id, edge) in &self.edges {
            let from_idx = self.vertex_map[&edge_id.from];
            let to_idx = self.vertex_map[&edge_id.to];
            self.graph.add_edge(from_idx, to_idx, (edge.weight, edge.properties.clone()));
        }
        
        Ok(())
    }
    
    /// 导出为 Graphviz DOT 格式
    pub fn export_dot(&self, path: &Path, directed: bool) -> Result<(), GraphError> {
        let mut dot = String::new();
        
        // DOT 文件头
        if directed {
            dot.push_str("digraph G {\n");
        } else {
            dot.push_str("graph G {\n");
        }
        dot.push_str("  node [shape=circle, style=filled];\n\n");
        
        // 顶点
        for (vertex_id, vertex) in &self.vertices {
            let label = vertex
                .properties
                .get("name")
                .map(|v| {
                    if let PropertyValue::String(s) = v {
                        s.clone()
                    } else {
                        vertex_id.to_string()
                    }
                })
                .unwrap_or_else(|| vertex_id.to_string());
            
            dot.push_str(&format!("  {} [label=\"{}\"];\n", vertex_id, label));
        }
        
        dot.push_str("\n");
        
        // 边
        for (edge_id, edge) in &self.edges {
            if directed {
                dot.push_str(&format!(
                    "  {} -> {} [weight={}];\n",
                    edge_id.from, edge_id.to, edge.weight
                ));
            } else {
                dot.push_str(&format!(
                    "  {} -- {} [weight={}];\n",
                    edge_id.from, edge_id.to, edge.weight
                ));
            }
        }
        
        dot.push_str("}\n");
        
        // 写入文件
        std::fs::write(path, dot)?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    
    #[test]
    fn test_save_load_json() {
        let mut db = GraphDB::new();
        
        // 添加数据
        let v1 = db.add_vertex(HashMap::new()).unwrap();
        let v2 = db.add_vertex(HashMap::new()).unwrap();
        db.add_edge(v1, v2, HashMap::new(), 1.0).unwrap();
        
        // 保存到临时文件
        let path = PathBuf::from("/tmp/test_graph.json");
        db.save_json(&path).unwrap();
        
        // 从文件加载
        let loaded_db = GraphDB::load_json(&path).unwrap();
        
        assert_eq!(loaded_db.vertex_count(), 2);
        assert_eq!(loaded_db.edge_count(), 1);
        
        // 清理
        std::fs::remove_file(path).ok();
    }
}
