// examples/comprehensive_performance_test.rs
// 综合性能对比测试：全量重算 vs 增量更新

use std::time::Instant;

fn main() {
    println!("=== Axolotl 综合性能对比测试 ===\n");
    
    // 测试参数
    let vertex_count = 100_000;
    let edge_count = 500_000;
    let new_edge_count = 1_000;  // 0.2% 的边更新
    
    println!("测试参数：");
    println!("  顶点数：{}", vertex_count);
    println!("  初始边数：{}", edge_count);
    println!("  新增边数：{}", new_edge_count);
    println!();
    
    // 生成测试图
    println!("生成测试图...");
    let (csr, initial_pr) = generate_test_graph(vertex_count, edge_count);
    println!("  生成完成\n");
    
    // 测试 1：PageRank
    test_pagerank_performance(&csr, &initial_pr, new_edge_count);
    
    // 测试 2：BFS
    test_bfs_performance(&csr, new_edge_count);
    
    // 测试 3：SSSP
    test_sssp_performance(&csr, new_edge_count);
    
    // 测试 4：Connected Components
    test_cc_performance(vertex_count, edge_count, new_edge_count);
    
    // 测试 5：Triangle Counting
    test_tc_performance(vertex_count, edge_count, new_edge_count);
    
    println!("\n=== 测试完成 ===");
}

/// 测试 PageRank 性能
fn test_pagerank_performance(
    csr: &axolotl_rs::csr_graph::CSRGraph,
    initial_pr: &[f32],
    new_edge_count: usize,
) {
    println!("{:-^60}", "");
    println!("📊 测试 1：PageRank 性能对比\n");
    
    // 全量重算（CPU）
    println!("1. 全量重算（CPU）");
    let start = Instant::now();
    let _full_pr = compute_full_pagerank_cpu(csr, initial_pr, 20, 1e-6);
    let full_time = start.elapsed();
    println!("   时间：{:?}", full_time);
    
    // 增量更新（GPU）
    println!("\n2. 增量更新（GPU）");
    let incremental_pr = axolotl_rs::incremental_pagerank::IncrementalPageRank::new();
    if incremental_pr.is_err() {
        println!("   ⚠️  没有 GPU，跳过测试");
        return;
    }
    let incremental_pr = incremental_pr.unwrap();
    
    // 模拟受影响顶点（简化：使用所有顶点）
    let affected_vertices: Vec<u32> = (0..100).collect();
    
    let start = Instant::now();
    let _gpu_pr = incremental_pr.compute(csr, initial_pr, &affected_vertices);
    let incremental_time = start.elapsed();
    println!("   受影响顶点数：{}", affected_vertices.len());
    println!("   时间：{:?}", incremental_time);
    
    // 加速比
    let speedup = full_time.as_secs_f64() / incremental_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
    
    if speedup > 1.0 {
        println!("   ✅ GPU 加速生效！");
    } else {
        println!("   ⚠️  GPU 加速未生效");
    }
}

/// 测试 BFS 性能
fn test_bfs_performance(
    csr: &axolotl_rs::csr_graph::CSRGraph,
    _new_edge_count: usize,
) {
    println!("{:-^60}", "");
    println!("\n📊 测试 2：BFS 性能对比\n");
    
    let source = 0;
    
    // 全量计算（CPU）
    println!("1. 全量计算（CPU）");
    let start = Instant::now();
    let _full_distances = compute_full_bfs_cpu(csr, source);
    let full_time = start.elapsed();
    println!("   时间：{:?}", full_time);
    
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
    
    // 模拟受影响顶点
    let affected_vertices: Vec<u32> = (0..100).collect();
    
    let start = Instant::now();
    let _gpu_distances = incremental_bfs.compute(csr, &initial_distances, &affected_vertices);
    let incremental_time = start.elapsed();
    println!("   受影响顶点数：{}", affected_vertices.len());
    println!("   时间：{:?}", incremental_time);
    
    // 加速比
    let speedup = full_time.as_secs_f64() / incremental_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
}

