// src/gpu/mod.rs
// GPU 加速模块（使用 Metal on macOS）
// 
// 严格按照 Swift 版本（第 316-342 行）实现

use metal::*;
use std::ffi::c_void;

/// GPU 加速器（使用 Metal）
pub struct GPUAccelerator {
    device: MTLDevice,
    queue: MTLCommandQueue,
    incremental_pr_pipeline: MTLComputePipelineState,
}

impl GPUAccelerator {
    /// 创建新的 GPU 加速器（Swift 第 131-137 行）
    pub fn new() -> Result<Self, String> {
        // Swift 第 132 行：创建 Metal 设备
        let device = MTLDevice::system_default_device()
            .ok_or_else(|| "No Metal device found".to_string())?;
        
        // Swift 第 135 行：创建命令队列
        let queue = device.new_command_queue();
        
        // Swift 第 284-286 行：创建 GPU 管线
        let kernel_source = include_str!("incremental_pagerank.metal");
        
        // 创建库（Swift 第 284 行）
        let library = device.new_library_with_source(kernel_source, &[])
            .map_err(|e| format!("Failed to create Metal library: {:?}", e))?;
        
        // 获取函数（Swift 第 285 行）
        let kernel = library.get_function("incremental_pagerank", None)
            .map_err(|e| format!("Failed to get Metal function: {:?}", e))?;
        
        // 创建计算管线（Swift 第 286 行）
        let incremental_pr_pipeline = device.new_compute_pipeline_state_with_function(&kernel)
            .map_err(|e| format!("Failed to create compute pipeline: {:?}", e))?;
        
        Ok(GPUAccelerator {
            device,
            queue,
            incremental_pr_pipeline,
        })
    }
    
    /// 计算增量 PageRank（GPU 加速）
    /// 
    /// 严格按照 Swift 第 316-342 行实现
    pub fn compute_incremental_pagerank(
        &self,
        csr_offsets: &[u32],
        csr_targets: &[u32],
        pr: &[f32],
        affected_vertices: &[u32],
        vertex_count: u32,
    ) -> Vec<f32> {
        let affected_count = affected_vertices.len() as u32;
        
        // Swift 第 316-321 行：创建缓冲区
        let affected_buffer = self.create_buffer(affected_vertices);
        let offsets_buffer = self.create_buffer(csr_offsets);
        let targets_buffer = self.create_buffer(csr_targets);
        let pr_buffer = self.create_buffer(pr);
        
        let mut new_pr = vec![0.0f32; vertex_count as usize];
        let new_pr_buffer = self.create_buffer(&new_pr);
        
        // Swift 第 323-324 行：创建命令缓冲区和编码器
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();
        
        // Swift 第 325-332 行：设置参数
        encoder.set_compute_pipeline_state(&self.incremental_pr_pipeline);
        encoder.set_buffer(0, Some(&affected_buffer), 0);
        encoder.set_bytes(1, std::mem::size_of::<u32>(), &affected_count);
        encoder.set_buffer(2, Some(&offsets_buffer), 0);
        encoder.set_buffer(3, Some(&targets_buffer), 0);
        encoder.set_buffer(4, Some(&pr_buffer), 0);
        encoder.set_buffer(5, Some(&new_pr_buffer), 0);
        encoder.set_bytes(6, std::mem::size_of::<u32>(), &vertex_count);
        
        // Swift 第 334-336 行：调度线程
        let grid_size = MTLSize::new(affected_count as u64, 1, 1);
        let threadgroup_size = MTLSize::new(256, 1, 1);
        encoder.dispatch_threads(grid_size, threadgroup_size);
        
        // Swift 第 337-339 行：执行
        encoder.end_encoding();
        command_buffer.commit();
        command_buffer.wait_until_completed();
        
        // Swift 第 342 行：读取结果
        let result_ptr = new_pr_buffer.contents() as *const f32;
        let result_slice = unsafe {
            std::slice::from_raw_parts(result_ptr, vertex_count as usize)
        };
        new_pr.copy_from_slice(result_slice);
        
        new_pr
    }
    
    /// 创建 Metal 缓冲区
    fn create_buffer<T>(&self, data: &[T]) -> metal::Buffer {
        let length = data.len() * std::mem::size_of::<T>();
        
        if length == 0 {
            // 创建空缓冲区
            return self.device.new_buffer(1, MTLResourceOptions::CPUCacheModeDefaultCache);
        }
        
        let buffer = self.device.new_buffer_with_data(
            data.as_ptr() as *const c_void,
            length,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        
        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gpu_accelerator() {
        // 检查是否有 Metal 设备
        let gpu = GPUAccelerator::new();
        if gpu.is_err() {
            println!("⚠️  没有 Metal 设备，跳过 GPU 测试");
            return;
        }
        let gpu = gpu.unwrap();
        
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
        
        println!("✅ GPU 加速器测试通过！");
    }
}
