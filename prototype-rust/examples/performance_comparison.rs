// examples/performance_comparison.rs
// 完整的性能对比测试：Axolotl (GPU) vs CPU 版本
// 使用真实数据集，测试 PageRank 和 BFS

use axolotl_rs::*;
use axolotl_rs::gpu::GPUAccelerator;
use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;
use std::time::Instant;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {
    println!("=== Axolotl 性能对比测试（GPU vs CPU）===\n");
    
    // 测试数据集
    let datasets = vec![
        ("dataset_1k_10k.edgelist", 1000, 10000),
        ("dataset_10k_100k.edgelist", 10000, 100000),
        ("dataset_100k_1m.edgelist", 100000, 1000000),  // 新增：100K 顶点
    ];
    
    for (filename, expected_vertices, expected_edges) in datasets {
        println!("\n{}", "=".repeat(80));
        println!("数据集: {}", filename);
        println!("预期: {} 顶点, {} 边", expected_vertices, expected_edges);
        println!("{}", "=".repeat(80));
        
        // 加载图数据
        let (edges, vertex_count) = load_edgelist(filename);
        println!("实际加载: {} 顶点, {} 边", vertex_count, edges.len());
        
        // 测试 PageRank（GPU 全量并发版本）
        test_pagerank_full_gpu(&edges, vertex_count);
        
        // 测试 PageRank（GPU 增量更新版本）
        test_pagerank_incremental_gpu(&edges, vertex_count);
        
        // 测试 PageRank（CPU 版本）
        test_pagerank_cpu(&edges, vertex_count);
        
        // 测试 BFS（GPU 版本）
        test_bfs_gpu(&edges, vertex_count);
        
        // 测试 BFS（CPU 版本）
        test_bfs_cpu(&edges, vertex_count);
    }
}

/// 加载边列表文件
fn load_edgelist(filename: &str) -> (Vec<(u64, u64)>, u32) {
    let path = format!("examples/{}", filename);
    let file = File::open(&path).expect("Failed to open file");
    let reader = BufReader::new(file);
    
    let mut edges = Vec::new();
    let mut max_vertex = 0;
    
    for line in reader.lines() {
        let line = line.expect("Failed to read line");
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() >= 2 {
            let u: u64 = parts[0].parse().expect("Invalid vertex ID");
            let v: u64 = parts[1].parse().expect("Invalid vertex ID");
            
            edges.push((u, v));
            max_vertex = max_vertex.max(u).max(v);
        }
    }
    
    let vertex_count = (max_vertex + 1) as u32;
    (edges, vertex_count)
}

/// 测试 PageRank（GPU 全量并发版本）
fn test_pagerank_full_gpu(edges: &[(u64, u64)], vertex_count: u32) {
    println!("\n--- PageRank (GPU 全量并发版本) ---");
    
    // 构建 CSRGraph
    let mut graph = CSRGraph::new();
    for i in 0..vertex_count as u64 {
        graph.add_vertex(i, std::collections::HashMap::new());
    }
    graph.build_csr(edges);
    
    // 创建 GPU 加速器
    let gpu = GPUAccelerator::new().expect("Failed to create GPU accelerator");
    
    // 计算 out_degrees
    let mut out_degrees = vec![0u32; vertex_count as usize];
    for v in 0..vertex_count as usize {
        let start = graph.offsets[v] as usize;
        let end = graph.offsets[v + 1] as usize;
        out_degrees[v] = (end - start) as u32;
    }
    
    // 预热
    println!("预热...");
    let _ = gpu.compute_full_pagerank(
        &graph.reverse_offsets,
        &graph.reverse_targets,
        &out_degrees,
        &vec![1.0f32 / vertex_count as f32; vertex_count as usize],
        vertex_count,
        0.85,
    );
    
    // 测试
    let iterations = 100;  // 增加迭代次数，提高测量精度
    let mut pr = vec![1.0f32 / vertex_count as f32; vertex_count as usize];
    
    // 预热
    println!("预热...");
    for _ in 0..10 {
        pr = gpu.compute_full_pagerank(
            &graph.reverse_offsets,
            &graph.reverse_targets,
            &out_degrees,
            &pr,
            vertex_count,
            0.85,
        );
    }
    
    // 测试
    let start = Instant::now();
    for _ in 0..iterations {
        pr = gpu.compute_full_pagerank(
            &graph.reverse_offsets,
            &graph.reverse_targets,
            &out_degrees,
            &pr,
            vertex_count,
            0.85,
        );
    }
    let elapsed = start.elapsed();
    
    let pr_sum: f32 = pr.iter().sum();
    
    println!("  迭代次数: {}", iterations);
    println!("  总时间: {:.4}s", elapsed.as_secs_f32());
    println!("  平均每次: {:.4}ms", elapsed.as_millis() as f32 / iterations as f32);
    println!("  PR 值之和: {:.10}", pr_sum);
    println!("  正确性: {}", if (pr_sum - 1.0).abs() < 1e-6 { "✓" } else { "✗" });
}

