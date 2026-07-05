// src/incremental_bfs_edgeblock.rs
// CPU/GPU 协同的增量 BFS（使用 EdgeBlock 格式）
// 
// 使用 EdgeBlock 格式优化 GPU 内存访问

use crate::csr_graph::CSRGraph;
use crate::gpu::GPUAccelerator;
use crate::gpu_edge_block::GPUEdgeBlockGraph;

/// CPU/GPU 协同的增量 BFS（使用 EdgeBlock 格式）
pub struct IncrementalBFS_EdgeBlock {
    /// GPU 加速器
    gpu: GPUAccelerator,
    /// GPU EdgeBlock 格式的图
    gpu_eb: GPUEdgeBlockGraph,
}

impl IncrementalBFS_EdgeBlock {
    pub fn new(csr: &CSRGraph) -> Result<Self, String> {
        let gpu = GPUAccelerator::new()?;
        
        // 转换成 GPU EdgeBlock 格式
        let gpu_eb = GPUEdgeBlockGraph::from_csr(
            &csr.offsets,
            &csr.targets,
            csr.vertex_count,
        );
        
        Ok(IncrementalBFS_EdgeBlock {
            gpu,
            gpu_eb,
        })
    }
    
    /// 计算增量 BFS（使用 EdgeBlock 格式）
    /// 
    /// 参数：
    /// - initial_distances: 初始距离数组
    /// - affected_vertices: 受影响顶点列表（距离已更新的顶点）
    /// 
    /// 返回：更新后的距离数组
    pub fn compute(
        &self,
        initial_distances: &[u32],
        affected_vertices: &[u32],
    ) -> Vec<u32> {
        // 调用 GPU 计算增量 BFS
        // compute_incremental_bfs_edgeblock() 已经实现了完整的 BFS 循环
        let distances = self.gpu.compute_incremental_bfs_edgeblock(
            &self.gpu_eb,
            initial_distances,
            affected_vertices,
        );
        
        distances
    }
}
