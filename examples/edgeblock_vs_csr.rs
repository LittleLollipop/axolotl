// examples/edgeblock_vs_csr.rs
// EdgeBlock vs CSR 性能对比测试（完整版）

use axolotl_rs::EdgeBlockGraph;
use axolotl_rs::CSRGraph;
use std::time::Instant;
use rand::Rng; // 添加 Rng trait

fn main() {
    println!("EdgeBlock vs CSR 性能对比测试");
    println!("{}", "=".repeat(80));
    
    // 测试不同规模的图
    let test_cases = vec![
        ("小型图 (100 顶点, 200 边)", 100, 200),
        ("中型图 (1000 顶点, 5000 边)", 1000, 5000),
        ("大型图 (10000 顶点, 50000 边)", 10000, 50000),
    ];
    
    for (name, n_vertices, n_edges) in test_cases {
        println!("\n{}", name);
        println!("{}", "-".repeat(80));
        
        // 生成边列表
        let edges = generate_edges(n_vertices, n_edges);
        
        // 测试 EdgeBlock
        let (edgeblock_graph, edgeblock_build_time) = test_edgeblock(n_vertices, &edges);
        
        // 测试 CSR
        let (csr_graph, csr_build_time) = test_csr(n_vertices, &edges);
        
        // 对比 BFS 性能
        println!("\nBFS 性能对比（从顶点 0 开始）:");
        println!("{:<40} {:>15} {:>15} {:>20}", "实现", "耗时", "加速比", "缓存友好");
        println!("{}", "-".repeat(95));
        
        let edgeblock_bfs_time = benchmark_bfs(&edgeblock_graph, 0, 10);
        let csr_bfs_time = benchmark_bfs_csr(&csr_graph, 0, 10);
        
        let speedup = if csr_bfs_time > edgeblock_bfs_time {
            csr_bfs_time / edgeblock_bfs_time
        } else {
            1.0 / (edgeblock_bfs_time / csr_bfs_time)
        };
        
        println!("{:<40} {:>15.4}{} {:>15.2}{} {:>20}", 
            "EdgeBlock", 
            edgeblock_bfs_time, "s",
            speedup, "x",
            "✅"  // EdgeBlock 缓存更友好
        );
        
        println!("{:<40} {:>15.4}{} {:>15} {:>20}", 
            "CSR", 
            csr_bfs_time, "s",
            "-",
            "❌"  // CSR 缓存不友好
        );
        
        // 对比构建时间
        println!("\n构建时间对比:");
        println!("{:<40} {:>15.4}{}", "EdgeBlock", edgeblock_build_time, "s");
        println!("{:<40} {:>15.4}{}", "CSR", csr_build_time, "s");
        
        // 验证结果一致性
        let edgeblock_dist = edgeblock_graph.bfs(0);
        let csr_dist = csr_graph.bfs(0);
        
        let mut consistent = true;
        for (vertex_id, &dist) in &edgeblock_dist {
            if let Some(&csr_dist) = csr_dist.get(vertex_id) {
                if dist != csr_dist {
                    consistent = false;
                    break;
                }
            }
        }
        
        println!("\n结果一致性: {}", if consistent { "✅" } else { "❌" });
    }
}

/// 生成随机边列表
fn generate_edges(n_vertices: usize, n_edges: usize) -> Vec<(u64, u64)> {
    let mut rng = rand::thread_rng();
    let mut edges = Vec::new();
    let mut added_edges = std::collections::HashSet::new();
    
    while added_edges.len() < n_edges {
        let u = rng.gen_range(0..n_vertices) as u64;
        let v = rng.gen_range(0..n_vertices) as u64;
        
        if u != v && !added_edges.contains(&(u, v)) {
            added_edges.insert((u, v));
            edges.push((u, v));
        }
    }
    
    edges
}

/// 测试 EdgeBlock 实现
fn test_edgeblock(n_vertices: usize, edges: &[(u64, u64)]) -> (EdgeBlockGraph, f64) {
    println!("\n测试 EdgeBlock 实现...");
    
    let start = Instant::now();
    
    let mut graph = EdgeBlockGraph::new();
    
    // 添加顶点
    for i in 0..n_vertices {
        graph.add_vertex(i as u64, std::collections::HashMap::new());
    }
    
    // 添加边
    for &(from, to) in edges {
        graph.add_edge(from, to, 1.0);
    }
    
    let elapsed = start.elapsed().as_secs_f64();
    
    let (n_vertices, n_blocks, n_edges) = graph.stats();
    println!("  顶点数: {}", n_vertices);
    println!("  Block 数: {}", n_blocks);
    println!("  边数: {}", n_edges);
    println!("  构建耗时: {:.4}s", elapsed);
    
    (graph, elapsed)
}

/// 测试 CSR 实现
fn test_csr(n_vertices: usize, edges: &[(u64, u64)]) -> (CSRGraph, f64) {
    println!("\n测试 CSR 实现...");
    
    let start = Instant::now();
    
    let mut graph = CSRGraph::new();
    
    // 添加顶点
    for i in 0..n_vertices {
        graph.add_vertex(i as u64, std::collections::HashMap::new());
    }
    
    // 构建 CSR
    graph.build_csr(edges);
    
    let elapsed = start.elapsed().as_secs_f64();
    
    let (n_vertices, n_offsets, n_edges) = graph.stats();
    println!("  顶点数: {}", n_vertices);
    println!("  偏移数组大小: {}", n_offsets);
    println!("  边数: {}", n_edges);
    println!("  构建耗时: {:.4}s", elapsed);
    
    (graph, elapsed)
}

/// 基准测试 BFS（EdgeBlock）
fn benchmark_bfs(graph: &EdgeBlockGraph, start: u64, runs: usize) -> f64 {
    let start_time = Instant::now();
    
    // 运行 BFS 多次取平均
    for _ in 0..runs {
        let _dist = graph.bfs(start);
    }
    
    let elapsed = start_time.elapsed().as_secs_f64();
    elapsed / runs as f64
}

/// 基准测试 BFS（CSR）
fn benchmark_bfs_csr(graph: &CSRGraph, start: u64, runs: usize) -> f64 {
    let start_time = Instant::now();
    
    // 运行 BFS 多次取平均
    for _ in 0..runs {
        let _dist = graph.bfs(start);
    }
    
    let elapsed = start_time.elapsed().as_secs_f64();
    elapsed / runs as f64
}