/// 测试 PageRank（GPU 增量更新版本）
fn test_pagerank_incremental_gpu(edges: &[(u64, u64)], vertex_count: u32) {
    println!("\n--- PageRank (GPU 增量更新版本) ---");
    
    // 构建 CSRGraph
    let mut csr = CSRGraph::new();
    for i in 0..vertex_count as u64 {
        csr.add_vertex(i, std::collections::HashMap::new());
    }
    csr.build_csr(edges);
    
    let gpu_graph = GPUEdgeBlockGraph::from_csr(
        &csr.offsets,
        &csr.targets,
        vertex_count,
    );
    
    // 创建 GPU 加速器
    let gpu = GPUAccelerator::new().expect("Failed to create GPU accelerator");
    
    // 计算 out_degrees
    let mut out_degrees = vec![0u32; vertex_count as usize];
    for v in 0..vertex_count as usize {
        let start = csr.offsets[v] as usize;
        let end = csr.offsets[v + 1] as usize;
        out_degrees[v] = (end - start) as u32;
    }
    
    // 预热
    println!("预热...");
    let affected_vertices: Vec<u32> = (0..vertex_count).collect();
    let _ = gpu.compute_incremental_pagerank_edgeblock(
        &gpu_graph,
        &vec![1.0f32 / vertex_count as f32; vertex_count as usize],
        &affected_vertices,
        &out_degrees,
        0.85,
    );
    
    // 测试
    let iterations = 100;  // 增加迭代次数
    let mut pr = vec![1.0f32 / vertex_count as f32; vertex_count as usize];
    
    // 预热
    println!("预热...");
    for _ in 0..10 {
        pr = gpu.compute_incremental_pagerank_edgeblock(
            &gpu_graph,
            &pr,
            &affected_vertices,
            &out_degrees,
            0.85,
        );
    }
    
    // 测试
    let start = Instant::now();
    for _ in 0..iterations {
        pr = gpu.compute_incremental_pagerank_edgeblock(
            &gpu_graph,
            &pr,
            &affected_vertices,
            &out_degrees,
            0.85,
        );
    }
    let elapsed = start.elapsed();
    
    let pr_sum: f32 = pr.iter().sum();
    
    println!("  迭代次数: {}", iterations);
    println!("  总时间: {:.4}s", elapsed.as_secs_f32());
    println!("  平均每次: {:.4}ms", elapsed.as_millis() as f32 / iterations as f32);
    println!("  PR 值之和: {:.10}", pr_sum);
    println!("  正确性: {}", if (pr_sum - 1.0).abs() < 1e-6 { "✓" } else { "✗" });
}

