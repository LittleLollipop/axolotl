// examples/test_metal_api.rs
// 最小可工作例子：测试 metal-rs 的 API（向量加法）

use metal::*;
use std::ffi::c_void;

fn main() {
    println!("=== 测试 metal-rs API ===\n");

    // 1. 创建 Metal 设备
    println!("1. 创建 Metal 设备...");
    let device = Device::system_default().expect("No Metal device found");
    println!("   ✓ 设备创建成功: {}", device.name());

    // 2. 创建命令队列
    println!("\n2. 创建命令队列...");
    let queue = device.new_command_queue();
    println!("   ✓ 命令队列创建成功");

    // 3. 创建 GPU 内核（向量加法）
    println!("\n3. 创建 GPU 内核（向量加法）...");
    let kernel_source = "
        #include <metal_stdlib>
        using namespace metal;
        
        // 向量加法：C[i] = A[i] + B[i]
        kernel void vector_add(
            device const float* A [[buffer(0)]],
            device const float* B [[buffer(1)]],
            device float* C [[buffer(2)]],
            uint id [[thread_position_in_grid]]
        ) {
            C[id] = A[id] + B[id];
        }
    ";

    let options = CompileOptions::new();
    let library = device
        .new_library_with_source(kernel_source, &options)
        .expect("Failed to create Metal library");
    println!("   ✓ Metal 库创建成功");

    let kernel = library
        .get_function("vector_add", None)
        .expect("Failed to get kernel function");
    println!("   ✓ 内核函数获取成功");

    let pipeline = device
        .new_compute_pipeline_state_with_function(&kernel)
        .expect("Failed to create compute pipeline");
    println!("   ✓ 计算管线创建成功");

    // 4. 准备数据
    println!("\n4. 准备数据...");
    let n = 10; // 向量长度
    let mut a: Vec<f32> = (0..n).map(|i| i as f32).collect();
    let mut b: Vec<f32> = (0..n).map(|i| (i * 2) as f32).collect();
    let mut c = vec![0.0f32; n as usize];

    println!("   A = {:?}", a);
    println!("   B = {:?}", b);
    println!("   C = {:?}", c);

    // 5. 创建 Metal 缓冲区
    println!("\n5. 创建 Metal 缓冲区...");
    let buffer_a = device.new_buffer_with_data(
        a.as_ptr() as *const c_void,
        (n as usize * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache,
    );
    let buffer_b = device.new_buffer_with_data(
        b.as_ptr() as *const c_void,
        (n as usize * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache,
    );
    let buffer_c = device.new_buffer_with_data(
        c.as_ptr() as *const c_void,
        (n as usize * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache,
    );
    println!("   ✓ 缓冲区创建成功");

    // 6. 创建命令缓冲区并编码
    println!("\n6. 编码 GPU 命令...");
    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_compute_command_encoder();

    encoder.set_buffer(0, Some(&buffer_a), 0);
    encoder.set_buffer(1, Some(&buffer_b), 0);
    encoder.set_buffer(2, Some(&buffer_c), 0);

    // 调度线程
    let grid_size = MTLSize::new(n as u64, 1, 1);
    let threadgroup_size = MTLSize::new(n as u64, 1, 1); // 简化：1 个线程组
    encoder.dispatch_threads(grid_size, threadgroup_size);

    encoder.end_encoding();
    println!("   ✓ 命令编码成功");

    // 7. 执行并等待完成
    println!("\n7. 执行 GPU 计算...");
    command_buffer.commit();
    command_buffer.wait_until_completed();
    println!("   ✓ GPU 计算完成");

    // 8. 读取结果
    println!("\n8. 读取结果...");
    let result_ptr = buffer_c.contents() as *const f32;
    let result_slice = unsafe { std::slice::from_raw_parts(result_ptr, n as usize) };
    c.copy_from_slice(result_slice);

    println!("   C = {:?}", c);

    // 9. 验证结果
    println!("\n9. 验证结果...");
    let mut all_correct = true;
    for i in 0..n as usize {
        let expected = a[i] + b[i];
        if (c[i] - expected).abs() > 1e-6 {
            println!("   ✗ C[{}] = {}，期望 {}", i, c[i], expected);
            all_correct = false;
        }
    }

    if all_correct {
        println!("   ✓ 所有结果正确！");
    }

    println!("\n=== 测试完成 ===");
}
