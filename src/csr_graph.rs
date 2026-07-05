// src/csr_graph.rs
// CSR (Compressed Sparse Row) 格式的图（用于对比）

use std::collections::{HashMap, VecDeque};

/// 使用 CSR 格式的图
#[derive(Debug)]
pub struct CSRGraph {
    /// 顶点列表
    pub vertices: HashMap<u64, VertexData>,
    /// CSR 格式的偏移数组（vertex_id -> 第一条边在 edges 中的索引）
    pub offsets: Vec<usize>,
    /// CSR 格式的边数组（所有顶点的边连续存储）
    pub edges: Vec<u64>,
    /// 顶点 ID 到索引的映射
    pub vertex_to_idx: HashMap<u64, usize>,
    /// 索引到顶点 ID 的映射
    pub idx_to_vertex: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct VertexData {
    pub id: u64,
    pub properties: HashMap<String, PropertyValue>,
}

/// 属性值（简化版）
#[derive(Debug, Clone)]
pub enum PropertyValue {
    Int(i64),
    String(String),
    Bool(bool),
}

impl CSRGraph {
    /// 创建新的图
    pub fn new() -> Self {
        CSRGraph {
            vertices: HashMap::new(),
            offsets: Vec::new(),
            edges: Vec::new(),
            vertex_to_idx: HashMap::new(),
            idx_to_vertex: Vec::new(),
        }
    }
    
    /// 添加顶点
    pub fn add_vertex(&mut self, id: u64, properties: HashMap<String, PropertyValue>) {
        let vertex_data = VertexData { id, properties };
        self.vertices.insert(id, vertex_data);
    }
    
    /// 构建 CSR 格式（必须在添加所有边之后调用）
    pub fn build_csr(&mut self, edges: &[(u64, u64)]) {
        // 收集所有顶点 ID 并排序
        let mut vertex_ids: Vec<u64> = self.vertices.keys().cloned().collect();
        vertex_ids.sort();
        
        // 创建映射
        self.vertex_to_idx.clear();
        self.idx_to_vertex.clear();
        
        for (idx, &vertex_id) in vertex_ids.iter().enumerate() {
            self.vertex_to_idx.insert(vertex_id, idx);
            self.idx_to_vertex.push(vertex_id);
        }
        
        let n = vertex_ids.len();
        
        // 统计每个顶点的边数
        let mut edge_counts = vec![0; n];
        for &(from, _) in edges {
            if let Some(&idx) = self.vertex_to_idx.get(&from) {
                edge_counts[idx] += 1;
            }
        }
        
        // 构建偏移数组
        self.offsets = vec![0; n + 1];
        for i in 0..n {
            self.offsets[i + 1] = self.offsets[i] + edge_counts[i];
        }
        
        // 填充边数组
        let mut current_offset: Vec<usize> = self.offsets[0..n].to_vec(); // 修复：正确初始化
        self.edges = vec![0; self.offsets[n]];
        
        for &(from, to) in edges {
            if let Some(&idx) = self.vertex_to_idx.get(&from) {
                let offset = current_offset[idx];
                self.edges[offset] = to;
                current_offset[idx] += 1;
            }
        }
    }
    
    /// 获取顶点的所有邻居（使用 CSR 格式）
    pub fn get_neighbors(&self, vertex_id: u64) -> Vec<u64> {
        if let Some(&idx) = self.vertex_to_idx.get(&vertex_id) {
            let start = self.offsets[idx];
            let end = self.offsets[idx + 1];
            self.edges[start..end].to_vec()
        } else {
            Vec::new()
        }
    }
    
    /// BFS 使用 CSR 格式
    pub fn bfs(&self, start: u64) -> HashMap<u64, i32> {
        let mut distance = HashMap::new();
        let mut queue = VecDeque::new();
        
        distance.insert(start, 0);
        queue.push_back(start);
        
        while let Some(current) = queue.pop_front() {
            let current_dist = distance[&current];
            
            // 使用 CSR 获取邻居
            for &neighbor in &self.get_neighbors(current) {
                if !distance.contains_key(&neighbor) {
                    distance.insert(neighbor, current_dist + 1);
                    queue.push_back(neighbor);
                }
            }
        }
        
        distance
    }
    
    /// 统计信息
    pub fn stats(&self) -> (usize, usize, usize) {
        let n_vertices = self.vertices.len();
        let n_edges = self.edges.len();
        let n_offsets = self.offsets.len();
        
        (n_vertices, n_offsets, n_edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    
    #[test]
    fn test_csr_graph() {
        let mut graph = CSRGraph::new();
        
        // 添加顶点
        graph.add_vertex(1, HashMap::new());
        graph.add_vertex(2, HashMap::new());
        graph.add_vertex(3, HashMap::new());
        
        // 添加边
        let edges = vec![
            (1, 2),
            (1, 3),
            (2, 3),
        ];
        
        graph.build_csr(&edges);
        
        // 测试邻居获取
        let neighbors = graph.get_neighbors(1);
        assert_eq!(neighbors.len(), 2);
        assert!(neighbors.contains(&2));
        assert!(neighbors.contains(&3));
        
        // 测试 BFS
        let dist = graph.bfs(1);
        assert_eq!(dist[&1], 0);
        assert_eq!(dist[&2], 1);
        assert_eq!(dist[&3], 1);
    }
    
    #[test]
    fn test_large_graph() {
        let mut graph = CSRGraph::new();
        
        // 创建大型图（1000 顶点，5000 边）
        for i in 0..1000 {
            graph.add_vertex(i, HashMap::new());
        }
        
        let mut edges = Vec::new();
        let mut rng = rand::thread_rng();
        let mut added_edges = std::collections::HashSet::new();
        
        while added_edges.len() < 5000 {
            let u = rng.gen_range(0..1000);
            let v = rng.gen_range(0..1000);
            
            if u != v && !added_edges.contains(&(u, v)) {
                added_edges.insert((u, v));
                edges.push((u, v));
            }
        }
        
        // 构建 CSR
        graph.build_csr(&edges);
        
        // 测试 BFS 性能
        let start = std::time::Instant::now();
        let dist = graph.bfs(0);
        let elapsed = start.elapsed();
        
        println!("BFS (CSR) 耗时: {:?}", elapsed);
        println!("访问了 {} 个顶点", dist.len());
        
        // 验证正确性
        assert!(dist.contains_key(&0));
        assert_eq!(dist[&0], 0);
    }
}
