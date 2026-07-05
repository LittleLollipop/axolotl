// examples/gpu_vs_cpu.rs
// 对比 GPU 全量计算 vs CPU 全量计算的性能

use std::time::Instant;
use axolotl_rs::csr_graph::CSRGraph;

fn main() {
    println!("=== GPU vs CPU 全量计算性能对比 ===\n");
    
    // 创建测试图（100K 顶点，500K 边）
    let vertex_count = 100_000;
    let edge_count = 500_000;
    
    println!("生成测试图：{} 个顶点，{} 条边", vertex_count, edge_count);
    let (csr, initial_pr) = generate_test_graph(vertex_count, edge_count);
    
    // 测试 1：CPU 全量 PageRank（优化版本：使用 rayon 并行）
    println!("\n📊 测试 1：CPU 全量 PageRank（单线程）");
    let start = Instant::now();
    let cpu_pr = compute_full_pagerank_cpu_single(&csr, &initial_pr, 50, 1e-6);
    let cpu_time = start.elapsed();
    println!("   时间：{:?}", cpu_time);
    
    // 测试 2：CPU 全量 PageRank（多线程）
    println!("\n📊 测试 2：CPU 全量 PageRank（多线程 rayon）");
    let start = Instant::now();
    let cpu_parallel_pr = compute_full_pagerank_cpu_parallel(&csr, &initial_pr, 50, 1e-6);
    let cpu_parallel_time = start.elapsed();
    println!("   时间：{:?}", cpu_parallel_time);
    
    // 测试 3：GPU 全量 PageRank（传入所有顶点作为受影响顶点）
    println!("\n📊 测试 3：GPU 全量 PageRank");
    let incremental_pr = axolotl_rs::incremental_pagerank::IncrementalPageRank::new();
    if incremental_pr.is_err() {
        println!("   ⚠️  没有 GPU，跳过测试");
        return;
    }
    let incremental_pr = incremental_pr.unwrap();
    
    // 传入所有顶点（相当于全量计算）
    let all_vertices: Vec<u32> = (0..vertex_count as u32).collect();
    
    let start = Instant::now();
    let gpu_pr = incremental_pr.compute(&csr, &initial_pr, &all_vertices);
    let gpu_time = start.elapsed();
    println!("   时间：{:?}", gpu_time);
    
    // 计算加速比
    let speedup = cpu_time.as_secs_f64() / gpu_time.as_secs_f64();
    println!("\n🚀 GPU 加速比：{:.2}x", speedup);
    
    if speedup > 1.0 {
        println!("   ✅ GPU 加速生效！");
    } else {
        println!("   ⚠️  GPU 加速未生效（CPU 更快）");
    }
    
    // 验证结果一致性
    println!("\n📋 验证结果一致性...");
    let mut max_diff = 0.0f32;
    for i in 0..vertex_count {
        let diff = (cpu_pr[i] - gpu_pr[i]).abs();
        if diff > max_diff {
            max_diff = diff;
        }
    }
    println!("   最大差异：{:.6}", max_diff);
    
    if max_diff < 1e-3 {
        println!("   ✅ 结果一致");
    } else {
        println!("   ❌ 结果不一致！");
    }
    
    println!("\n=== 测试完成 ===");
}

/// 生成测试图
fn generate_test_graph(vertex_count: usize, edge_count: usize) -> (CSRGraph, Vec<f32>) {
    let mut csr = CSRGraph::new();
    
    // 添加顶点
    for i in 0..vertex_count {
        let mut props = std::collections::HashMap::new();
        props.insert("id".to_string(), axolotl_rs::csr_graph::PropertyValue::Int(i as i64));
        csr.add_vertex(i as u64, props);
    }
    
    // 添加随机边
    let mut seed = 12345u64;
    let mut edges = Vec::with_capacity(edge_count);
    
    for _ in 0..edge_count {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let src = (seed >> 32) % vertex_count as u64;
        
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let dst = (seed >> 32) % vertex_count as u64;
        
        if src != dst {
            edges.push((src, dst));
        }
    }
    
    csr.build_csr(&edges);
    
    // 初始 PR 值
    let initial_pr = vec![1.0 / vertex_count as f32; vertex_count];
    
    (csr, initial_pr)
}

/// 计算全量 PageRank（CPU 版本）
fn compute_full_pagerank_cpu(
    csr: &CSRGraph,
    initial_pr: &[f32],
    max_iterations: usize,
    tolerance: f32,
) -> Vec<f32> {
    let vertex_count = csr.vertex_count as usize;
    let damping = 0.85f32;
    let teleportation = (1.0 - damping) / vertex_count as f32;
    
    let mut pr = initial_pr.to_vec();
    let mut new_pr = vec![0.0; vertex_count];
    
    for _ in 0..max_iterations {
        // 初始化 new_pr
        for i in 0..vertex_count {
            new_pr[i] = teleportation;
        }
        
        // 计算每个顶点的贡献
        for v in 0..vertex_count {
            let start = csr.offsets[v] as usize;
            let end = csr.offsets[v + 1] as usize;
            let out_degree = end - start;
            
            if out_degree > 0 {
                let contribution = damping * pr[v] / out_degree as f32;
                for i in start..end {
                    let neighbor = csr.targets[i] as usize;
                    new_pr[neighbor] += contribution;
                }
            }
        }
        
        // 检查收敛
        let mut converged = true;
        for i in 0..vertex_count {
            if (new_pr[i] - pr[i]).abs() > tolerance {
                converged = false;
                break;
            }
        }
        
        pr.copy_from_slice(&new_pr);
        
        if converged {
            break;
        }
    }
    
    pr
}
