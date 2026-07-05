// examples/check_gpu_double_support.rs
// 检查 GPU 是否支持 double (f64)

use metal::Device;

fn main() {
    println!("=== 检查 GPU 是否支持 double ===\n");
    
    // 获取默认 GPU 设备
    let device = Device::system_default().expect("No GPU device found");
    
    println!("GPU 设备: {}", device.name());
    println!("是否支持 double: {}", device.supports_family(metal::MTLLanguageVersion::V1_0));
    
    // 检查特性
    println!("\nGPU 特性:");
    println!("- 最大线程组大小: {:?}", device.max_threads_per_threadgroup());
    println!("- 最大缓冲区长度: {}", device.max_buffer_length());
    
    // 尝试编译一个使用 double 的 Metal 内核
    let library = match device.new_library_with_source(
        "#include <metal_stdlib>\n\
         using namespace metal;\n\
         \n\
         kernel void test_double(device const double* a [[buffer(0)]], \n\
                                device double* b [[buffer(1)]], \n\
                                uint gid [[thread_position_in_grid]]) {\n\
             b[gid] = a[gid] * 2.0;\n\
         }",
        &metal::CompileOptions::new()
    ) {
        Ok(_) => {
            println!("\n✓ GPU 支持 double (编译成功)");
            true
        }
        Err(e) => {
            println!("\n✗ GPU 不支持 double (编译失败): {:?}", e);
            false
        }
    };
    
    if library {
        println!("\n可以使用 f64 (double) 进行 GPU 计算");
        println!("但性能可能比 f32 (float) 慢 2-4x");
    } else {
        println!("\n建议使用 f32 (float)，但改进数值稳定性");
        println!("例如：使用 Kahan summation 算法");
    }
}
