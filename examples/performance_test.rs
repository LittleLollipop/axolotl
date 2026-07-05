// examples/performance_test.rs
// 性能对比测试：增量算法 vs 全量算法

use std::time::Instant;
use axolotl_rs::csr_graph::CSRGraph;
use axolotl_rs::incremental_pagerank::IncrementalPageRank;
use axolotl_rs::incremental_bfs::IncrementalBFS;

fn main() {
    println!("=== Axolotl 性能对比测试 ===\n");
    
    // 测试 1：PageRank
    test_pagerank_performance();
    
    println!("\n{:-^60}", "");
    
    // 测试 2：BFS
    test_bfs_performance();
    
    println!("\n=== 测试完成 ===");
}

/// 测试 PageRank 性能
fn test_pagerank_performance() {
    println!("📊 测试 1：PageRank 性能对比\n");
    
    // 创建测试图（10000 个顶点，50000 条边）
    let vertex_count = 10000;
    let edge_count = 50000;
    
    println!("生成测试图：{} 个顶点，{} 条边", vertex_count, edge_count);
    let (csr, initial_pr) = generate_test_graph(vertex_count, edge_count);
    
    // 测试全量 PageRank（CPU）
    println!("\n1. 全量 PageRank（CPU，迭代 50 次）");
    let start = Instant::now();
    let full_pr = compute_full_pagerank_cpu(&csr, &initial_pr, 50, 1e-6);
    let cpu_time = start.elapsed();
    println!("   时间：{:?}", cpu_time);
    
    // 测试增量 PageRank（GPU）- 传入所有顶点作为受影响顶点
    println!("\n2. 增量 PageRank（GPU，受影响顶点 = 所有顶点）");
    let incremental_pr = IncrementalPageRank::new();
    if incremental_pr.is_err() {
        println!("   ⚠️  没有 GPU，跳过测试");
        println!("   错误：{:?}", incremental_pr.err());
        return;
    }
    let incremental_pr = incremental_pr.unwrap();
    
    // 传入所有顶点作为受影响顶点（相当于全量计算）
    let affected_vertices: Vec<u32> = (0..vertex_count as u32).collect();
    
    let start = Instant::now();
    let gpu_pr = incremental_pr.compute(&csr, &initial_pr, &affected_vertices);
    let gpu_time = start.elapsed();
    println!("   受影响顶点数：{}", affected_vertices.len());
    println!("   时间：{:?}", gpu_time);
    
    // 计算加速比
    let speedup = cpu_time.as_secs_f64() / gpu_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
    
    // 验证结果一致性
    let mut max_diff = 0.0f32;
    for i in 0..vertex_count {
        let diff = (full_pr[i] - gpu_pr[i]).abs();
        if diff > max_diff {
            max_diff = diff;
        }
    }
    println!("   最大差异：{:.6}", max_diff);
}

/// 测试 BFS 性能
fn test_bfs_performance() {
    println!("📊 测试 2：BFS 性能对比\n");
    
    // 创建测试图（10000 个顶点，50000 条边）
    let vertex_count = 10000;
    let edge_count = 50000;
    
    println!("生成测试图：{} 个顶点，{} 条边", vertex_count, edge_count);
    let (csr, _) = generate_test_graph(vertex_count, edge_count);
    
    // 选择源顶点
    let source = 0;
    
    // 测试全量 BFS（CPU）
    println!("\n1. 全量 BFS（CPU）");
    let start = Instant::now();
    let full_distances = compute_full_bfs_cpu(&csr, source);
    let cpu_time = start.elapsed();
    println!("   时间：{:?}", cpu_time);
    
    // 测试增量 BFS（GPU）
    println!("\n2. 增量 BFS（GPU）");
    let incremental_bfs = IncrementalBFS::new();
    if incremental_bfs.is_err() {
        println!("   ⚠️  没有 GPU，跳过测试");
        println!("   错误：{:?}", incremental_bfs.err());
        return;
    }
    let incremental_bfs = incremental_bfs.unwrap();
    
    // 创建初始距离数组
    let mut initial_distances = vec![u32::MAX; vertex_count];
    initial_distances[source as usize] = 0;
    
    // 模拟边更新：随机选择 100 个顶点作为受影响顶点
    let affected_vertices: Vec<u32> = (0..100).collect();
    
    let start = Instant::now();
    let gpu_distances = incremental_bfs.compute(&csr, &initial_distances, &affected_vertices);
    let gpu_time = start.elapsed();
    println!("   受影响顶点数：{}", affected_vertices.len());
    println!("   时间：{:?}", gpu_time);
    
    // 计算加速比
    let speedup = cpu_time.as_secs_f64() / gpu_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
    
    // 验证结果一致性
    let mut correct = true;
    for i in 0..vertex_count {
        if full_distances[i] != gpu_distances[i] {
            if full_distances[i] != u32::MAX && gpu_distances[i] != u32::MAX {
                println!("   ❌ 顶点 {} 的距离不正确：CPU {}, GPU {}", 
                         i, full_distances[i], gpu_distances[i]);
                correct = false;
            }
        }
    }
    if correct {
        println!("   ✅ 结果一致");
    }
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

/// 计算全量 BFS（CPU 版本）
fn compute_full_bfs_cpu(csr: &CSRGraph, source: u32) -> Vec<u32> {
    let vertex_count = csr.vertex_count as usize;
    let mut distances = vec![u32::MAX; vertex_count];
    let mut visited = vec![false; vertex_count];
    let mut queue = std::collections::VecDeque::new();
    
    distances[source as usize] = 0;
    visited[source as usize] = true;
    queue.push_back(source);
    
    while let Some(v) = queue.pop_front() {
        let start = csr.offsets[v as usize] as usize;
        let end = csr.offsets[v as usize + 1] as usize;
        
        for i in start..end {
            let neighbor = csr.targets[i] as usize;
            if !visited[neighbor] {
                visited[neighbor] = true;
                distances[neighbor] = distances[v as usize] + 1;
                queue.push_back(neighbor as u32);
            }
        }
    }
    
    distances
}
