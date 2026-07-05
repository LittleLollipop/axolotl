// examples/performance_large.rs
// 大规模图性能测试

use std::time::Instant;
use axolotl_rs::csr_graph::CSRGraph;
use axolotl_rs::incremental_pagerank::IncrementalPageRank;

fn main() {
    println!("=== Axolotl 大规模图性能测试 ===\n");
    
    // 测试不同规模的图
    let test_cases = vec![
        (100_000, 500_000),    // 100K 顶点，500K 边
        (500_000, 2_500_000),  // 500K 顶点，2.5M 边
        (1_000_000, 5_000_000), // 1M 顶点，5M 边（如果内存允许）
    ];
    
    for (vertex_count, edge_count) in test_cases {
        println!("\n{:-^60}", "");
        println!("📊 测试图规模：{} 顶点，{} 边", vertex_count, edge_count);
        
        // 生成测试图
        println!("\n生成测试图...");
        let start = Instant::now();
        let (csr, initial_pr) = generate_large_graph(vertex_count, edge_count);
        let gen_time = start.elapsed();
        println!("   生成时间：{:?}", gen_time);
        
        // 检查内存使用
        let mem_usage = (vertex_count * 4 * 2 + edge_count * 4) / 1024 / 1024; // MB
        println!("   预估内存使用：{} MB", mem_usage);
        
        // 测试全量 PageRank（CPU）
        println!("\n1. 全量 PageRank（CPU）");
        let start = Instant::now();
        let _full_pr = compute_full_pagerank_cpu(&csr, &initial_pr, 20, 1e-6);
        let cpu_time = start.elapsed();
        println!("   时间：{:?}", cpu_time);
        
        // 测试增量 PageRank（GPU）
        println!("\n2. 增量 PageRank（GPU）");
        let incremental_pr = IncrementalPageRank::new();
        if incremental_pr.is_err() {
            println!("   ⚠️  没有 GPU，跳过测试");
            continue;
        }
        let incremental_pr = incremental_pr.unwrap();
        
        // 模拟边更新：随机选择 100 个顶点作为受影响顶点
        let affected_vertices: Vec<u32> = (0..100).collect();
        
        let start = Instant::now();
        let _gpu_pr = incremental_pr.compute(&csr, &initial_pr, &affected_vertices);
        let gpu_time = start.elapsed();
        println!("   受影响顶点数：{}", affected_vertices.len());
        println!("   时间：{:?}", gpu_time);
        
        // 计算加速比
        let speedup = cpu_time.as_secs_f64() / gpu_time.as_secs_f64();
        println!("\n🚀 加速比：{:.2}x", speedup);
        
        // 如果加速比 < 1.0，说明 GPU 没有优势
        if speedup < 1.0 {
            println!("   ⚠️  GPU 加速未生效（加速比 < 1.0）");
        } else {
            println!("   ✅ GPU 加速生效！");
        }
    }
    
    println!("\n=== 测试完成 ===");
}

/// 生成大规模测试图
fn generate_large_graph(vertex_count: usize, edge_count: usize) -> (CSRGraph, Vec<f32>) {
    let mut csr = CSRGraph::new();
    
    // 添加顶点
    for i in 0..vertex_count {
        let mut props = std::collections::HashMap::new();
        props.insert("id".to_string(), axolotl_rs::csr_graph::PropertyValue::Int(i as i64));
        csr.add_vertex(i as u64, props);
    }
    
    // 添加随机边（使用简单的线性同余生成器，避免依赖 rand）
    let mut edges = Vec::with_capacity(edge_count);
    let mut seed = 12345u64;
    
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
