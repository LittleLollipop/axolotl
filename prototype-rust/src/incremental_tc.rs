// src/incremental_tc.rs
// 增量 Triangle Counting（三角形计数）
// 
// 翻译自：/tmp/axolotl_tmp/Experiments/incremental_triangle_counting.swift

use std::collections::HashSet;

/// 使用邻接集合的图（用于 Triangle Counting）
pub struct GraphWithAdjacencySets {
    vertex_count: usize,
    adjacency_sets: Vec<HashSet<u32>>,
}

impl GraphWithAdjacencySets {
    /// 创建新的图
    pub fn new(vertex_count: usize) -> Self {
        let mut adjacency_sets = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            adjacency_sets.push(HashSet::new());
        }
        
        GraphWithAdjacencySets {
            vertex_count,
            adjacency_sets,
        }
    }
    
    /// 添加无向边
    pub fn add_edge(&mut self, u: u32, v: u32) {
        if u != v && !self.adjacency_sets[u as usize].contains(&v) {
            self.adjacency_sets[u as usize].insert(v);
            self.adjacency_sets[v as usize].insert(u);
        }
    }
    
    /// 检查边是否存在
    pub fn has_edge(&self, u: u32, v: u32) -> bool {
        self.adjacency_sets[u as usize].contains(&v)
    }
    
    /// 获取顶点的邻居
    pub fn neighbors(&self, v: u32) -> &HashSet<u32> {
        &self.adjacency_sets[v as usize]
    }
    
    /// 生成随机图（Erdős-Rényi 模型）
    pub fn generate(vertex_count: usize, edge_count: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut graph = GraphWithAdjacencySets::new(vertex_count);
        let mut attempts = 0;
        
        while graph.edge_count() < edge_count && attempts < edge_count * 2 {
            let u = rng.gen_range(0..vertex_count) as u32;
            let v = rng.gen_range(0..vertex_count) as u32;
            if u != v {
                graph.add_edge(u, v);
            }
            attempts += 1;
        }
        
        graph
    }
    
    /// 获取边数量
    pub fn edge_count(&self) -> usize {
        let mut count = 0;
        for adj in &self.adjacency_sets {
            count += adj.len();
        }
        count / 2
    }
    
    /// 添加新边（用于增量更新）
    pub fn add_new_edges(&mut self, count: usize) -> Vec<(u32, u32)> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut new_edges = Vec::new();
        let mut attempts = 0;
        
        while new_edges.len() < count && attempts < count * 2 {
            let u = rng.gen_range(0..self.vertex_count) as u32;
            let v = rng.gen_range(0..self.vertex_count) as u32;
            if u != v && !self.has_edge(u, v) {
                self.add_edge(u, v);
                new_edges.push((u.min(v), v.max(v)));
            }
            attempts += 1;
        }
        
        new_edges
    }
}

/// 全量 Triangle Counting（返回三角形集合）
pub fn full_triangle_counting_set(graph: &GraphWithAdjacencySets) -> (HashSet<[u32; 3]>, f64) {
    let start = std::time::Instant::now();
    let mut triangles = HashSet::new();
    
    // 遍历所有边 (u, v) 其中 u < v
    for u in 0..graph.vertex_count {
        for &v in &graph.adjacency_sets[u] {
            if v > u as u32 {
                // 找出 u 和 v 的公共邻居
                let neighbors_u = &graph.adjacency_sets[u];
                let neighbors_v = &graph.adjacency_sets[v as usize];
                
                // 求交集
                let common_neighbors = neighbors_u.intersection(neighbors_v);
                
                // 每个公共邻居 w 形成一个三角形 (u, v, w)
                for &w in common_neighbors {
                    // 存储为排序数组（避免重复计数）
                    let mut triangle = [u as u32, v, w];
                    triangle.sort();
                    triangles.insert(triangle);
                }
            }
        }
    }
    
    let elapsed = start.elapsed();
    (triangles, elapsed.as_secs_f64())
}

/// 全量 Triangle Counting（只返回计数）
pub fn full_triangle_counting(graph: &GraphWithAdjacencySets) -> (usize, f64) {
    let (triangles, time) = full_triangle_counting_set(graph);
    (triangles.len(), time)
}

/// 增量 Triangle Counting（只处理新边）
pub fn incremental_triangle_counting(
    graph: &GraphWithAdjacencySets,
    new_edges: &[(u32, u32)],
) -> (usize, f64) {
    let start = std::time::Instant::now();
    
    // 使用 HashSet 存储唯一的三角形
    let mut new_triangles = HashSet::new();
    
    // 只处理新边
    for &(u, v) in new_edges {
        // 找出 u 和 v 的公共邻居
        let neighbors_u = graph.neighbors(u);
        let neighbors_v = graph.neighbors(v);
        
        // 求交集
        let common_neighbors = neighbors_u.intersection(neighbors_v);
        
        // 每个公共邻居 w 形成一个三角形 (u, v, w)
        for &w in common_neighbors {
            // 存储为排序数组（避免重复计数）
            let mut triangle = [u, v, w];
            triangle.sort();
            new_triangles.insert(triangle);
        }
    }
    
    let elapsed = start.elapsed();
    (new_triangles.len(), elapsed.as_secs_f64())
}

/// 增量 Triangle Counting（正确但较慢的方法：计算前后差值）
pub fn incremental_triangle_counting_correct(
    graph: &GraphWithAdjacencySets,
    _new_edges: &[(u32, u32)],
    triangles_before: &HashSet<[u32; 3]>,
) -> (usize, f64) {
    let start = std::time::Instant::now();
    
    // 计算添加新边后的三角形
    let (triangles_after, _) = full_triangle_counting_set(graph);
    
    // 新三角形 = 添加后 - 添加前
    let new_triangles: HashSet<_> = triangles_after.difference(triangles_before).collect();
    
    let elapsed = start.elapsed();
    (new_triangles.len(), elapsed.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_triangle_counting() {
        // 创建图
        let mut graph = GraphWithAdjacencySets::new(5);
        
        // 添加边：0-1, 1-2, 2-0（形成一个三角形）
        graph.add_edge(0, 1);
        graph.add_edge(1, 2);
        graph.add_edge(2, 0);
        
        // 全量计数
        let (count, _) = full_triangle_counting(&graph);
        
        // 应该有 1 个三角形
        assert_eq!(count, 1);
        
        println!("✅ Triangle Counting 测试通过！");
    }
    
    #[test]
    fn test_incremental_triangle_counting() {
        // 创建图
        let mut graph = GraphWithAdjacencySets::new(5);
        
        // 添加边：0-1, 1-2（还没有三角形）
        graph.add_edge(0, 1);
        graph.add_edge(1, 2);
        
        // 全量计数
        let (triangles_before, _) = full_triangle_counting_set(&graph);
        assert_eq!(triangles_before.len(), 0);
        
        // 添加边：2-0（形成三角形）
        let new_edges = vec![(2, 0)];
        graph.add_edge(2, 0);
        
        // 增量计数
        let (new_count, _) = incremental_triangle_counting(&graph, &new_edges);
        
        // 应该有 1 个新三角形
        assert_eq!(new_count, 1);
        
        println!("✅ 增量 Triangle Counting 测试通过！");
    }
}