/// 测试 PageRank（CPU 版本，使用 f64）
fn test_pagerank_cpu(edges: &[(u64, u64)], vertex_count: u32) {
    println!("\n--- PageRank (CPU 版本，使用 f64) ---");
    
    // 构建邻接表（入边）
    let mut out_degrees = vec![0u32; vertex_count as usize];
    let mut in_edges = vec![Vec::new(); vertex_count as usize];
    
    for (u, v) in edges {
        let u_idx = *u as usize;
        let v_idx = *v as usize;
        out_degrees[u_idx] += 1;
        in_edges[v_idx].push(*u);
    }
    
    // 找出悬挂顶点
    let dangling: Vec<usize> = (0..vertex_count as usize)
        .filter(|&v| out_degrees[v] == 0)
        .collect();
    
    // 测试
    let iterations = 100;
    let mut pr = vec![1.0f64 / vertex_count as f64; vertex_count as usize];
    
    let start = Instant::now();
    for _ in 0..iterations {
        let dangling_sum: f64 = dangling.iter()
            .map(|&v| pr[v])
            .sum();
        let dangling_contribution = dangling_sum / vertex_count as f64;
        
        let mut new_pr = vec![0.0f64; vertex_count as usize];
        
        for v in 0..vertex_count as usize {
            let mut contribution = 0.0f64;
            for &u in &in_edges[v] {
                contribution += pr[u as usize] / out_degrees[u as usize] as f64;
            }
            
            new_pr[v] = (1.0 - 0.85) / vertex_count as f64
                + 0.85 * (contribution + dangling_contribution);
        }
        
        pr = new_pr;
    }
    let elapsed = start.elapsed();
    
    let pr_sum: f64 = pr.iter().sum();
    
    println!("  迭代次数: {}", iterations);
    println!("  总时间: {:.4}s", elapsed.as_secs_f64());
    println!("  平均每次: {:.4}ms", elapsed.as_millis() as f64 / iterations as f64);
    println!("  PR 值之和: {:.15}", pr_sum);
    println!("  正确性: {}", if (pr_sum - 1.0).abs() < 1e-10 { "✓" } else { "✗" });
}

/// 测试 BFS（GPU 版本）
fn test_bfs_gpu(edges: &[(u64, u64)], vertex_count: u32) {
    println!("\n--- BFS (GPU 版本) ---");
    
    // 构建 GPUEdgeBlockGraph
    let mut csr = CSRGraph::new();
    for i in 0..vertex_count as u64 {
        csr.add_vertex(i, std::collections::HashMap::new());
    }
    csr.build_csr(edges);
    
    let gpu_graph = GPUEdgeBlockGraph::from_csr(
        &csr.offsets,
        &csr.targets,
        vertex_count,
    );
    
    // 创建 GPU 加速器
    let gpu = GPUAccelerator::new().expect("Failed to create GPU accelerator");
    
    // 测试（从顶点 0 开始 BFS）
    let source = 0;
    let iterations = 10;
    
    let start = Instant::now();
    for _ in 0..iterations {
        let _distances = gpu.compute_bfs_edgeblock(&gpu_graph, source);
    }
    let elapsed = start.elapsed();
    
    println!("  源点: {}", source);
    println!("  迭代次数: {}", iterations);
    println!("  总时间: {:.4}s", elapsed.as_secs_f32());
    println!("  平均每次: {:.4}ms", elapsed.as_millis() as f32 / iterations as f32);
}

/// 测试 BFS（CPU 版本）
fn test_bfs_cpu(edges: &[(u64, u64)], vertex_count: u32) {
    println!("\n--- BFS (CPU 版本) ---");
    
    // 构建邻接表
    let mut adj_list = vec![Vec::new(); vertex_count as usize];
    for (u, v) in edges {
        adj_list[*u as usize].push(*v);
    }
    
    // 测试（从顶点 0 开始 BFS）
    let source = 0;
    let iterations = 10;
    
    let start = Instant::now();
    for _ in 0..iterations {
        let mut visited = vec![false; vertex_count as usize];
        let mut queue = std::collections::VecDeque::new();
        let mut distances = vec![-1; vertex_count as usize];
        
        visited[source] = true;
        queue.push_back(source);
        distances[source] = 0;
        
        while let Some(u) = queue.pop_front() {
            for &v in &adj_list[u] {
                if !visited[v as usize] {
                    visited[v as usize] = true;
                    queue.push_back(v as usize);
                    distances[v as usize] = distances[u] + 1;
                }
            }
        }
    }
    let elapsed = start.elapsed();
    
    println!("  源点: {}", source);
    println!("  迭代次数: {}", iterations);
    println!("  总时间: {:.4}s", elapsed.as_secs_f32());
    println!("  平均每次: {:.4}ms", elapsed.as_millis() as f32 / iterations as f32);
}
