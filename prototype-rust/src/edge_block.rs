// src/edge_block.rs
// EdgeBlock 数据结构实现（优化 CPU 缓存利用率）

use std::collections::{HashMap, VecDeque};
use crate::PropertyValue;

/// EdgeBlock 数据结构
/// 将边分组为固定大小的块（block），优化缓存访问
#[derive(Debug, Clone)]
pub struct EdgeBlock {
    /// 所属顶点 ID
    pub owner_vertex: u64,
    /// 实际边数（≤ BLOCK_SIZE）
    pub edge_count: usize,
    /// 邻接顶点数组（不足时用 0 填充，0 表示无效）
    pub edges: Vec<u64>,
}

impl EdgeBlock {
    /// Block 大小（一个 cache line 的大小 = 32 个 u64）
    pub const BLOCK_SIZE: usize = 32;
    
    /// 创建新的 EdgeBlock
    pub fn new(owner_vertex: u64) -> Self {
        EdgeBlock {
            owner_vertex,
            edge_count: 0,
            edges: vec![0; Self::BLOCK_SIZE],
        }
    }
    
    /// 添加一条边
    pub fn add_edge(&mut self, neighbor: u64) -> bool {
        if self.edge_count >= Self::BLOCK_SIZE {
            return false; // Block 已满
        }
        
        self.edges[self.edge_count] = neighbor;
        self.edge_count += 1;
        true
    }
    
    /// 获取所有有效边
    pub fn get_edges(&self) -> &[u64] {
        &self.edges[0..self.edge_count]
    }
}

/// 使用 EdgeBlock 格式的图
#[derive(Debug)]
pub struct EdgeBlockGraph {
    /// 顶点列表
    pub vertices: HashMap<u64, VertexData>,
    /// EdgeBlock 列表（所有 block 连续存储）
    pub edge_blocks: Vec<EdgeBlock>,
    /// 顶点 ID -> 第一个 block 的索引
    pub vertex_to_block: HashMap<u64, usize>,
}

#[derive(Debug, Clone)]
pub struct VertexData {
    pub id: u64,
    pub properties: HashMap<String, PropertyValue>,
}

impl EdgeBlockGraph {
    /// 创建新的图
    pub fn new() -> Self {
        EdgeBlockGraph {
            vertices: HashMap::new(),
            edge_blocks: Vec::new(),
            vertex_to_block: HashMap::new(),
        }
    }
    
    /// 添加顶点
    pub fn add_vertex(&mut self, id: u64, properties: HashMap<String, PropertyValue>) {
        let vertex_data = VertexData { id, properties };
        self.vertices.insert(id, vertex_data);
    }
    
    /// 添加边（自动构建 EdgeBlock）
    pub fn add_edge(&mut self, from: u64, to: u64, weight: f64) {
        // 确保顶点存在
        if !self.vertices.contains_key(&from) {
            self.add_vertex(from, HashMap::new());
        }
        if !self.vertices.contains_key(&to) {
            self.add_vertex(to, HashMap::new());
        }
        
        // 获取 or 创建 from 顶点的 block
        let block_idx = if let Some(&idx) = self.vertex_to_block.get(&from) {
            idx
        } else {
            // 创建新的 block
            let idx = self.edge_blocks.len();
            self.vertex_to_block.insert(from, idx);
            self.edge_blocks.push(EdgeBlock::new(from));
            idx
        };
        
        // 尝试添加边到 block
        if !self.edge_blocks[block_idx].add_edge(to) {
            // Block 已满，创建新的 block
            let new_block_idx = self.edge_blocks.len();
            self.vertex_to_block.insert(from, new_block_idx);
            let mut new_block = EdgeBlock::new(from);
            new_block.add_edge(to);
            self.edge_blocks.push(new_block);
        }
    }
    
    /// 获取顶点的所有邻居（使用 EdgeBlock）
    pub fn get_neighbors(&self, vertex_id: u64) -> Vec<u64> {
        let mut neighbors = Vec::new();
        
        // 找到顶点的第一个 block
        if let Some(&block_idx) = self.vertex_to_block.get(&vertex_id) {
            // 遍历所有属于该顶点的 block
            let mut current_idx = block_idx;
            while current_idx < self.edge_blocks.len() {
                let block = &self.edge_blocks[current_idx];
                if block.owner_vertex != vertex_id {
                    break;
                }
                
                // 添加 block 中的所有边
                for &neighbor in block.get_edges() {
                    if neighbor != 0 {
                        neighbors.push(neighbor);
                    }
                }
                
                current_idx += 1;
            }
        }
        
        neighbors
    }
    
    /// BFS 使用 EdgeBlock（优化缓存利用率）
    pub fn bfs(&self, start: u64) -> HashMap<u64, i32> {
        let mut distance = HashMap::new();
        let mut queue = VecDeque::new();
        
        distance.insert(start, 0);
        queue.push_back(start);
        
        while let Some(current) = queue.pop_front() {
            let current_dist = distance[&current];
            
            // 使用 EdgeBlock 获取邻居（缓存友好）
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
        let n_blocks = self.edge_blocks.len();
        
        let mut total_edges = 0;
        for block in &self.edge_blocks {
            total_edges += block.edge_count;
        }
        
        (n_vertices, n_blocks, total_edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng; // 添加 Rng trait
    use std::collections::HashMap;
    
    #[test]
    fn test_edge_block() {
        let mut graph = EdgeBlockGraph::new();
        
        // 添加顶点
        graph.add_vertex(1, HashMap::new());
        graph.add_vertex(2, HashMap::new());
        graph.add_vertex(3, HashMap::new());
        
        // 添加边
        graph.add_edge(1, 2, 1.0);
        graph.add_edge(1, 3, 1.0);
        graph.add_edge(2, 3, 1.0);
        
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
        let mut graph = EdgeBlockGraph::new();
        
        // 创建大型图（1000 顶点，5000 边）
        for i in 0..1000 {
            graph.add_vertex(i, HashMap::new());
        }
        
        let mut rng = rand::thread_rng();
        let mut added_edges = std::collections::HashSet::new();
        
        while added_edges.len() < 5000 {
            let u = rng.gen_range(0..1000);
            let v = rng.gen_range(0..1000);
            
            if u != v && !added_edges.contains(&(u, v)) {
                added_edges.insert((u, v));
                graph.add_edge(u, v, 1.0);
            }
        }
        
        // 测试 BFS 性能
        let start = std::time::Instant::now();
        let dist = graph.bfs(0);
        let elapsed = start.elapsed();
        
        println!("BFS (EdgeBlock) 耗时: {:?}", elapsed);
        println!("访问了 {} 个顶点", dist.len());
        
        // 验证正确性
        assert!(dist.contains_key(&0));
        assert_eq!(dist[&0], 0);
    }
}
