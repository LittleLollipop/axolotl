// src/incremental_pagerank.rs
// CPU/GPU 协同的增量 PageRank 实现（严格按照 Swift 版本）

use std::collections::{HashMap, HashSet};

/// CPU/GPU 协同的增量 PageRank
/// 
/// 核心思想（来自 Swift 版本）：
/// 1. CPU 管理"受影响顶点集合"
/// 2. GPU 计算受影响顶点的 PR 值
/// 3. CPU 检查收敛，动态更新受影响集合
/// 4. 直到受影响集合为空或达到最大迭代次数
pub struct IncrementalPageRank {
    /// 当前 PR 值
    pr: HashMap<u64, f64>,
    /// 阻尼因子
    damping_factor: f64,
    /// 最大迭代次数
    max_iterations: usize,
    /// 收敛阈值
    tolerance: f64,
    /// GPU 加速器（可选，如果没有 GPU 则使用 CPU）
    gpu: Option<crate::gpu::GPUAccelerator>,
}

impl IncrementalPageRank {
    pub fn new() -> Self {
        // 尝试创建 GPU 加速器
        let gpu = crate::gpu::GPUAccelerator::new().ok();
        
        IncrementalPageRank {
            pr: HashMap::new(),
            damping_factor: 0.85,
            max_iterations: 100,
            tolerance: 1e-6,
            gpu,
        }
    }
    
    /// 初始化（首次全量计算，使用 GPU）
    pub fn initialize(&mut self, graph: &crate::graph::GraphDB) {
        println!("初始化增量 PageRank（使用 GPU）...");
        
        let n = graph.vertex_count() as f64;
        let initial = 1.0 / n;
        
        // 初始化所有顶点的 PR 值
        self.pr.clear();
        for &vertex_id in graph.vertices.keys() {
            self.pr.insert(vertex_id, initial);
        }
        
        // 全量幂迭代（使用 GPU）
        let mut iteration = 0;
        
        loop {
            iteration += 1;
            
            // 使用 GPU 计算新的 PR 值
            // TODO: 实现全量 PageRank 的 GPU 内核
            
            // 暂时使用 CPU 计算
            let mut new_pr = HashMap::new();
            
            // 初始化所有顶点
            for &vertex_id in graph.vertices.keys() {
                new_pr.insert(vertex_id, (1.0 - self.damping_factor) / n);
            }
            
            // 传播 PR 值（push 方式）
            for (&vertex_id, &pr_value) in &self.pr {
                if let Some(neighbors) = graph.adjacency_list.get(&vertex_id) {
                    if !neighbors.is_empty() {
                        // 正常情况：贡献分给邻居
                        let contribution = pr_value * self.damping_factor / neighbors.len() as f64;
                        for &neighbor in neighbors {
                            *new_pr.get_mut(&neighbor).unwrap() += contribution;
                        }
                    } else {
                        // dead end：贡献分给所有顶点
                        let contribution = pr_value * self.damping_factor / n;
                        for &other_id in graph.vertices.keys() {
                            *new_pr.get_mut(&other_id).unwrap() += contribution;
                        }
                    }
                }
            }
            
            // 检查收敛
            let mut diff = 0.0;
            for (&vertex_id, &new_value) in &new_pr {
                if let Some(&old_value) = self.pr.get(&vertex_id) {
                    diff += (new_value - old_value).abs();
                }
            }
            
            self.pr = new_pr;
            
            if diff < self.tolerance || iteration >= self.max_iterations {
                break;
            }
        }
        
        println!("    初始化完成：{} 次迭代", iteration);
        
        // 验证 PR 值之和
        let sum: f64 = self.pr.values().sum();
        println!("    初始化后 PR 值之和：{:.6}", sum);
    }
    
