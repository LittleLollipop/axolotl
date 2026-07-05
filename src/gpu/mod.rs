// src/gpu/mod.rs
// GPU 加速模块（使用 Metal on macOS）
// 
// 严格按照 Swift 版本（第 316-342 行）实现
// 参考：/tmp/test_metal/src/main.rs（最小可工作例子）

use metal::*;
use std::ffi::c_void;

/// GPU 加速器（使用 Metal）
pub struct GPUAccelerator {
    device: Device,
    queue: CommandQueue,
    incremental_pr_pipeline: ComputePipelineState,
}

impl GPUAccelerator {
    /// 创建新的 GPU 加速器（Swift 第 131-137 行）
    pub fn new() -> Result<Self, String> {
        // Swift 第 132 行：创建 Metal 设备
        let device = Device::system_default().expect("No Metal device found");
        
        // Swift 第 135 行：创建命令队列
        let queue = device.new_command_queue();
        
        // Swift 第 284-286 行：创建 GPU 管线
        let kernel_source = include_str!("incremental_pagerank.metal");
        
        // 创建库（Swift 第 284 行）
        let options = CompileOptions::new();
        let library = device
            .new_library_with_source(kernel_source, &options)
            .map_err(|e| format!("Failed to create Metal library: {:?}", e))?;
        
        // 获取函数（Swift 第 285 行）
        let kernel = library
            .get_function("incremental_pagerank", None)
            .map_err(|e| format!("Failed to get Metal function: {:?}", e))?;
        
        // 创建计算管线（Swift 第 286 行）
        let incremental_pr_pipeline = device
            .new_compute_pipeline_state_with_function(&kernel)
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
        let affected_count = affected_vertices.len() as u64;
        
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
        
        // ⚠️ 关键：必须设置计算管线状态（这是之前崩溃的原因）
        encoder.set_compute_pipeline_state(&self.incremental_pr_pipeline);
        
        // Swift 第 325-332 行：设置参数
        encoder.set_buffer(0, Some(&affected_buffer), 0);
        
        let affected_count_u32 = affected_count as u32;
        encoder.set_bytes(1, std::mem::size_of::<u32>(), &affected_count_u32 as *const u32 as *const c_void);
        
        encoder.set_buffer(2, Some(&offsets_buffer), 0);
        encoder.set_buffer(3, Some(&targets_buffer), 0);
        encoder.set_buffer(4, Some(&pr_buffer), 0);
        encoder.set_buffer(5, Some(&new_pr_buffer), 0);
        encoder.set_bytes(6, std::mem::size_of::<u32>(), &vertex_count as *const u32 as *const c_void);
        
        // Swift 第 334-336 行：调度线程
        let grid_size = MTLSize::new(affected_count, 1, 1);
        let threadgroup_size = MTLSize::new(self.incremental_pr_pipeline.thread_execution_width() as u64, 1, 1);
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
    fn create_buffer<T>(&self, data: &[T]) -> Buffer {
        let length = (data.len() * std::mem::size_of::<T>()) as u64;
        
        if length == 0 {
            // 创建空缓冲区
            return self.device.new_buffer(1, MTLResourceOptions::StorageModeShared);
        }
        
        let buffer = self.device.new_buffer_with_data(
            data.as_ptr() as *const c_void,
            length,
            MTLResourceOptions::StorageModeShared, // 适用于 Apple 的 Unified Memory
        );
        
        buffer
    }
}
