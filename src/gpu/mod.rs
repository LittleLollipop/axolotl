// src/gpu/mod.rs
// GPU 加速模块（使用 Metal on macOS）

use metal::*;
use objc::*;

/// GPU 加速器（使用 Metal）
pub struct GPUAccelerator {
    device: MTLDevice,
    queue: MTLCommandQueue,
    incremental_pr_pipeline: MTLComputePipelineState,
}

impl GPUAccelerator {
    /// 创建新的 GPU 加速器
    pub fn new() -> Result<Self, String> {
        // 创建 Metal 设备
        let device = MTLDevice::system_default_device()
            .ok_or_else(|| "No Metal device found".to_string())?;
        
        let queue = device.new_command_queue();
        
        // 加载增量 PageRank 的 Metal 着色器
        let kernel_source = include_str!("incremental_pagerank.metal");
        
        // 创建库
        let library = device.new_library_with_source(kernel_source, &[])
            .map_err(|e| format!("Failed to create library: {:?}", e))?;
        
        // 获取函数
        let kernel = library.get_function("incremental_pagerank", None)
            .map_err(|e| format!("Failed to get function: {:?}", e))?;
        
        // 创建计算管线
        let incremental_pr_pipeline = device.new_compute_pipeline_state_with_function(&kernel)
            .map_err(|e| format!("Failed to create pipeline: {:?}", e))?;
        
        Ok(GPUAccelerator {
            device,
            queue,
            incremental_pr_pipeline,
        })
    }
    
    /// 计算增量 PageRank（GPU 加速）
    /// 
    /// 参数：
    /// - csr_offsets: CSR 格式的偏移数组
    /// - csr_targets: CSR 格式的边数组（目标顶点索引）
    /// - pr: 当前 PR 值（float 数组）
    /// - affected_vertices: 受影响顶点列表（索引）
    /// - vertex_count: 顶点数量
    /// 
    /// 返回：更新后的 PR 值
    pub fn compute_incremental_pagerank(
        &self,
        csr_offsets: &[u32],
        csr_targets: &[u32],
        pr: &[f32],
        affected_vertices: &[u32],
        vertex_count: u32,
    ) -> Vec<f32> {
        let affected_count = affected_vertices.len() as u32;
        
        // 创建缓冲区
        let affected_buffer = self.device.new_buffer_with_data(
            affected_vertices.as_ptr() as *const _,
            (affected_count as usize) * std::mem::size_of::<u32>(),
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        
        let offsets_buffer = self.device.new_buffer_with_data(
            csr_offsets.as_ptr() as *const _,
            (vertex_count as usize + 1) * std::mem::size_of::<u32>(),
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        
        let targets_buffer = self.device.new_buffer_with_data(
            csr_targets.as_ptr() as *const _,
            csr_targets.len() * std::mem::size_of::<u32>(),
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        
        let pr_buffer = self.device.new_buffer_with_data(
            pr.as_ptr() as *const _,
            pr.len() * std::mem::size_of::<f32>(),
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        
        let mut new_pr = vec![0.0f32; pr.len()];
        let new_pr_buffer = self.device.new_buffer_with_data(
            new_pr.as_ptr() as *const _,
            new_pr.len() * std::mem::size_of::<f32>(),
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        
        // 创建命令缓冲区
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();
        
        encoder.set_compute_pipeline_state(&self.incremental_pr_pipeline);
        encoder.set_buffer(0, Some(&affected_buffer), 0);
        encoder.set_bytes(1, std::mem::size_of::<u32>(), &affected_count);
        encoder.set_buffer(2, Some(&offsets_buffer), 0);
        encoder.set_buffer(3, Some(&targets_buffer), 0);
        encoder.set_buffer(4, Some(&pr_buffer), 0);
        encoder.set_buffer(5, Some(&new_pr_buffer), 0);
        encoder.set_bytes(6, std::mem::size_of::<u32>(), &vertex_count);
        
        // 调度线程
        let grid_size = MTLSize::new(affected_count as u64, 1, 1);
        let threadgroup_size = MTLSize::new(256, 1, 1);
        encoder.dispatch_threads(grid_size, threadgroup_size);
        
        encoder.end_encoding();
        command_buffer.commit();
        command_buffer.wait_until_completed();
        
        // 读取结果
        let result_ptr = new_pr_buffer.contents() as *const f32;
        let result_slice = std::slice::from_raw_parts(result_ptr, pr.len());
        new_pr.copy_from_slice(result_slice);
        
        new_pr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gpu_accelerator() {
        // 检查是否有 Metal 设备
        if MTLDevice::system_default_device().is_null() {
            println!("⚠️  没有 Metal 设备，跳过 GPU 测试");
            return;
        }
        
        let gpu = GPUAccelerator::new();
        
        // 创建一个简单的 CSR 图
        // 3 个顶点：0 -> 1, 0 -> 2, 1 -> 2
        let csr_offsets = vec![0, 2, 3, 3];  // 顶点 0：边 0-1，顶点 1：边 2，顶点 2：无边
        let csr_targets = vec![1, 2, 2];  // 边 0：0->1，边 1：0->2，边 2：1->2
        let vertex_count = 3;
        
        // 初始 PR 值
        let pr = vec![1.0 / 3.0; 3];
        
        // 受影响顶点：0, 1
        let affected_vertices = vec![0, 1];
        
        // 调用 GPU
        let new_pr = gpu.compute_incremental_pagerank(
            &csr_offsets,
            &csr_targets,
            &pr,
            &affected_vertices,
            vertex_count,
        );
        
        println!("GPU 计算后的 PR 值：{:?}", new_pr);
        
        // 验证 PR 值之和接近 1.0
        let sum: f32 = new_pr.iter().sum();
        println!("PR 值之和：{}", sum);
        
        assert!((sum - 1.0).abs() < 0.01, "PR 值之和应该接近 1.0");
    }
}
