// examples/gpu_demo.rs
// 演示 GPU 加速的增量 PageRank

use metal::*;
use std::ffi::c_void;

fn main() {
    println!("=== GPU 增量 PageRank 演示 ===\n");
    
    // 1. 创建 Metal 设备
    let device = Device::system_default().expect("No Metal device found");
    println!("✓ 设备: {}", device.name());
    
    // 2. 创建命令队列
    let queue = device.new_command_queue();
    
    // 3. 创建增量 PageRank 的 GPU 内核
    let kernel_source = "
        #include <metal_stdlib>
        using namespace metal;
        
        // 增量 PageRank 内核（简化版：只计算一个迭代）
        kernel void incremental_pagerank(
            device const uint* affected_vertices [[buffer(0)]],
            constant uint& affected_count [[buffer(1)]],
            device const uint* csr_offsets [[buffer(2)]],
            device const uint* csr_targets [[buffer(3)]],
            device const float* pr [[buffer(4)]],
            device float* new_pr [[buffer(5)]],
            constant uint& vertex_count [[buffer(6)]],
            uint id [[thread_position_in_grid]]
        ) {
            if (id >= affected_count) return;
            
            // 计算新的 PR 值（简化：只从入边接收贡献）
            float new_value = 0.15; // teleportation factor
            
            // 遍历入边
            uint start = csr_offsets[affected_vertices[id]];
            uint end = csr_offsets[affected_vertices[id] + 1];
            
            for (uint i = start; i < end; i++) {
                // 简化：假设所有顶点的出度都是 2
                new_value += 0.85 * pr[csr_targets[i]] / 2.0;
            }
            
            new_pr[affected_vertices[id]] = new_value;
        }
    ";
    
    let options = CompileOptions::new();
    let library = device
        .new_library_with_source(kernel_source, &options)
        .expect("Failed to create library");
    let kernel = library
        .get_function("incremental_pagerank", None)
        .expect("Failed to get function");
    let pipeline = device
        .new_compute_pipeline_state_with_function(&kernel)
        .expect("Failed to create pipeline");
    
    println!("✓ GPU 内核创建成功");
    
    // 4. 准备测试数据（创建一个简单的图）
    // 图结构：0 -> 1 -> 2 -> 3 -> 4 -> 0 (闭环)
    let vertex_count = 5u32;
    
    // CSR 格式
    let csr_offsets: Vec<u32> = vec![0, 1, 2, 3, 4, 5]; // 每个顶点的边偏移
    let csr_targets: Vec<u32> = vec![1, 2, 3, 4, 0]; // 边的目标顶点
    
    // 初始 PR 值
    let pr: Vec<f32> = vec![0.2, 0.2, 0.2, 0.2, 0.2];
    
    // 受影响顶点（假设顶点 0 和 3 受影响）
    let affected_vertices: Vec<u32> = vec![0, 3];
    let affected_count = affected_vertices.len() as u32;
    
    println!("\n测试数据：");
    println!("  顶点数: {}", vertex_count);
    println!("  初始 PR: {:?}", pr);
    println!("  受影响顶点: {:?}", affected_vertices);
    
    // 5. 创建缓冲区
    let affected_buffer = device.new_buffer_with_data(
        affected_vertices.as_ptr() as *const c_void,
        (affected_vertices.len() * std::mem::size_of::<u32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    
    let offsets_buffer = device.new_buffer_with_data(
        csr_offsets.as_ptr() as *const c_void,
        (csr_offsets.len() * std::mem::size_of::<u32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    
    let targets_buffer = device.new_buffer_with_data(
        csr_targets.as_ptr() as *const c_void,
        (csr_targets.len() * std::mem::size_of::<u32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    
    let pr_buffer = device.new_buffer_with_data(
        pr.as_ptr() as *const c_void,
        (pr.len() * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    
    let mut new_pr = pr.clone(); // 用旧 PR 值初始化
    let new_pr_buffer = device.new_buffer_with_data(
        new_pr.as_ptr() as *const c_void,
        (new_pr.len() * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    
    println!("✓ 缓冲区创建成功");
    
    // 6. 编码命令
    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_compute_command_encoder();
    
    // ⚠️ 关键：设置计算管线状态
    encoder.set_compute_pipeline_state(&pipeline);
    
    encoder.set_buffer(0, Some(&affected_buffer), 0);
    encoder.set_bytes(1, std::mem::size_of::<u32>() as u64, &affected_count as *const u32 as *const c_void);
    encoder.set_buffer(2, Some(&offsets_buffer), 0);
    encoder.set_buffer(3, Some(&targets_buffer), 0);
    encoder.set_buffer(4, Some(&pr_buffer), 0);
    encoder.set_buffer(5, Some(&new_pr_buffer), 0);
    encoder.set_bytes(6, std::mem::size_of::<u32>() as u64, &vertex_count as *const u32 as *const c_void);
    
    // 调度线程
    let grid_size = MTLSize::new(affected_count as u64, 1, 1);
    let threadgroup_size = MTLSize::new(pipeline.thread_execution_width() as u64, 1, 1);
    encoder.dispatch_threads(grid_size, threadgroup_size);
    
    encoder.end_encoding();
    println!("✓ 命令编码成功");
    
    // 7. 执行
    command_buffer.commit();
    command_buffer.wait_until_completed();
    println!("✓ GPU 计算完成");
    
    // 8. 读取结果
    let result_ptr = new_pr_buffer.contents() as *const f32;
    let result_slice = unsafe { std::slice::from_raw_parts(result_ptr, vertex_count as usize) };
    new_pr.copy_from_slice(result_slice);
    
    println!("\nGPU 计算结果：");
    println!("  新的 PR 值: {:?}", new_pr);
    
    // 正确的验证方法：
    // 1. 复制旧 PR 值到新 PR 值
    // 2. GPU 只更新受影响顶点
    let mut expected_pr = pr.clone();
    for &vertex in &affected_vertices {
        let vertex = vertex as usize;
        let mut new_value = 0.15f32;
        
        let start = csr_offsets[vertex] as usize;
        let end = csr_offsets[vertex + 1] as usize;
        
        for i in start..end {
            let neighbor = csr_targets[i] as usize;
            new_value += 0.85 * pr[neighbor] / 2.0;
        }
        
        expected_pr[vertex] = new_value;
    }
    
    println!("  期望的 PR 值: {:?}", expected_pr);
    
    // 验证结果
    let mut all_correct = true;
    for i in 0..vertex_count as usize {
        if (new_pr[i] - expected_pr[i]).abs() > 1e-6 {
            println!("✗ 错误 at {}: GPU = {}, CPU = {}", i, new_pr[i], expected_pr[i]);
            all_correct = false;
        }
    }
    
    if all_correct {
        println!("\n✓ GPU 结果正确！");
    }
    
    println!("\n=== 演示完成 ===");
}
