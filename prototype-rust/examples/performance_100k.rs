// examples/performance_100k.rs
// 10 万顶点性能测试（快速查看结果）

use std::time::Instant;

fn main() {
    println!("=== Axolotl 10 万顶点性能测试 ===\n");
    
    let vertex_count = 100_000;
    let edge_count = 500_000;
    let affected_count = 1000;
    
    println!("测试规模：");
    println!("  顶点数：{}", vertex_count);
    println!("  边数：{}", edge_count);
    println!("  受影响顶点数：{}", affected_count);
    println!();
    
    // 生成测试图
    println!("生成测试图...");
    let start = Instant::now();
    let (csr, initial_pr) = generate_test_graph(vertex_count, edge_count);
    let gen_time = start.elapsed();
    println!("  生成时间：{:?}", gen_time);
    println!();
    
    // 测试 1：PageRank
    test_pagerank(&csr, &initial_pr, affected_count);
    
    // 测试 2：BFS
    test_bfs(&csr, affected_count);
    
    println!("\n=== 测试完成 ===");
    
    // 估算 100 万顶点的性能
    println!("\n📊 性能估算（100 万顶点）：");
    println!("  如果 10 万顶点耗时 T，则 100 万顶点可能耗时 10T - 100T");
    println!("  （取决于算法复杂度和内存访问模式）");
}

/// 测试 PageRank 性能
fn test_pagerank(
    csr: &axolotl_rs::csr_graph::CSRGraph,
    initial_pr: &[f32],
    affected_count: usize,
) {
    println!("{:-^60}", "");
    println!("📊 测试 1：PageRank（10 万顶点）\n");
    
    // 全量重算（CPU）
    println!("1. 全量重算（CPU）");
    let start = Instant::now();
    let _full_pr = compute_full_pagerank_cpu(csr, initial_pr, 10, 1e-6);
    let cpu_time = start.elapsed();
    println!("   时间：{:?}", cpu_time);
    
    // 增量更新（GPU）
    println!("\n2. 增量更新（GPU）");
    let incremental_pr = axolotl_rs::incremental_pagerank::IncrementalPageRank::new();
    if incremental_pr.is_err() {
        println!("   ⚠️  没有 GPU，跳过测试");
        return;
    }
    let incremental_pr = incremental_pr.unwrap();
    
    let affected_vertices: Vec<u32> = (0..affected_count as u32).collect();
    
    let start = Instant::now();
    let _gpu_pr = incremental_pr.compute(csr, initial_pr, &affected_vertices);
    let gpu_time = start.elapsed();
    println!("   受影响顶点数：{}", affected_vertices.len());
    println!("   时间：{:?}", gpu_time);
    
    // 加速比
    let speedup = cpu_time.as_secs_f64() / gpu_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
    
    if speedup > 1.0 {
        println!("   ✅ GPU 加速生效！");
    } else {
        println!("   ⚠️  GPU 加速未生效（加速比 < 1.0）");
    }
}

/// 测试 BFS 性能
fn test_bfs(
    csr: &axolotl_rs::csr_graph::CSRGraph,
    affected_count: usize,
) {
    println!("{:-^60}", "");
    println!("\n📊 测试 2：BFS（10 万顶点）\n");
    
    let source = 0;
    
    // 全量计算（CPU）
    println!("1. 全量计算（CPU）");
    let start = Instant::now();
    let _full_distances = compute_full_bfs_cpu(csr, source);
    let cpu_time = start.elapsed();
    println!("   时间：{:?}", cpu_time);
    
    // 增量更新（GPU）
    println!("\n2. 增量更新（GPU）");
    let incremental_bfs = axolotl_rs::incremental_bfs::IncrementalBFS::new();
    if incremental_bfs.is_err() {
        println!("   ⚠️  没有 GPU，跳过测试");
        return;
    }
    let incremental_bfs = incremental_bfs.unwrap();
    
    // 创建初始距离数组
    let mut initial_distances = vec![u32::MAX; csr.vertex_count as usize];
    initial_distances[source as usize] = 0;
    
    let affected_vertices: Vec<u32> = (0..affected_count as u32).collect();
    
    let start = Instant::now();
    let _gpu_distances = incremental_bfs.compute(csr, &initial_distances, &affected_vertices);
    let gpu_time = start.elapsed();
    println!("   受影响顶点数：{}", affected_vertices.len());
    println!("   时间：{:?}", gpu_time);
    
    // 加速比
    let speedup = cpu_time.as_secs_f64() / gpu_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
    
    if speedup > 1.0 {
        println!("   ✅ GPU 加速生效！");
    } else {
        println!("   ⚠️  GPU 加速未生效（加速比 < 1.0）");
    }
}

/// 生成测试图
fn generate_test_graph(vertex_count: usize, edge_count: usize) -> (axolotl_rs::csr_graph::CSRGraph, Vec<f32>) {
    let mut csr = axolotl_rs::csr_graph::CSRGraph::new();
    
    // 添加顶点
    for i in 0..vertex_count {
        let mut props = std::collections::HashMap::new();
        props.insert("id".to_string(), axolotl_rs::csr_graph::PropertyValue::Int(i as i64));
        csr.add_vertex(i as u64, props);
    }
    
    // 添加随机边
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
    csr: &axolotl_rs::csr_graph::CSRGraph,
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
fn compute_full_bfs_cpu(
    csr: &axolotl_rs::csr_graph::CSRGraph,
    source: u32,
) -> Vec<u32> {
    let vertex_count = csr.vertex_count as usize;
    let mut distances = vec![u32::MAX; vertex_count];
    distances[source as usize] = 0;
    
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(source);
    
    while let Some(v) = queue.pop_front() {
        let v_dist = distances[v as usize];
        let start = csr.offsets[v as usize] as usize;
        let end = csr.offsets[v as usize + 1] as usize;
        
        for i in start..end {
            let neighbor = csr.targets[i] as usize;
            if distances[neighbor] == u32::MAX {
                distances[neighbor] = v_dist + 1;
                queue.push_back(neighbor as u32);
            }
        }
    }
    
    distances
}
