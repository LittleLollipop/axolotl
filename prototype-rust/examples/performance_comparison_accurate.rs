// examples/performance_comparison_accurate.rs
// 准确的性能对比测试（使用更多迭代次数）

use axolotl_rs::*;
use axolotl_rs::gpu::GPUAccelerator;
use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;
use std::time::Instant;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {
    println!("=== Axolotl 准确性能测试（使用更多迭代次数）===\n");
    
    // 只测试一个数据集
    let filename = "dataset_10k_100k.edgelist";
    let (edges, vertex_count) = load_edgelist(filename);
    println!("数据集: {} ({} 顶点, {} 边)\n", filename, vertex_count, edges.len());
    
    // 测试 PageRank (CPU 版本，10000 次迭代)
    test_pagerank_cpu_accurate(&edges, vertex_count);
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

/// 测试 PageRank (CPU 版本，准确测量)
fn test_pagerank_cpu_accurate(edges: &[(u64, u64)], vertex_count: u32) {
    println!("--- PageRank (CPU 版本，准确测量) ---");
    
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
    
    // 测试（使用大量迭代次数）
    let iterations = 10000;
    let mut pr = vec![1.0f32 / vertex_count as f32; vertex_count as usize];
    
    let start = Instant::now();
    for _ in 0..iterations {
        let dangling_sum: f32 = dangling.iter()
            .map(|&v| pr[v])
            .sum();
        let dangling_contribution = dangling_sum / vertex_count as f32;
        
        let mut new_pr = vec![0.0f32; vertex_count as usize];
        
        for v in 0..vertex_count as usize {
            let mut contribution = 0.0f32;
            for &u in &in_edges[v] {
                contribution += pr[u as usize] / out_degrees[u as usize] as f32;
            }
            
            new_pr[v] = (1.0 - 0.85) / vertex_count as f32
                + 0.85 * (contribution + dangling_contribution);
        }
        
        pr = new_pr;
    }
    let elapsed = start.elapsed();
    
    let pr_sum: f32 = pr.iter().sum();
    let avg_time_us = elapsed.as_micros() as f32 / iterations as f32;
    
    println!("  迭代次数: {}", iterations);
    println!("  总时间: {:.4}s", elapsed.as_secs_f32());
    println!("  平均每次: {:.4}μs ({:.4}ms)", avg_time_us, avg_time_us / 1000.0);
    println!("  PR 值之和: {:.10}", pr_sum);
    println!("  正确性: {}", if (pr_sum - 1.0).abs() < 1e-6 { "✓" } else { "✗" });
}