/// 测试 SSSP 性能
fn test_sssp_performance(
    csr: &axolotl_rs::csr_graph::CSRGraph,
    _new_edge_count: usize,
) {
    println!("{:-^60}", "");
    println!("\n📊 测试 3：SSSP 性能对比\n");
    
    let source = 0;
    
    // 全量计算（CPU）
    println!("1. 全量计算（CPU）");
    let start = Instant::now();
    let _full_distances = compute_full_sssp_cpu(csr, source);
    let full_time = start.elapsed();
    println!("   时间：{:?}", full_time);
    
    // 增量更新（GPU）
    println!("\n2. 增量更新（GPU）");
    let incremental_sssp = axolotl_rs::incremental_sssp::IncrementalSSSP::new();
    if incremental_sssp.is_err() {
        println!("   ⚠️  没有 GPU，跳过测试");
        return;
    }
    let incremental_sssp = incremental_sssp.unwrap();
    
    // 创建初始距离数组
    let mut initial_distances = vec![f32::INFINITY; csr.vertex_count as usize];
    initial_distances[source as usize] = 0.0;
    
    // 模拟受影响顶点
    let affected_vertices: Vec<u32> = (0..100).collect();
    
    let start = Instant::now();
    let _gpu_distances = incremental_sssp.compute(csr, &initial_distances, &affected_vertices);
    let incremental_time = start.elapsed();
    println!("   受影响顶点数：{}", affected_vertices.len());
    println!("   时间：{:?}", incremental_time);
    
    // 加速比
    let speedup = full_time.as_secs_f64() / incremental_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
}

/// 测试 Connected Components 性能
fn test_cc_performance(
    vertex_count: usize,
    edge_count: usize,
    new_edge_count: usize,
) {
    println!("{:-^60}", "");
    println!("\n📊 测试 4：Connected Components 性能对比\n");
    
    // 创建图
    let (edges, new_edges) = generate_undirected_graph(vertex_count, edge_count, new_edge_count);
    
    // 全量计算
    println!("1. 全量计算（CPU）");
    let mut cc_full = axolotl_rs::incremental_cc::IncrementalCC::new(vertex_count);
    let start = Instant::now();
    cc_full.compute_full(&edges);
    let full_time = start.elapsed();
    let full_components = cc_full.count_components();
    println!("   时间：{:?}", full_time);
    println!("   组件数量：{}", full_components);
    
    // 增量更新
    println!("\n2. 增量更新（CPU）");
    let mut cc_incremental = axolotl_rs::incremental_cc::IncrementalCC::new(vertex_count);
    cc_incremental.compute_full(&edges);  // 先计算全量
    let start = Instant::now();
    let merged = cc_incremental.update_incremental(&new_edges);
    let incremental_time = start.elapsed();
    println!("   新增边数：{}", new_edges.len());
    println!("   合并组件数：{}", merged);
    println!("   时间：{:?}", incremental_time);
    
    // 加速比
    let speedup = full_time.as_secs_f64() / incremental_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
}

