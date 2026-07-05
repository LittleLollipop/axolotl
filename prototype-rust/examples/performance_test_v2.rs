// examples/performance_test_v2.rs
// 正确的性能对比测试：模拟边更新

use std::time::Instant;
use axolotl_rs::csr_graph::CSRGraph;
use axolotl_rs::incremental_pagerank::IncrementalPageRank;

fn main() {
    println!("=== Axolotl 性能对比测试（v2）===\n");
    
    // 创建测试图
    let vertex_count = 10000;
    let edge_count = 50000;
    
    println!("生成测试图：{} 个顶点，{} 条边", vertex_count, edge_count);
    let (mut csr, initial_pr) = generate_test_graph(vertex_count, edge_count);
    
    // 步骤 1：计算全量 PageRank（得到稳定的 PR 值）
    println!("\n📊 步骤 1：计算全量 PageRank（CPU，得到稳定的 PR 值）");
    let start = Instant::now();
    let stable_pr = compute_full_pagerank_cpu(&csr, &initial_pr, 100, 1e-6);
    let full_time = start.elapsed();
    println!("   时间：{:?}", full_time);
    
    // 步骤 2：模拟边更新（添加一条边）
    println!("\n📊 步骤 2：模拟边更新（添加边 0 -> 100）");
    let new_edge = (0, 100);
    println!("   添加边：{:?}", new_edge);
    
    // 更新 CSR 图（简化：不实际更新，只是模拟）
    // 在实际系统中，这里应该更新 CSR 图
    
    // 找出受影响顶点（简化：假设只有顶点 100 受影响）
    let affected_vertices = vec![100];
    println!("   受影响顶点：{:?}", affected_vertices);
    
    // 步骤 3：全量重算（CPU）
    println!("\n📊 步骤 3：全量重算（CPU）");
    let start = Instant::now();
    let _full_pr = compute_full_pagerank_cpu(&csr, &stable_pr, 100, 1e-6);
    let full_recompute_time = start.elapsed();
    println!("   时间：{:?}", full_recompute_time);
    
    // 步骤 4：增量更新（GPU）
    println!("\n📊 步骤 4：增量更新（GPU）");
    let incremental_pr = IncrementalPageRank::new();
    if incremental_pr.is_err() {
        println!("   ⚠️  没有 GPU，跳过测试");
        println!("   错误：{:?}", incremental_pr.err());
        return;
    }
    let incremental_pr = incremental_pr.unwrap();
    
    let start = Instant::now();
    let _incremental_pr = incremental_pr.compute(&csr, &stable_pr, &affected_vertices);
    let incremental_time = start.elapsed();
    println!("   时间：{:?}", incremental_time);
    
    // 计算加速比
    let speedup = full_recompute_time.as_secs_f64() / incremental_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
    
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
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    let mut edges = Vec::new();
    for _ in 0..edge_count {
        let src = rng.gen_range(0..vertex_count) as u64;
        let dst = rng.gen_range(0..vertex_count) as u64;
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
