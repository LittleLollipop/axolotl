// src/incremental_pagerank.rs
// CPU/GPU 协同的增量 PageRank（严格按照 Swift 版本翻译）
// 
// 翻译自：/tmp/axolotl_tmp/Experiments/incremental_pagerank.swift
// Swift 第 275-369 行：computeIncrementalPageRank 函数

use std::collections::HashSet;
use crate::csr_graph::CSRGraph;
use crate::gpu::GPUAccelerator;

/// CPU/GPU 协同的增量 PageRank
/// 
/// 严格按照 Swift 版本（第 275-369 行）实现
pub struct IncrementalPageRank {
    /// GPU 加速器
    gpu: GPUAccelerator,
    /// 阻尼因子
    damping_factor: f32,
    /// 最大迭代次数
    max_iterations: usize,
    /// 收敛阈值
    tolerance: f32,
}

impl IncrementalPageRank {
    pub fn new() -> Result<Self, String> {
        let gpu = GPUAccelerator::new()?;
        
        Ok(IncrementalPageRank {
            gpu,
            damping_factor: 0.85,
            max_iterations: 50,
            tolerance: 1e-6,
        })
    }
    
    /// 计算增量 PageRank（严格按照 Swift 第 275-369 行）
    /// 
    /// 参数：
    /// - csr: CSR 格式的图
    /// - initial_pr: 初始 PR 值
    /// - affected_vertices: 受影响顶点列表（索引）
    /// 
    /// 返回：更新后的 PR 值
    pub fn compute(
        &self,
        csr: &CSRGraph,
        initial_pr: &[f32],
        affected_vertices: &[u32],
    ) -> Vec<f32> {
        // Swift 第 284-286 行：创建 GPU 管线
        // （已经在 GPUAccelerator::new() 中完成了）
        
        // Swift 第 288 行：初始化 PR 值
        let mut pr = initial_pr.to_vec();
        let vertex_count = csr.vertex_count as usize;
        
        // Swift 第 292-300 行：构建反向邻接表
        let mut reverse_adjacency = vec![Vec::new(); vertex_count];
        for u in 0..vertex_count {
            let start = csr.offsets[u] as usize;
            let end = csr.offsets[u + 1] as usize;
            for i in start..end {
                let v = csr.targets[i] as usize;
                reverse_adjacency[v].push(u as u32);
            }
        }
        
        // Swift 第 303-304 行：初始化受影响顶点集合
        let mut affected_set: HashSet<u32> = affected_vertices.iter().cloned().collect();
        let mut iteration = 0;
        
        // Swift 第 306 行：开始计时（可选）
        let start_time = std::time::Instant::now();
        
        // Swift 第 308 行：主循环
        while !affected_set.is_empty() && iteration < self.max_iterations {
            iteration += 1;
            
            // Swift 第 310 行：转换受影响的顶点为数组
            let affected_array: Vec<u32> = affected_set.iter().cloned().collect();
            let affected_count = affected_array.len() as u32;
            
            // Swift 第 314 行：复制当前 PR 值
            let mut new_pr = pr.clone();
            
            // Swift 第 316-321 行：创建 GPU 缓冲区
            // 调用 GPU 计算受影响顶点的 PR 值
            let updated_pr = self.gpu.compute_incremental_pagerank(
                &csr.offsets,
                &csr.targets,
                &pr,
                &affected_array,
                csr.vertex_count,
            );
            
            // Swift 第 345-354 行：检查收敛，找出新的受影响顶点
            let mut new_affected = HashSet::new();
            for i in 0..vertex_count {
                let diff = (updated_pr[i] - pr[i]).abs();
                if diff > self.tolerance {
                    // Swift 第 350-352 行：传播给邻居
                    for &neighbor in &reverse_adjacency[i] {
                        new_affected.insert(neighbor);
                    }
                }
            }
            
            // Swift 第 356-358 行：更新
            pr = updated_pr;
            affected_set = new_affected;
            
            // Swift 第 360-362 行：打印进度
            if iteration % 5 == 0 {
                let pr_sum: f32 = pr.iter().sum();
                println!("    增量迭代 {}：{} 个受影响顶点，PR 值之和 = {:.6}", 
                         iteration, affected_set.len(), pr_sum);
            }
        }
        
        // Swift 第 365-366 行：打印耗时
        let elapsed = start_time.elapsed();
        println!("    增量 PageRank：{} 次迭代，{:.2} ms", 
                 iteration, elapsed.as_secs_f64() * 1000.0);
        
        // Swift 第 368 行：返回结果
        pr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    
    #[test]
    fn test_incremental_pagerank() {
        // 检查是否有 GPU
        let incremental_pr = IncrementalPageRank::new();
        if incremental_pr.is_err() {
            println!("⚠️  没有 GPU，跳过测试");
            return;
        }
        let incremental_pr = incremental_pr.unwrap();
        
        // 创建一个简单的 CSR 图
        // 5 个顶点：0->1, 0->2, 1->2, 2->0, 3->4
        let mut csr = CSRGraph::new();
        
        // 添加顶点
        for i in 0..5 {
            let mut props = std::collections::HashMap::new();
            props.insert("id".to_string(), crate::csr_graph::PropertyValue::Int(i as i64));
            csr.add_vertex(i, props);
        }
        
        // 添加边
        let edges = vec![(0, 1), (0, 2), (1, 2), (2, 0), (3, 4)];
        csr.build_csr(&edges);
        
        // 初始 PR 值
        let initial_pr = vec![1.0 / 5.0; 5];
        
        // 受影响顶点：0, 1
        let affected_vertices = vec![0, 1];
        
        // 计算增量 PageRank
        let new_pr = incremental_pr.compute(&csr, &initial_pr, &affected_vertices);
        
        // 验证 PR 值之和接近 1.0
        let sum: f32 = new_pr.iter().sum();
        println!("PR 值之和：{}", sum);
        
        assert!((sum - 1.0).abs() < 0.01, "PR 值之和应该接近 1.0");
        
        println!("✅ 增量 PageRank 测试通过！");
    }
}
