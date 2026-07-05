// 简化的 GPU f32 vs f64 性能测试
// 直接在你项目里编译运行

use std::time::Instant;
use metal::*;

fn main() {
    let device = Device::system_default().expect("No GPU");
    let queue = device.new_command_queue();
    let opts = CompileOptions::new();

    // --- f32 内核 ---
    let src_f32 = r#"
        kernel void pagerank_f32(
            device const float* pr [[buffer(0)]],
            device const uint* offsets [[buffer(1)]],
            device const uint* targets [[buffer(2)]],
            device const uint* out_degrees [[buffer(3)]],
            device float* new_pr [[buffer(4)]],
            constant uint& vertex_count [[buffer(5)]],
            uint gid [[thread_position_in_grid]]
        ) {
            if (gid >= vertex_count) return;
            float contrib = 0.0;
            uint start = offsets[gid];
            uint end = offsets[gid + 1];
            for (uint i = start; i < end; i++) {
                uint src = targets[i];
                contrib += pr[src] / (float)out_degrees[src];
            }
            new_pr[gid] = 0.15 / (float)vertex_count + 0.85 * contrib;
        }
    "#;

    // --- f64 内核 ---
    let src_f64 = r#"
        kernel void pagerank_f64(
            device const double* pr [[buffer(0)]],
            device const uint* offsets [[buffer(1)]],
            device const uint* targets [[buffer(2)]],
            device const uint* out_degrees [[buffer(3)]],
            device double* new_pr [[buffer(4)]],
            constant uint& vertex_count [[buffer(5)]],
            uint gid [[thread_position_in_grid]]
        ) {
            if (gid >= vertex_count) return;
            double contrib = 0.0;
            uint start = offsets[gid];
            uint end = offsets[gid + 1];
            for (uint i = start; i < end; i++) {
                uint src = targets[i];
                contrib += pr[src] / (double)out_degrees[src];
            }
            new_pr[gid] = 0.15 / (double)vertex_count + 0.85 * contrib;
        }
    "#;

    let lib_f32 = device.new_library_with_source(src_f32, &opts).unwrap();
    let lib_f64 = device.new_library_with_source(src_f64, &opts).unwrap();
    let func_f32 = lib_f32.get_function("pagerank_f32", None).unwrap();
    let func_f64 = lib_f64.get_function("pagerank_f64", None).unwrap();
    let pipe_f32 = device.new_compute_pipeline_state_with_function(&func_f32).unwrap();
    let pipe_f64 = device.new_compute_pipeline_state_with_function(&func_f64).unwrap();

    // 构建模拟的 10K 顶点图数据
    let vertex_count = 10000usize;
    let edges_per_vertex = 10u32;
    let total_edges = vertex_count * edges_per_vertex as usize;

    let mut offsets = vec![0u32; vertex_count + 1];
    let mut targets = vec![0u32; total_edges];
    let mut out_degrees = vec![edges_per_vertex; vertex_count];

    for i in 0..vertex_count {
        offsets[i + 1] = offsets[i] + edges_per_vertex;
        for j in 0..edges_per_vertex {
            targets[offsets[i] as usize + j as usize] =
                ((i * 7 + j as usize * 13) % vertex_count) as u32;
        }
    }

    // 创建 f32 缓冲区
    let pr_f32: Vec<f32> = (0..vertex_count).map(|i| 1.0 / vertex_count as f32).collect();
    let new_pr_f32: Vec<f32> = vec![0.0; vertex_count];
    let buf_offsets = device.new_buffer_with_data(
        offsets.as_ptr() as *const _,
        ((vertex_count + 1) * 4) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let buf_targets = device.new_buffer_with_data(
        targets.as_ptr() as *const _,
        (total_edges * 4) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let buf_out_degrees = device.new_buffer_with_data(
        out_degrees.as_ptr() as *const _,
        (vertex_count * 4) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let buf_pr_f32 = device.new_buffer_with_data(
        pr_f32.as_ptr() as *const _,
        (vertex_count * 4) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let buf_new_pr_f32 = device.new_buffer_with_data(
        new_pr_f32.as_ptr() as *const _,
        (vertex_count * 4) as u64,
        MTLResourceOptions::StorageModeShared,
    );

    // 创建 f64 缓冲区
    let pr_f64: Vec<f64> = (0..vertex_count).map(|i| 1.0 / vertex_count as f64).collect();
    let new_pr_f64: Vec<f64> = vec![0.0; vertex_count];
    let buf_pr_f64 = device.new_buffer_with_data(
        pr_f64.as_ptr() as *const _,
        (vertex_count * 8) as u64,
        MTLResourceOptions::StorageModeShared,
    );
    let buf_new_pr_f64 = device.new_buffer_with_data(
        new_pr_f64.as_ptr() as *const _,
        (vertex_count * 8) as u64,
        MTLResourceOptions::StorageModeShared,
    );

    let vc = vertex_count as u32;
    let grid = MTLSize::new(vertex_count as u64, 1, 1);
    let tg = MTLSize::new(pipe_f32.thread_execution_width() as u64, 1, 1);
    let iterations = 50;

    // --- 测试 f32 ---
    let start = Instant::now();
    for _ in 0..iterations {
        let cmd = queue.new_command_buffer();
        let enc = cmd.new_compute_command_encoder();
        enc.set_compute_pipeline_state(&pipe_f32);
        enc.set_buffer(0, Some(&buf_pr_f32), 0);
        enc.set_buffer(1, Some(&buf_offsets), 0);
        enc.set_buffer(2, Some(&buf_targets), 0);
        enc.set_buffer(3, Some(&buf_out_degrees), 0);
        enc.set_buffer(4, Some(&buf_new_pr_f32), 0);
        enc.set_bytes(5, 4, &vc as *const _ as *const _);
        enc.dispatch_threads(grid, tg);
        enc.end_encoding();
        cmd.commit();
        cmd.wait_until_completed();
    }
    let elapsed_f32 = start.elapsed();

    // --- 测试 f64 ---
    let start = Instant::now();
    for _ in 0..iterations {
        let cmd = queue.new_command_buffer();
        let enc = cmd.new_compute_command_encoder();
        enc.set_compute_pipeline_state(&pipe_f64);
        enc.set_buffer(0, Some(&buf_pr_f64), 0);
        enc.set_buffer(1, Some(&buf_offsets), 0);
        enc.set_buffer(2, Some(&buf_targets), 0);
        enc.set_buffer(3, Some(&buf_out_degrees), 0);
        enc.set_buffer(4, Some(&buf_new_pr_f64), 0);
        enc.set_bytes(5, 4, &vc as *const _ as *const _);
        enc.dispatch_threads(grid, tg);
        enc.end_encoding();
        cmd.commit();
        cmd.wait_until_completed();
    }
    let elapsed_f64 = start.elapsed();

    println!("=== GPU PageRank f32 vs f64 实际性能 ===");
    println!("顶点数: {}", vertex_count);
    println!("每个顶点入边数: {}", edges_per_vertex);
    println!("迭代次数: {}", iterations);
    println!();
    println!("f32: 总时间 {:.4}s,  平均 {:.4}ms/iter",
        elapsed_f32.as_secs_f64(),
        elapsed_f32.as_secs_f64() * 1000.0 / iterations as f64);
    println!("f64: 总时间 {:.4}s,  平均 {:.4}ms/iter",
        elapsed_f64.as_secs_f64(),
        elapsed_f64.as_secs_f64() * 1000.0 / iterations as f64);
    println!();
    let ratio = elapsed_f64.as_secs_f64() / elapsed_f32.as_secs_f64();
    println!("f64/f32 性能比: {:.2}x", ratio);
    println!();
    println!("结论: f64 {}",
        if ratio > 2.5 { "明显更慢 (>2.5x)".to_string() }
        else if ratio > 1.5 { format!("较慢 ({:.1}x)", ratio) }
        else { format!("接近 f32 ({:.1}x)", ratio) }
    );
}