    /// 增量更新（CPU/GPU 协同）
    pub fn update(
        &mut self,
        graph: &crate::graph::GraphDB,
        added_edges: &[(u64, u64)],
    ) {
        if added_edges.is_empty() {
            return;
        }
        
        println!("=== 增量更新（CPU/GPU 协同）===");
        println!("添加的边：{:?}", added_edges);
        
        let n = graph.vertex_count() as f64;
        
        // 步骤 1：找出初始受影响顶点（CPU）
        let mut affected_set: HashSet<u64> = HashSet::new();
        
        for &(u, v) in added_edges {
            affected_set.insert(u);
            affected_set.insert(v);
            
            // 添加指向 u 的顶点（因为 u 的出边变化了）
            for (&vertex_id, neighbors) in &graph.adjacency_list {
                if neighbors.contains(&u) {
                    affected_set.insert(vertex_id);
                }
            }
            
            // 添加 v 的邻居（因为 v 的 PR 值变化会影响它们）
            if let Some(neighbors) = graph.adjacency_list.get(&v) {
                for &neighbor in neighbors {
                    affected_set.insert(neighbor);
                }
            }
        }
        
        println!("初始受影响顶点数：{}", affected_set.len());
        
        // 步骤 2：构建 CSR 格式（用于 GPU）
        let csr = self.build_csr(graph);
        
        // 步骤 3：迭代更新，直到收敛（CPU/GPU 协同）
        let mut iteration = 0;
        
        while !affected_set.is_empty() && iteration < self.max_iterations {
            iteration += 1;
            
            // 将受影响顶点转换成索引
            let affected_indices: Vec<u32> = affected_set
                .iter()
                .map(|&vertex_id| csr.vertex_to_idx[&vertex_id])
                .collect();
            
            // GPU：计算受影响顶点的新 PR 值
            let new_pr_values = if let Some(ref gpu) = self.gpu {
                println!("    迭代 {}：调用 GPU 计算 {} 个受影响顶点", iteration, affected_indices.len());
                
                // 准备 PR 值数组
                let pr_values: Vec<f32> = (0..csr.vertex_count)
                    .map(|i| self.pr[&csr.idx_to_vertex[i]] as f32)
                    .collect();
                
                // 调用 GPU
                let gpu_result = gpu.compute_incremental_pagerank(
                    &csr.offsets,
                    &csr.targets,
                    &pr_values,
                    &affected_indices,
                    csr.vertex_count,
                );
                
                gpu_result
            } else {
                // 使用 CPU 计算（临时方案）
                println!("    迭代 {}：使用 CPU 计算 {} 个受影响顶点", iteration, affected_indices.len());
                self.compute_pr_cpu(graph, &affected_set, n)
            };
            
            // 更新 PR 值
            for (i, &vertex_id) in csr.idx_to_vertex.iter().enumerate() {
                self.pr.insert(vertex_id, new_pr_values[i] as f64);
            }
            
            // CPU：检查收敛，找出新的受影响顶点
            let mut new_affected = HashSet::new();
            
            for &vertex_id in &affected_set {
                let old_value = self.pr[&vertex_id] as f32;
                let new_value = new_pr_values[csr.vertex_to_idx[&vertex_id]];
                let diff = (new_value - old_value).abs();
                
                if diff > self.tolerance as f32 {
                    // 这个顶点变化显著，需要传播给它的邻居（出边顶点）
                    if let Some(neighbors) = graph.adjacency_list.get(&vertex_id) {
                        for &neighbor in neighbors {
                            new_affected.insert(neighbor);
                        }
                    }
                }
            }
            
            // 更新受影响集合
            affected_set = new_affected;
            
            if iteration % 5 == 0 {
                println!("    增量迭代 {}：{} 个受影响顶点", iteration, affected_set.len());
            }
            
            if affected_set.is_empty() {
                println!("    增量更新收敛于第 {} 次迭代", iteration);
                break;
            }
        }
        
        if iteration >= self.max_iterations {
            println!("    增量更新达到最大迭代次数：{}", self.max_iterations);
        }
        
        // 验证 PR 值之和
        let sum: f64 = self.pr.values().sum();
        println!("增量更新后 PR 值之和：{:.6}", sum);
    }
    
    /// 构建 CSR 格式（用于 GPU）
    fn build_csr(&self, graph: &crate::graph::GraphDB) -> CSRGraph {
        let mut csr = CSRGraph::new();
        
        // 添加顶点
        let mut vertex_ids: Vec<u64> = graph.vertices.keys().cloned().collect();
        vertex_ids.sort();
        
        for &vertex_id in &vertex_ids {
            csr.add_vertex(vertex_id, HashMap::new());
        }
        
        // 添加边
        let mut edges = Vec::new();
        for (&from, neighbors) in &graph.adjacency_list {
            for &to in neighbors {
                edges.push((from, to));
            }
        }
        
        csr.build_csr(&edges);
        
        csr
    }
    
    /// 获取 PR 值
    pub fn get_pagerank(&self) -> HashMap<u64, f64> {
        self.pr.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    
    #[test]
    fn test_incremental_pagerank() {
        let mut graph = crate::graph::GraphDB::new();
        
        // 添加顶点
        for i in 0..5 {
            let mut props = HashMap::new();
            props.insert("id".to_string(), crate::PropertyValue::Int(i));
            graph.add_vertex(props).unwrap();
        }
        
        // 添加边
        graph.add_edge(0, 1, HashMap::new(), 1.0).unwrap();
        graph.add_edge(1, 2, HashMap::new(), 1.0).unwrap();
        graph.add_edge(2, 0, HashMap::new(), 1.0).unwrap();
        
        // 初始化增量 PageRank
        let mut incremental_pr = IncrementalPageRank::new();
        incremental_pr.initialize(&graph);
        
        // 验证初始化结果
        let pr_init = incremental_pr.get_pagerank();
        let sum_init: f64 = pr_init.values().sum();
        println!("初始化后 PR 值之和：{}", sum_init);
        assert!((sum_init - 1.0).abs() < 0.01, "PR 值之和应该接近 1.0");
        
        // 添加新边（触发增量更新）
        let added_edges = vec![(3, 4)];
        graph.add_edge(3, 4, HashMap::new(), 1.0).unwrap();
        
        incremental_pr.update(&graph, &added_edges);
        
        // 验证增量更新结果
        let pr_updated = incremental_pr.get_pagerank();
        let sum_updated: f64 = pr_updated.values().sum();
        println!("增量更新后 PR 值之和：{}", sum_updated);
        assert!((sum_updated - 1.0).abs() < 0.01, "PR 值之和应该接近 1.0");
        
        println!("✅ 增量 PageRank 测试通过！");
    }
}