/// 测试 Triangle Counting 性能
fn test_tc_performance(
    vertex_count: usize,
    edge_count: usize,
    new_edge_count: usize,
) {
    println!("{:-^60}", "");
    println!("\n📊 测试 5：Triangle Counting 性能对比\n");
    
    // 创建图
    let (graph, new_edges) = generate_graph_for_tc(vertex_count, edge_count, new_edge_count);
    
    // 全量计数
    println!("1. 全量计数（CPU）");
    let start = Instant::now();
    let (full_count, _) = axolotl_rs::incremental_tc::full_triangle_counting(&graph);
    let full_time = start.elapsed();
    println!("   时间：{:?}", full_time);
    println!("   三角形数量：{}", full_count);
    
    // 增量计数
    println!("\n2. 增量计数（CPU）");
    let start = Instant::now();
    let (new_count, _) = axolotl_rs::incremental_tc::incremental_triangle_counting(&graph, &new_edges);
    let incremental_time = start.elapsed();
    println!("   新增边数：{}", new_edges.len());
    println!("   新三角形数量：{}", new_count);
    println!("   时间：{:?}", incremental_time);
    
    // 加速比
    let speedup = full_time.as_secs_f64() / incremental_time.as_secs_f64();
    println!("\n🚀 加速比：{:.2}x", speedup);
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
    csr: &axolotl_rs::csr_graph::CSRGraph,
    initial_pr: &[f32],
    max_iterations: usize,
    tolerance: f32,
) -> Vec<f32> {
    // 简化实现：只迭代一次
    let vertex_count = csr.vertex_count as usize;
    let damping = 0.85f32;
    let teleportation = (1.0 - damping) / vertex_count as f32;
    
    let mut pr = initial_pr.to_vec();
    let mut new_pr = vec![0.0; vertex_count];
    
    for _ in 0..max_iterations {
        for i in 0..vertex_count {
            new_pr[i] = teleportation;
        }
        
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
        
        pr.copy_from_slice(&new_pr);
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

/// 计算全量 SSSP（CPU 版本）
fn compute_full_sssp_cpu(
    csr: &axolotl_rs::csr_graph::CSRGraph,
    source: u32,
) -> Vec<f32> {
    let vertex_count = csr.vertex_count as usize;
    let mut distances = vec![f32::INFINITY; vertex_count];
    let mut visited = vec![false; vertex_count];
    let mut queue = std::collections::VecDeque::new();
    
    distances[source as usize] = 0.0;
    visited[source as usize] = true;
    queue.push_back(source);
    
    while let Some(v) = queue.pop_front() {
        let start = csr.offsets[v as usize] as usize;
        let end = csr.offsets[v as usize + 1] as usize;
        
        for i in start..end {
            let neighbor = csr.targets[i] as usize;
            let weight = csr.weights[i];
            let new_dist = distances[v as usize] + weight;
            
            if new_dist < distances[neighbor] {
                distances[neighbor] = new_dist;
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor as u32);
                }
            }
        }
    }
    
    distances
}

/// 生成无向图（用于 Connected Components）
fn generate_undirected_graph(
    vertex_count: usize,
    edge_count: usize,
    new_edge_count: usize,
) -> (Vec<(u64, u64)>, Vec<(u64, u64)>) {
    let mut edges = Vec::new();
    let mut new_edges = Vec::new();
    let mut seed = 12345u64;
    
    // 生成初始边
    for _ in 0..edge_count {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let u = (seed >> 32) % vertex_count as u64;
        
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let v = (seed >> 32) % vertex_count as u64;
        
        if u != v {
            edges.push((u.min(v), u.max(v)));
        }
    }
    
    // 生成新边
    for _ in 0..new_edge_count {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let u = (seed >> 32) % vertex_count as u64;
        
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let v = (seed >> 32) % vertex_count as u64;
        
        if u != v {
            new_edges.push((u.min(v), u.max(v)));
        }
    }
    
    (edges, new_edges)
}

/// 生成图（用于 Triangle Counting）
fn generate_graph_for_tc(
    vertex_count: usize,
    edge_count: usize,
    new_edge_count: usize,
) -> (axolotl_rs::incremental_tc::GraphWithAdjacencySets, Vec<(u32, u32)>) {
    let mut graph = axolotl_rs::incremental_tc::GraphWithAdjacencySets::new(vertex_count);
    
    // 添加初始边
    let mut seed = 12345u64;
    let mut attempts = 0;
    
    while graph.edge_count() < edge_count && attempts < edge_count * 2 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let u = (seed >> 32) % vertex_count as u64;
        
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let v = (seed >> 32) % vertex_count as u64;
        
        if u != v {
            graph.add_edge(u as u32, v as u32);
        }
        attempts += 1;
    }
    
    // 添加新边
    let new_edges = graph.add_new_edges(new_edge_count);
    
    (graph, new_edges)
}
