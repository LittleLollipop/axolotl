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
        
    // 构建正向邻接表（出边）和反向邻接表（入边）
    let mut forward_adjacency = vec![Vec::new(); vertex_count];  // 出边邻居
    let mut reverse_adjacency = vec![Vec::new(); vertex_count];  // 入边邻居
    for u in 0..vertex_count {
        let start = csr.offsets[u] as usize;
        let end = csr.offsets[u + 1] as usize;
        for i in start..end {
            let v = csr.targets[i] as usize;
            forward_adjacency[u].push(v as u32);  // u 指向 v
            reverse_adjacency[v].push(u as u32);   // v 的入边来自 u
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
                &csr.reverse_offsets,  // 反向 CSR
                &csr.reverse_targets,  // 反向 CSR
                &pr,
                &affected_array,
                csr.vertex_count,
            );
            
            // Swift 第 345-354 行：检查收敛，找出新的受影响顶点
            let mut new_affected = HashSet::new();
            for i in 0..vertex_count {
                let diff = (updated_pr[i] - pr[i]).abs();
                if diff > self.tolerance {
                    // 这个顶点的 PR 值变化了，需要传播给出边邻居
                    for &neighbor in &forward_adjacency[i] {
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
        
        // 初始 PR 值（均匀分布）
        let initial_pr = vec![1.0 / 5.0; 5];
        
        // 测试 1：计算完整的 PageRank（传入所有顶点作为受影响顶点）
        println!("\n测试 1：计算完整的 PageRank（所有顶点都受影响）");
        let all_vertices = vec![0, 1, 2, 3, 4];
        let full_pr = incremental_pr.compute(&csr, &initial_pr, &all_vertices);
        
        // 测试 2：计算完整的 PageRank（CPU 版本，用于验证）
        println!("\n测试 2：计算完整的 PageRank（CPU 版本）");
        let cpu_pr = compute_full_pagerank_cpu(&csr, &initial_pr, 100);
        
        // 对比结果
        println!("\n对比 GPU 和 CPU 的结果：");
        let mut max_diff = 0.0f32;
        for i in 0..5 {
            let diff = (full_pr[i] - cpu_pr[i]).abs();
            if diff > max_diff {
                max_diff = diff;
            }
            println!("  顶点 {}：GPU = {:.6}, CPU = {:.6}, 差异 = {:.6}", 
                     i, full_pr[i], cpu_pr[i], diff);
        }
        
        println!("\n最大差异：{:.6}", max_diff);
        
        // 验证：GPU 和 CPU 的结果应该非常接近
        assert!(max_diff < 1e-4, "GPU 和 CPU 的 PageRank 结果差异过大");
        
        println!("\n✅ 增量 PageRank 测试通过！");
    }
}

/// CPU 版本的完整 PageRank（用于验证）
fn compute_full_pagerank_cpu(
    csr: &CSRGraph,
    initial_pr: &[f32],
    iterations: usize,
) -> Vec<f32> {
    let vertex_count = csr.vertex_count as usize;
    let mut pr = initial_pr.to_vec();
    let damping = 0.85f32;
    
    for _ in 0..iterations {
        let mut new_pr = vec![0.0; vertex_count];
        
        // 使用反向 CSR 找出每个顶点的入边邻居
        for v in 0..vertex_count {
            let mut contribution = 0.0f32;
            let start = csr.reverse_offsets[v] as usize;
            let end = csr.reverse_offsets[v + 1] as usize;
            
            for i in start..end {
                let u = csr.reverse_targets[i] as usize;  // 有边从 u 指向 v
                let out_degree = (csr.offsets[u + 1] - csr.offsets[u]) as f32;
                if out_degree > 0.0 {
                    contribution += pr[u] / out_degree;
                }
            }
            
            new_pr[v] = (1.0 - damping) / vertex_count as f32 + damping * contribution;
        }
        
        pr = new_pr;
    }
    
    pr
}
