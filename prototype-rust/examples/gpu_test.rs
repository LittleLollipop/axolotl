// examples/gpu_test.rs
// 简单的 GPU 测试（验证 Metal 是否能正常工作）

#[cfg(target_os = "macos")]
fn main() {
    use metal::*;
    use objc::*;
    
    // 创建 Metal 设备
    let device = Device::system_default().expect("需要 macOS 和 Metal 支持");
    println!("使用 GPU 设备: {}", device.name());
    
    // 创建命令队列
    let command_queue = device.new_command_queue();
    
    // 创建计算管道（简单的向量加法）
    let library = device.new_library_with_source(
        "
        #include <metal_stdlib>
        using namespace metal;
        
        kernel void vector_add(
            device const float* a [[buffer(0)]],
            device const float* b [[buffer(1)]],
            device float* result [[buffer(2)]],
            uint id [[thread_position_in_grid]]
        ) {
            result[id] = a[id] + b[id];
        }
        ",
        &CompileOptions::new()
    ).expect("编译 Metal 代码失败");
    
    let function = library.get_function("vector_add", None).expect("找不到 vector_add 函数");
    let pipeline = device.new_compute_pipeline_state_with_function(&function).expect("创建计算管道失败");
    
    // 创建测试数据
    let n = 1000000;
    let mut a = vec![0.0f32; n];
    let mut b = vec![0.0f32; n];
    let mut result = vec![0.0f32; n];
    
    for i in 0..n {
        a[i] = i as f32;
        b[i] = 2.0 * i as f32;
    }
    
    // 创建 GPU 缓冲区
    let a_buffer = device.new_buffer_with_data(
        a.as_ptr() as *const _,
        (n * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared
    );
    
    let b_buffer = device.new_buffer_with_data(
        b.as_ptr() as *const _,
        (n * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared
    );
    
    let result_buffer = device.new_buffer(
        (n * std::mem::size_of::<f32>()) as u64,
        MTLResourceOptions::StorageModeShared
    );
    
    // 创建命令缓冲区
    let command_buffer = command_queue.new_command_buffer();
    let compute_encoder = command_buffer.new_compute_command_encoder();
    
    compute_encoder.set_compute_pipeline_state(&pipeline);
    compute_encoder.set_buffer(0, Some(&a_buffer), 0);
    compute_encoder.set_buffer(1, Some(&b_buffer), 0);
    compute_encoder.set_buffer(2, Some(&result_buffer), 0);
    
    // 分发计算任务
    let threads_per_group = pipeline.max_total_threads_per_threadgroup();
    let num_groups = (n as u64 + threads_per_group - 1) / threads_per_group;
    
    compute_encoder.dispatch_thread_groups(
        metal::MTLSize::new(num_groups, 1, 1),
        metal::MTLSize::new(threads_per_group, 1, 1)
    );
    
    compute_encoder.end_encoding();
    command_buffer.commit();
    command_buffer.wait_until_completed();
    
    // 读取结果
    let result_ptr = result_buffer.contents() as *const f32;
    for i in 0..n {
        result[i] = unsafe { *result_ptr.add(i) };
    }
    
    // 验证结果
    let mut correct = true;
    for i in 0..10 {
        if (result[i] - (a[i] + b[i])).abs() > 1e-6 {
            correct = false;
            break;
        }
    }
    
    if correct {
        println!("✅ GPU 测试通过！向量加法正确。");
        println!("   计算了 {} 个元素", n);
    } else {
        println!("❌ GPU 测试失败！结果不正确。");
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("❌ GPU 支持仅在 macOS 上可用（需要 Metal）");
    println!("   当前系统不支持 GPU 加速。");
}

#[test]
#[cfg(target_os = "macos")]
fn test_gpu() {
    main();
}
