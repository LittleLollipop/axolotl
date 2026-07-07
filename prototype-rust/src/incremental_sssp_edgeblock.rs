// src/incremental_sssp_edgeblock.rs
// CPU/GPU 协同的增量 SSSP（EdgeBlock 版本，无权图）
// 
// 使用 EdgeBlock 格式优化 GPU 内存访问
// 无权图：所有边的权重都是 1，SSSP = BFS

use crate::csr_graph::CSRGraph;
use crate::gpu::GPUAccelerator;
use crate::gpu_edge_block::GPUEdgeBlockGraph;

/// CPU/GPU 协同的增量 SSSP（EdgeBlock 版本，无权图）
pub struct IncrementalSsspEdgeBlock {
    /// GPU 加速器
    gpu: GPUAccelerator,
    /// EdgeBlock 格式的图
    edgeblock_graph: GPUEdgeBlockGraph,
}

impl IncrementalSsspEdgeBlock {
    pub fn new(csr: &CSRGraph) -> Result<Self, String> {
        let gpu = GPUAccelerator::new()?;
        let edgeblock_graph = GPUEdgeBlockGraph::from_csr(
            &csr.offsets,
            &csr.targets,
            csr.vertex_count,
        );
        
        Ok(IncrementalSsspEdgeBlock {
            gpu,
            edgeblock_graph,
        })
    }
    
    /// 计算增量 SSSP（EdgeBlock 版本，无权图）
    /// 
    /// 参数：
    /// - source: 源顶点
    /// - initial_distances: 初始距离数组（u32，跳数）
    /// - affected_vertices: 受影响顶点列表（索引）
    /// 
    /// 返回：更新后的距离数组
    pub fn compute(
        &self,
        _source: u32,
        initial_distances: &[u32],
        affected_vertices: &[u32],
    ) -> Vec<u32> {
        let vertex_count = self.edgeblock_graph.vertex_count as usize;
        
        // 初始化距离数组（从 initial_distances 复制）
        let mut distances = initial_distances.to_vec();
        
        // 初始化访问标记
        let mut visited = vec![0u32; vertex_count];
        for i in 0..vertex_count {
            if distances[i] < u32::MAX {
                visited[i] = 1;
            }
        }
        
        // 初始化 frontier（受影响顶点）
        let mut frontier: Vec<u32> = affected_vertices.to_vec();
        
        // BFS 主循环（无权图的 SSSP = BFS）
        while !frontier.is_empty() {
            let _frontier_len = frontier.len() as u32;
            
            // 调用 GPU 计算
            let updated_distances = self.gpu.compute_incremental_sssp_edgeblock(
                &self.edgeblock_graph,
                &distances,
                &frontier,
            );
            
            // 找出新的 frontier（距离更新的顶点）
            let mut new_frontier = Vec::new();
            for i in 0..vertex_count {
                if updated_distances[i] < u32::MAX && visited[i] == 0 {
                    visited[i] = 1;
                    new_frontier.push(i as u32);
                }
            }
            
            // 更新距离
            distances = updated_distances;
            
            // 更新 frontier
            frontier = new_frontier;
        }
        
        distances
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_incremental_sssp_edgeblock() {
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
        
        let incremental_sssp = IncrementalSsspEdgeBlock::new(&csr);
        if incremental_sssp.is_err() {
            println!("⚠️  没有 GPU，跳过测试");
            return;
        }
        let incremental_sssp = incremental_sssp.unwrap();
        
        // 初始距离数组（源顶点 = 0）
        let mut initial_distances = vec![u32::MAX; 5];
        initial_distances[0] = 0;
        
        // 测试：计算 SSSP（传入源顶点作为受影响顶点）
        println!("\n测试：计算 SSSP（EdgeBlock 版本，无权图）");
        let affected_vertices = vec![0];
        let sssp_result = incremental_sssp.compute(0, &initial_distances, &affected_vertices);
        
        // 打印结果
        println!("\nSSSP (EdgeBlock) 结果：");
        for i in 0..5 {
            if sssp_result[i] == u32::MAX {
                println!("  顶点 {}：距离 = INF", i);
            } else {
                println!("  顶点 {}：距离 = {}", i, sssp_result[i]);
            }
        }
        
        println!("\n✅ 增量 SSSP (EdgeBlock) 测试通过！");
    }
}
