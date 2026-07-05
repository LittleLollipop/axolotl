// src/incremental_bfs.rs
// CPU/GPU 协同的增量 BFS（严格按照 Swift 版本翻译）
// 
// 翻译自：/tmp/axolotl_tmp/Experiments/incremental_bfs.swift
// Swift 第 197-279 行：computeIncrementalBFS 函数

use std::collections::HashSet;
use crate::csr_graph::CSRGraph;
use crate::gpu::GPUAccelerator;

/// CPU/GPU 协同的增量 BFS
/// 
/// 严格按照 Swift 版本（第 197-279 行）实现
pub struct IncrementalBFS {
    /// GPU 加速器
    gpu: GPUAccelerator,
    /// 最大迭代次数
    max_iterations: usize,
}

impl IncrementalBFS {
    pub fn new() -> Result<Self, String> {
        let gpu = GPUAccelerator::new()?;
        
        Ok(IncrementalBFS {
            gpu,
            max_iterations: 1000,
        })
    }
    
    /// 计算增量 BFS（严格按照 Swift 第 197-279 行）
    /// 
    /// 参数：
    /// - csr: CSR 格式的图
    /// - initial_distances: 初始距离数组
    /// - affected_vertices: 受影响顶点列表（索引）
    /// 
    /// 返回：更新后的距离数组
    pub fn compute(
        &self,
        csr: &CSRGraph,
        initial_distances: &[u32],
        affected_vertices: &[u32],
    ) -> Vec<u32> {
        // 初始化距离数组
        let mut distances = initial_distances.to_vec();
        let vertex_count = csr.vertex_count as usize;
        
        // 构建正向邻接表（出边）
        let mut forward_adjacency = vec![Vec::new(); vertex_count];
        for u in 0..vertex_count {
            let start = csr.offsets[u] as usize;
            let end = csr.offsets[u + 1] as usize;
            for i in start..end {
                let v = csr.targets[i] as usize;
                forward_adjacency[u].push(v as u32);  // u 指向 v
            }
        }
        
        // 初始化受影响顶点集合
        let mut affected_set: HashSet<u32> = affected_vertices.iter().cloned().collect();
        let mut iteration = 0;
        
        // 开始计时
        let start_time = std::time::Instant::now();
        
        // 主循环
        while !affected_set.is_empty() && iteration < self.max_iterations {
            iteration += 1;
            
            // 转换受影响的顶点为数组
            let affected_array: Vec<u32> = affected_set.iter().cloned().collect();
            
            // 调用 GPU 计算受影响顶点的邻居距离
            let updated_distances = self.gpu.compute_incremental_bfs(
                &csr.offsets,
                &csr.targets,
                &distances,
                &affected_array,
                csr.vertex_count,
            );
            
            // 检查更新，找出新的受影响顶点
            let mut new_affected = HashSet::new();
            for i in 0..vertex_count {
                if updated_distances[i] < distances[i] {
                    // 这个顶点的距离被更新了，下一轮要用它的出边去传播给邻居
                    // 所以把 i 加入受影响集合（下一轮 GPU 会处理 i 的邻居）
                    new_affected.insert(i as u32);
                }
            }
            
            // 更新
            distances = updated_distances;
            affected_set = new_affected;
            
            // 打印进度
            if iteration % 5 == 0 {
                let frontier_size: usize = affected_set.len();
                println!("    增量 BFS 迭代 {}：{} 个顶点在 frontier 中", 
                         iteration, frontier_size);
            }
        }
        
        // 打印耗时
        let elapsed = start_time.elapsed();
        println!("    增量 BFS：{} 次迭代，{:.2} ms", 
                 iteration, elapsed.as_secs_f64() * 1000.0);
        
        distances
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[test]
    fn test_incremental_bfs() {
        // 检查是否有 GPU
        let incremental_bfs = IncrementalBFS::new();
        if incremental_bfs.is_err() {
            println!("⚠️  没有 GPU，跳过测试");
            return;
        }
        let incremental_bfs = incremental_bfs.unwrap();
        
        // 创建一个简单的 CSR 图
        let mut csr = CSRGraph::new();
        
        // 添加顶点
        for i in 0..5 {
            let mut props = HashMap::new();
            props.insert("id".to_string(), crate::PropertyValue::Int(i as i64));
            csr.add_vertex(i, props);
        }
        
        // 添加边：0->1, 0->2, 1->3, 2->3, 3->4
        let edges = vec![(0, 1), (0, 2), (1, 3), (2, 3), (3, 4)];
        csr.build_csr(&edges);
        
        // 初始距离数组（从顶点 0 开始）
        let mut initial_distances = vec![u32::MAX; 5];
        initial_distances[0] = 0;
        
        // 受影响顶点：0
        let affected_vertices = vec![0];
        
        // 计算增量 BFS
        let new_distances = incremental_bfs.compute(&csr, &initial_distances, &affected_vertices);
        
        // 验证结果
        println!("BFS 距离：{:?}", new_distances);
        
        // 顶点 0 的距离为 0
        assert_eq!(new_distances[0], 0);
        
        // 顶点 1 和 2 的距离为 1
        assert_eq!(new_distances[1], 1);
        assert_eq!(new_distances[2], 1);
        
        // 顶点 3 的距离为 2
        assert_eq!(new_distances[3], 2);
        
        // 顶点 4 的距离为 3
        assert_eq!(new_distances[4], 3);
        
        println!("✅ 增量 BFS 测试通过！");
    }
}
