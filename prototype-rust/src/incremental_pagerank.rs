// src/incremental_pagerank.rs
// CPU/GPU 协同的增量 PageRank（EdgeBlock 版本）
// 
// 使用 EdgeBlock 格式优化 GPU 内存访问

use std::collections::HashSet;
use crate::csr_graph::CSRGraph;
use crate::gpu::GPUAccelerator;
use crate::gpu_edge_block::GPUEdgeBlockGraph;

/// CPU/GPU 协同的增量 PageRank（EdgeBlock 版本）
pub struct IncrementalPageRank_EdgeBlock {
    /// GPU 加速器
    gpu: GPUAccelerator,
    /// EdgeBlock 格式的图
    edgeblock_graph: GPUEdgeBlockGraph,
    /// 阻尼因子
    damping_factor: f32,
    /// 最大迭代次数
    max_iterations: usize,
    /// 收敛阈值
    tolerance: f32,
}

impl IncrementalPageRank_EdgeBlock {
    pub fn new(csr: &CSRGraph) -> Result<Self, String> {
        let gpu = GPUAccelerator::new()?;
        let edgeblock_graph = GPUEdgeBlockGraph::from_csr(
            &csr.offsets,
            &csr.targets,
            csr.vertex_count,
        );
        
        Ok(IncrementalPageRank_EdgeBlock {
            gpu,
            edgeblock_graph,
            damping_factor: 0.85,
            max_iterations: 50,
            tolerance: 1e-6,
        })
    }
    
    /// 计算增量 PageRank（EdgeBlock 版本）
    /// 
    /// 参数：
    /// - initial_pr: 初始 PR 值
    /// - affected_vertices: 受影响顶点列表（索引）
    /// 
    /// 返回：更新后的 PR 值
    pub fn compute(
        &self,
        initial_pr: &[f32],
        affected_vertices: &[u32],
    ) -> Vec<f32> {
        let mut pr = initial_pr.to_vec();
        let vertex_count = self.edgeblock_graph.vertex_count as usize;
        
        // 计算出度数组
        let mut out_degrees = vec![0u32; vertex_count];
        for v in 0..vertex_count {
            let start = self.edgeblock_graph.vertices[v];
            let count = self.edgeblock_graph.block_counts[v];
            
            // 计算这个顶点的总边数
            let mut total_edges = 0u32;
            for b in 0..count {
                let block_idx = (start + b) as usize;
                let edge_count = self.edgeblock_graph.blocks[(block_idx * 34) + 1];
                total_edges += edge_count;
            }
            
            out_degrees[v] = total_edges;
        }
        
        // 初始化受影响顶点集合
        let mut affected_set: HashSet<u32> = affected_vertices.iter().cloned().collect();
        let mut iteration = 0;
        
        let start_time = std::time::Instant::now();
        
        // 主循环
        while !affected_set.is_empty() && iteration < self.max_iterations {
            iteration += 1;
            
            // 转换受影响的顶点为数组
            let affected_array: Vec<u32> = affected_set.iter().cloned().collect();
            let affected_count = affected_array.len() as u32;
            
            // 调用 GPU 计算受影响顶点的 PR 值
            let updated_pr = self.gpu.compute_incremental_pagerank_edgeblock(
                &self.edgeblock_graph,
                &pr,
                &affected_array,
                &out_degrees,
                self.damping_factor,
            );
            
            // 检查收敛，找出新的受影响顶点
            let mut new_affected = HashSet::new();
            for &i in &affected_array {
                let diff = (updated_pr[i as usize] - pr[i as usize]).abs();
                if diff > self.tolerance {
                    // 这个顶点的 PR 值变化了，需要传播给出边邻居
                    // 遍历出边（正向 EdgeBlock）
                    let start = self.edgeblock_graph.vertices[i as usize];
                    let count = self.edgeblock_graph.block_counts[i as usize];
                    
                    for b in 0..count {
                        let block_idx = (start + b) as usize;
                        let edge_count = self.edgeblock_graph.blocks[(block_idx * 34) + 1];
                        
                        for j in 0..edge_count {
                            let neighbor = self.edgeblock_graph.blocks[(block_idx * 34) + 2 + j as usize];
                            new_affected.insert(neighbor);
                        }
                    }
                }
            }
            
            // 更新
            pr = updated_pr;
            affected_set = new_affected;
            
            // 打印进度
            if iteration % 5 == 0 {
                let pr_sum: f32 = pr.iter().sum();
                println!("    增量 PageRank (EdgeBlock)：{} 次迭代，{} 个受影响顶点，PR 值之和 = {:.6}", 
                         iteration, affected_set.len(), pr_sum);
            }
        }
        
        // 打印耗时
        let elapsed = start_time.elapsed();
        println!("    增量 PageRank (EdgeBlock)：{} 次迭代，{:.2} ms", 
                 iteration, elapsed.as_secs_f64() * 1000.0);
        
        pr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_incremental_pagerank_edgeblock() {
        // 检查是否有 GPU
        let csr = CSRGraph::new();
        // 创建一个简单的 CSR 图
        // 5 个顶点：0->1, 0->2, 1->2, 2->0, 3->4
        let mut csr = CSRGraph::new();
        
        // 添加顶点
        for i in 0..5 {
            let mut props = std::collections::HashMap::new();
            props.insert("id".to_string(), crate::PropertyValue::Int(i as i64));
            csr.add_vertex(i, props);
        }
        
        // 添加边
        let edges = vec![(0, 1), (0, 2), (1, 2), (2, 0), (3, 4)];
        csr.build_csr(&edges);
        
        let incremental_pr = IncrementalPageRank_EdgeBlock::new(&csr);
        if incremental_pr.is_err() {
            println!("⚠️  没有 GPU，跳过测试");
            return;
        }
        let incremental_pr = incremental_pr.unwrap();
        
        // 初始 PR 值（均匀分布）
        let initial_pr = vec![1.0 / 5.0; 5];
        
        // 测试：计算完整的 PageRank（传入所有顶点作为受影响顶点）
        println!("\n测试：计算完整的 PageRank（EdgeBlock 版本）");
        let all_vertices = vec![0, 1, 2, 3, 4];
        let full_pr = incremental_pr.compute(&initial_pr, &all_vertices);
        
        // 打印结果
        println!("\nPageRank (EdgeBlock) 结果：");
        for i in 0..5 {
            println!("  顶点 {}：PR = {:.6}", i, full_pr[i]);
        }
        
        println!("\n✅ 增量 PageRank (EdgeBlock) 测试通过！");
    }
}
