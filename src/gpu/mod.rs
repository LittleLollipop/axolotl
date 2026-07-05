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
    incremental_bfs_pipeline: ComputePipelineState,
    bfs_edgeblock_pipeline: ComputePipelineState,  // BFS EdgeBlock 管线
    incremental_sssp_pipeline: ComputePipelineState,  // SSSP 管线
    pagerank_edgeblock_pipeline: ComputePipelineState,  // PageRank EdgeBlock 管线
    sssp_edgeblock_pipeline: ComputePipelineState,  // SSSP EdgeBlock 管线
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
        
        // 创建 BFS 管线
        let bfs_kernel_source = include_str!("incremental_bfs.metal");
        let bfs_library = device
            .new_library_with_source(bfs_kernel_source, &options)
            .map_err(|e| format!("Failed to create BFS Metal library: {:?}", e))?;
        
        let bfs_kernel = bfs_library
            .get_function("incremental_bfs", None)
            .map_err(|e| format!("Failed to get BFS Metal function: {:?}", e))?;
        
        let incremental_bfs_pipeline = device
            .new_compute_pipeline_state_with_function(&bfs_kernel)
            .map_err(|e| format!("Failed to create BFS compute pipeline: {:?}", e))?;
        
        // 创建 BFS EdgeBlock 管线
        let bfs_edgeblock_kernel_source = include_str!("bfs_edgeblock.metal");
        let bfs_edgeblock_library = device
            .new_library_with_source(bfs_edgeblock_kernel_source, &options)
            .map_err(|e| format!("Failed to create BFS EdgeBlock Metal library: {:?}", e))?;
        
        let bfs_edgeblock_kernel = bfs_edgeblock_library
            .get_function("bfs_edgeblock", None)
            .map_err(|e| format!("Failed to get BFS EdgeBlock Metal function: {:?}", e))?;
        
        let bfs_edgeblock_pipeline = device
            .new_compute_pipeline_state_with_function(&bfs_edgeblock_kernel)
            .map_err(|e| format!("Failed to create BFS EdgeBlock compute pipeline: {:?}", e))?;
        
        // 创建 SSSP 管线
        let sssp_kernel_source = include_str!("incremental_sssp.metal");
        let sssp_library = device
            .new_library_with_source(sssp_kernel_source, &options)
            .map_err(|e| format!("Failed to create SSSP Metal library: {:?}", e))?;
        
        let sssp_kernel = sssp_library
            .get_function("incremental_sssp", None)
            .map_err(|e| format!("Failed to get SSSP Metal function: {:?}", e))?;
        
        let incremental_sssp_pipeline = device
            .new_compute_pipeline_state_with_function(&sssp_kernel)
            .map_err(|e| format!("Failed to create SSSP compute pipeline: {:?}", e))?;
        
        // 创建 PageRank EdgeBlock 管线
        let pagerank_edgeblock_kernel_source = include_str!("pagerank_edgeblock.metal");
        let pagerank_edgeblock_library = device
            .new_library_with_source(pagerank_edgeblock_kernel_source, &options)
            .map_err(|e| format!("Failed to create PageRank EdgeBlock Metal library: {:?}", e))?;
        
        let pagerank_edgeblock_kernel = pagerank_edgeblock_library
            .get_function("pagerank_edgeblock_optimized", None)
            .map_err(|e| format!("Failed to get PageRank EdgeBlock Metal function: {:?}", e))?;
        
        let pagerank_edgeblock_pipeline = device
            .new_compute_pipeline_state_with_function(&pagerank_edgeblock_kernel)
            .map_err(|e| format!("Failed to create PageRank EdgeBlock compute pipeline: {:?}", e))?;
        
        // 创建 SSSP EdgeBlock 管线
        let sssp_edgeblock_kernel_source = include_str!("sssp_edgeblock.metal");
        let sssp_edgeblock_library = device
            .new_library_with_source(sssp_edgeblock_kernel_source, &options)
            .map_err(|e| format!("Failed to create SSSP EdgeBlock Metal library: {:?}", e))?;
        
        let sssp_edgeblock_kernel = sssp_edgeblock_library
            .get_function("sssp_edgeblock_unweighted", None)
            .map_err(|e| format!("Failed to get SSSP EdgeBlock Metal function: {:?}", e))?;
        
        let sssp_edgeblock_pipeline = device
            .new_compute_pipeline_state_with_function(&sssp_edgeblock_kernel)
            .map_err(|e| format!("Failed to create SSSP EdgeBlock compute pipeline: {:?}", e))?;
        
        Ok(GPUAccelerator {
            device,
            queue,
            incremental_pr_pipeline,
            incremental_bfs_pipeline,
            bfs_edgeblock_pipeline,
            incremental_sssp_pipeline,
            pagerank_edgeblock_pipeline,
            sssp_edgeblock_pipeline,
        })
    }
    
    /// 计算增量 PageRank（GPU 加速）
    /// 
    /// 严格按照 Swift 第 316-342 行实现
    pub fn compute_incremental_pagerank(
        &self,
        csr_offsets: &[u32],
        csr_targets: &[u32],
        reverse_offsets: &[u32],  // 反向 CSR 的偏移数组
        reverse_targets: &[u32],  // 反向 CSR 的边数组
        pr: &[f32],
        affected_vertices: &[u32],
        vertex_count: u32,
    ) -> Vec<f32> {
        let affected_count = affected_vertices.len() as u64;
        
        // Swift 第 316-321 行：创建缓冲区
        let affected_buffer = self.create_buffer(affected_vertices);
        let reverse_offsets_buffer = self.create_buffer(reverse_offsets);
        let reverse_targets_buffer = self.create_buffer(reverse_targets);
        let offsets_buffer = self.create_buffer(csr_offsets);
        let pr_buffer = self.create_buffer(pr);
        
        // Swift 第 314 行：复制当前的 PR 值
        let mut new_pr = pr.to_vec();
        let new_pr_buffer = self.create_buffer(&new_pr);
        
        // Swift 第 323-324 行：创建命令缓冲区和编码器
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();
        
        // ⚠️ 关键：必须设置计算管线状态（这是之前崩溃的原因）
        encoder.set_compute_pipeline_state(&self.incremental_pr_pipeline);
        
        // Swift 第 325-332 行：设置参数
        encoder.set_buffer(0, Some(&affected_buffer), 0);
        
        let affected_count_u32 = affected_count as u32;
        encoder.set_bytes(1, std::mem::size_of::<u32>() as u64, &affected_count_u32 as *const u32 as *const c_void);
        
        encoder.set_buffer(2, Some(&reverse_offsets_buffer), 0);
        encoder.set_buffer(3, Some(&reverse_targets_buffer), 0);
        encoder.set_buffer(4, Some(&offsets_buffer), 0);
        encoder.set_buffer(5, Some(&pr_buffer), 0);
        encoder.set_buffer(6, Some(&new_pr_buffer), 0);
        encoder.set_bytes(7, std::mem::size_of::<u32>() as u64, &vertex_count as *const u32 as *const c_void);
        
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
    
    /// 计算增量 BFS（GPU 加速）
    /// 
    /// 严格按照 Swift 第 217-279 行实现
    pub fn compute_incremental_bfs(
        &self,
        csr_offsets: &[u32],
        csr_targets: &[u32],
        distances: &[u32],
        affected_vertices: &[u32],
        vertex_count: u32,
    ) -> Vec<u32> {
        let affected_count = affected_vertices.len() as u64;
        
        // 创建缓冲区
        let affected_buffer = self.create_buffer(affected_vertices);
        let offsets_buffer = self.create_buffer(csr_offsets);
        let targets_buffer = self.create_buffer(csr_targets);
        let distances_buffer = self.create_buffer(distances);
        
        // 复制当前的距离值
        let mut new_distances = distances.to_vec();
        let new_distances_buffer = self.create_buffer(&new_distances);
        
        // 创建命令缓冲区和编码器
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();
        
        // 设置计算管线状态
        encoder.set_compute_pipeline_state(&self.incremental_bfs_pipeline);
        
        // 设置参数
        encoder.set_buffer(0, Some(&affected_buffer), 0);
        
        let affected_count_u32 = affected_count as u32;
        encoder.set_bytes(1, std::mem::size_of::<u32>() as u64, &affected_count_u32 as *const u32 as *const c_void);
        
        encoder.set_buffer(2, Some(&offsets_buffer), 0);
        encoder.set_buffer(3, Some(&targets_buffer), 0);
        encoder.set_buffer(4, Some(&distances_buffer), 0);
        encoder.set_buffer(5, Some(&new_distances_buffer), 0);
        encoder.set_bytes(6, std::mem::size_of::<u32>() as u64, &vertex_count as *const u32 as *const c_void);
        
        // 调度线程
        let grid_size = MTLSize::new(affected_count, 1, 1);
        let threadgroup_size = MTLSize::new(self.incremental_bfs_pipeline.thread_execution_width() as u64, 1, 1);
        encoder.dispatch_threads(grid_size, threadgroup_size);
        
        // 执行
        encoder.end_encoding();
        command_buffer.commit();
        command_buffer.wait_until_completed();
        
        // 读取结果
        let result_ptr = new_distances_buffer.contents() as *const u32;
        let result_slice = unsafe {
            std::slice::from_raw_parts(result_ptr, vertex_count as usize)
        };
        new_distances.copy_from_slice(result_slice);
        
        new_distances
    }
    
    /// 计算增量 SSSP（GPU 加速）
    /// 
    /// 参考 Swift 版本的增量 SSSP 实现
    pub fn compute_incremental_sssp(
        &self,
        csr_offsets: &[u32],
        csr_targets: &[u32],
        weights: &[f32],
        distances: &[f32],
        affected_vertices: &[u32],
        vertex_count: u32,
    ) -> Vec<f32> {
        let affected_count = affected_vertices.len() as u64;
        
        // 创建缓冲区
        let affected_buffer = self.create_buffer(affected_vertices);
        let offsets_buffer = self.create_buffer(csr_offsets);
        let targets_buffer = self.create_buffer(csr_targets);
        let weights_buffer = self.create_buffer(weights);
        let distances_buffer = self.create_buffer(distances);
        
        // 复制当前的距离值
        let mut new_distances = distances.to_vec();
        let new_distances_buffer = self.create_buffer(&new_distances);
        
        // 创建命令缓冲区和编码器
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();
        
        // 设置计算管线状态
        encoder.set_compute_pipeline_state(&self.incremental_sssp_pipeline);
        
        // 设置参数
        encoder.set_buffer(0, Some(&affected_buffer), 0);
        
        let affected_count_u32 = affected_count as u32;
        encoder.set_bytes(1, std::mem::size_of::<u32>() as u64, &affected_count_u32 as *const u32 as *const c_void);
        
        encoder.set_buffer(2, Some(&offsets_buffer), 0);
        encoder.set_buffer(3, Some(&targets_buffer), 0);
        encoder.set_buffer(4, Some(&weights_buffer), 0);
        encoder.set_buffer(5, Some(&distances_buffer), 0);
        encoder.set_buffer(6, Some(&new_distances_buffer), 0);
        encoder.set_bytes(7, std::mem::size_of::<u32>() as u64, &vertex_count as *const u32 as *const c_void);
        
        // 调度线程
        let grid_size = MTLSize::new(affected_count, 1, 1);
        let threadgroup_size = MTLSize::new(self.incremental_sssp_pipeline.thread_execution_width() as u64, 1, 1);
        encoder.dispatch_threads(grid_size, threadgroup_size);
        
        // 执行
        encoder.end_encoding();
        command_buffer.commit();
        command_buffer.wait_until_completed();
        
        // 读取结果
        let result_ptr = new_distances_buffer.contents() as *const f32;
        let result_slice = unsafe {
            std::slice::from_raw_parts(result_ptr, vertex_count as usize)
        };
        new_distances.copy_from_slice(result_slice);
        
        new_distances
    }
    
    /// 计算 BFS 使用 EdgeBlock 格式（GPU 加速）
    /// 
    /// 对应 Swift 版本的 `bfs_edgeblock` kernel
    pub fn compute_bfs_edgeblock(
        &self,
        gpu_edge_block: &crate::gpu_edge_block::GPUEdgeBlockGraph,
        source: u32,
    ) -> Vec<u32> {
        let vertex_count = gpu_edge_block.vertex_count as usize;
        
        // 初始化距离数组
        let mut distances = vec![u32::MAX; vertex_count];
        distances[source as usize] = 0;
        
        // 初始化访问标记
        let mut visited = vec![0u32; vertex_count];
        visited[source as usize] = 1;
        
        // 初始化前驱
        let mut frontier = vec![source];
        
        // BFS 主循环
        while !frontier.is_empty() {
            let frontier_len = frontier.len() as u32;
            
            // 创建 GPU 缓冲区
            let frontier_buffer = self.create_buffer(&frontier[0..frontier_len as usize]);
            let vertices_buffer = self.create_buffer(&gpu_edge_block.vertices);
            let block_counts_buffer = self.create_buffer(&gpu_edge_block.block_counts);
            let blocks_buffer = self.create_buffer(&gpu_edge_block.blocks);
            
            let mut next_frontier = vec![0u32; vertex_count];
            let next_frontier_buffer = self.create_buffer(&next_frontier);
            
            let mut next_count = vec![0u32; 1];
            let next_count_buffer = self.create_buffer(&next_count);
            
            let mut visited_clone = visited.clone();
            let visited_buffer = self.create_buffer(&visited_clone);
            
            // 创建命令缓冲区和编码器
            let command_buffer = self.queue.new_command_buffer();
            let encoder = command_buffer.new_compute_command_encoder();
            
            // 设置计算管线状态
            encoder.set_compute_pipeline_state(&self.bfs_edgeblock_pipeline);
            
            // 设置参数
            encoder.set_buffer(0, Some(&vertices_buffer), 0);
            encoder.set_buffer(1, Some(&block_counts_buffer), 0);
            encoder.set_buffer(2, Some(&blocks_buffer), 0);
            encoder.set_buffer(3, Some(&frontier_buffer), 0);
            
            let frontier_len_u32 = frontier_len;
            encoder.set_bytes(4, std::mem::size_of::<u32>() as u64, &frontier_len_u32 as *const u32 as *const c_void);
            
            encoder.set_buffer(5, Some(&next_frontier_buffer), 0);
            encoder.set_buffer(6, Some(&next_count_buffer), 0);
            encoder.set_buffer(7, Some(&visited_buffer), 0);
            
            // 调度线程
            let grid_size = MTLSize::new(frontier_len as u64, 1, 1);
            let threadgroup_size = MTLSize::new(self.bfs_edgeblock_pipeline.thread_execution_width() as u64, 1, 1);
            encoder.dispatch_threads(grid_size, threadgroup_size);
            
            // 执行
            encoder.end_encoding();
            command_buffer.commit();
            command_buffer.wait_until_completed();
            
            // 读取结果
            let next_frontier_ptr = next_frontier_buffer.contents() as *const u32;
            let next_frontier_slice = unsafe {
                std::slice::from_raw_parts(next_frontier_ptr, vertex_count)
            };
            next_frontier.copy_from_slice(next_frontier_slice);
            
            let next_count_ptr = next_count_buffer.contents() as *const u32;
            let next_count_value = unsafe { *next_count_ptr };
            
            // 更新距离和访问标记
            let current_dist = distances[frontier[0] as usize] + 1;
            for i in 0..next_count_value as usize {
                let v = next_frontier[i];
                if visited[v as usize] == 0 {
                    distances[v as usize] = current_dist;
                    visited[v as usize] = 1;
                }
            }
            
            // 更新前驱
            frontier.clear();
            for i in 0..next_count_value as usize {
                frontier.push(next_frontier[i]);
            }
        }
        
        distances
    }
    
    /// 计算增量 BFS（使用 EdgeBlock 格式）
    /// 
    /// 与全量 BFS 的区别：
    /// - 初始距离数组不是全 MAX，而是传入的 initial_distances
    /// - 初始 frontier 是 affected_vertices（不是 [source]）
    pub fn compute_incremental_bfs_edgeblock(
        &self,
        gpu_edge_block: &crate::gpu_edge_block::GPUEdgeBlockGraph,
        initial_distances: &[u32],
        affected_vertices: &[u32],
    ) -> Vec<u32> {
        let vertex_count = gpu_edge_block.vertex_count as usize;
        
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
        
        // BFS 主循环
        while !frontier.is_empty() {
            let frontier_len = frontier.len() as u32;
            
            // 创建 GPU 缓冲区
            let frontier_buffer = self.create_buffer(&frontier[0..frontier_len as usize]);
            let vertices_buffer = self.create_buffer(&gpu_edge_block.vertices);
            let block_counts_buffer = self.create_buffer(&gpu_edge_block.block_counts);
            let blocks_buffer = self.create_buffer(&gpu_edge_block.blocks);
            
            let mut next_frontier = vec![0u32; vertex_count];
            let next_frontier_buffer = self.create_buffer(&next_frontier);
            
            let mut next_count = vec![0u32; 1];
            let next_count_buffer = self.create_buffer(&next_count);
            
            let mut visited_clone = visited.clone();
            let visited_buffer = self.create_buffer(&visited_clone);
            
            // 创建命令缓冲区和编码器
            let command_buffer = self.queue.new_command_buffer();
            let encoder = command_buffer.new_compute_command_encoder();
            
            // 设置计算管线状态
            encoder.set_compute_pipeline_state(&self.bfs_edgeblock_pipeline);
            
            // 设置参数
            encoder.set_buffer(0, Some(&vertices_buffer), 0);
            encoder.set_buffer(1, Some(&block_counts_buffer), 0);
            encoder.set_buffer(2, Some(&blocks_buffer), 0);
            encoder.set_buffer(3, Some(&frontier_buffer), 0);
            
            let frontier_len_u32 = frontier_len;
            encoder.set_bytes(4, std::mem::size_of::<u32>() as u64, &frontier_len_u32 as *const u32 as *const c_void);
            
            encoder.set_buffer(5, Some(&next_frontier_buffer), 0);
            encoder.set_buffer(6, Some(&next_count_buffer), 0);
            encoder.set_buffer(7, Some(&visited_buffer), 0);
            
            // 调度线程
            let grid_size = MTLSize::new(frontier_len as u64, 1, 1);
            let threadgroup_size = MTLSize::new(self.bfs_edgeblock_pipeline.thread_execution_width() as u64, 1, 1);
            encoder.dispatch_threads(grid_size, threadgroup_size);
            
            // 执行
            encoder.end_encoding();
            command_buffer.commit();
            command_buffer.wait_until_completed();
            
            // 读取结果
            let next_frontier_ptr = next_frontier_buffer.contents() as *const u32;
            let next_frontier_slice = unsafe {
                std::slice::from_raw_parts(next_frontier_ptr, vertex_count)
            };
            next_frontier.copy_from_slice(next_frontier_slice);
            
            let next_count_ptr = next_count_buffer.contents() as *const u32;
            let next_count_value = unsafe { *next_count_ptr };
            
            // 更新距离和访问标记
            let current_dist = if frontier.is_empty() {
                0
            } else {
                distances[frontier[0] as usize] + 1
            };
            
            for i in 0..next_count_value as usize {
                let v = next_frontier[i];
                if visited[v as usize] == 0 {
                    distances[v as usize] = current_dist;
                    visited[v as usize] = 1;
                }
            }
            
            // 更新 frontier
            frontier.clear();
            for i in 0..next_count_value as usize {
                frontier.push(next_frontier[i]);
            }
        }
        
        distances
    }
    
    /// 计算增量 PageRank（使用 EdgeBlock 格式）
    /// 
    /// 参数：
    /// - gpu_edge_block: GPU EdgeBlock 格式的图
    /// - pr: 当前 PR 值
    /// - affected_vertices: 受影响顶点列表
    /// - out_degrees: 每个顶点的出度
    /// - damping_factor: 阻尼因子
    /// 
    /// 返回：更新后的 PR 值
    pub fn compute_incremental_pagerank_edgeblock(
        &self,
        gpu_edge_block: &crate::gpu_edge_block::GPUEdgeBlockGraph,
        pr: &[f32],
        affected_vertices: &[u32],
        out_degrees: &[u32],
        damping_factor: f32,
    ) -> Vec<f32> {
        let affected_count = affected_vertices.len() as u64;
        let vertex_count = gpu_edge_block.vertex_count as usize;
        
        // 创建缓冲区
        let affected_buffer = self.create_buffer(affected_vertices);
        let reverse_vertices_buffer = self.create_buffer(&gpu_edge_block.reverse_vertices);
        let reverse_block_counts_buffer = self.create_buffer(&gpu_edge_block.reverse_block_counts);
        let reverse_blocks_buffer = self.create_buffer(&gpu_edge_block.reverse_blocks);
        let pr_buffer = self.create_buffer(pr);
        let out_degrees_buffer = self.create_buffer(out_degrees);
        
        // 复制当前的 PR 值
        let mut new_pr = pr.to_vec();
        let new_pr_buffer = self.create_buffer(&new_pr);
        
        // 创建命令缓冲区和编码器
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();
        
        // 设置计算管线状态
        encoder.set_compute_pipeline_state(&self.pagerank_edgeblock_pipeline);
        
        // 设置参数
        encoder.set_buffer(0, Some(&affected_buffer), 0);
        
        let affected_count_u32 = affected_count as u32;
        encoder.set_bytes(1, std::mem::size_of::<u32>() as u64, &affected_count_u32 as *const u32 as *const c_void);
        
        encoder.set_buffer(2, Some(&reverse_vertices_buffer), 0);
        encoder.set_buffer(3, Some(&reverse_block_counts_buffer), 0);
        encoder.set_buffer(4, Some(&reverse_blocks_buffer), 0);
        encoder.set_buffer(5, Some(&pr_buffer), 0);
        encoder.set_buffer(6, Some(&new_pr_buffer), 0);
        encoder.set_buffer(7, Some(&out_degrees_buffer), 0);
        
        encoder.set_bytes(8, std::mem::size_of::<f32>() as u64, &damping_factor as *const f32 as *const c_void);
        
        let vertex_count_u32 = vertex_count as u32;
        encoder.set_bytes(9, std::mem::size_of::<u32>() as u64, &vertex_count_u32 as *const u32 as *const c_void);
        
        // 调度线程
        let grid_size = MTLSize::new(affected_count, 1, 1);
        let threadgroup_size = MTLSize::new(self.pagerank_edgeblock_pipeline.thread_execution_width() as u64, 1, 1);
        encoder.dispatch_threads(grid_size, threadgroup_size);
        
        // 执行
        encoder.end_encoding();
        command_buffer.commit();
        command_buffer.wait_until_completed();
        
        // 读取结果
        let result_ptr = new_pr_buffer.contents() as *const f32;
        let result_slice = unsafe {
            std::slice::from_raw_parts(result_ptr, vertex_count)
        };
        new_pr.copy_from_slice(result_slice);
        
        new_pr
    }
    
    /// 计算增量 SSSP（使用 EdgeBlock 格式，无权图）
    /// 
    /// 参数：
    /// - gpu_edge_block: GPU EdgeBlock 格式的图
    /// - distances: 当前距离数组（u32，跳数）
    /// - affected_vertices: 受影响顶点列表
    /// 
    /// 返回：更新后的距离数组
    pub fn compute_incremental_sssp_edgeblock(
        &self,
        gpu_edge_block: &crate::gpu_edge_block::GPUEdgeBlockGraph,
        distances: &[u32],
        affected_vertices: &[u32],
    ) -> Vec<u32> {
        let affected_count = affected_vertices.len() as u64;
        let vertex_count = gpu_edge_block.vertex_count as usize;
        
        // 创建缓冲区
        let affected_buffer = self.create_buffer(affected_vertices);
        let vertices_buffer = self.create_buffer(&gpu_edge_block.vertices);
        let block_counts_buffer = self.create_buffer(&gpu_edge_block.block_counts);
        let blocks_buffer = self.create_buffer(&gpu_edge_block.blocks);
        let distances_buffer = self.create_buffer(distances);
        
        // 复制当前的距离值
        let mut new_distances = distances.to_vec();
        let new_distances_buffer = self.create_buffer(&new_distances);
        
        // 创建命令缓冲区和编码器
        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();
        
        // 设置计算管线状态
        encoder.set_compute_pipeline_state(&self.sssp_edgeblock_pipeline);
        
        // 设置参数
        encoder.set_buffer(0, Some(&affected_buffer), 0);
        
        let affected_count_u32 = affected_count as u32;
        encoder.set_bytes(1, std::mem::size_of::<u32>() as u64, &affected_count_u32 as *const u32 as *const c_void);
        
        encoder.set_buffer(2, Some(&vertices_buffer), 0);
        encoder.set_buffer(3, Some(&block_counts_buffer), 0);
        encoder.set_buffer(4, Some(&blocks_buffer), 0);
        encoder.set_buffer(5, Some(&distances_buffer), 0);
        encoder.set_buffer(6, Some(&new_distances_buffer), 0);
        
        let vertex_count_u32 = vertex_count as u32;
        encoder.set_bytes(7, std::mem::size_of::<u32>() as u64, &vertex_count_u32 as *const u32 as *const c_void);
        
        // 调度线程
        let grid_size = MTLSize::new(affected_count, 1, 1);
        let threadgroup_size = MTLSize::new(self.sssp_edgeblock_pipeline.thread_execution_width() as u64, 1, 1);
        encoder.dispatch_threads(grid_size, threadgroup_size);
        
        // 执行
        encoder.end_encoding();
        command_buffer.commit();
        command_buffer.wait_until_completed();
        
        // 读取结果
        let result_ptr = new_distances_buffer.contents() as *const u32;
        let result_slice = unsafe {
            std::slice::from_raw_parts(result_ptr, vertex_count)
        };
        new_distances.copy_from_slice(result_slice);
        
        new_distances
    }
}
