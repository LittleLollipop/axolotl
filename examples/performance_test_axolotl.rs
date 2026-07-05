// examples/performance_test_axolotl.rs
// Axolotl 性能测试（对比 NetworkX）
//
// 测试 PageRank、BFS、SSSP 算法的性能

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;
use axolotl_rs::*;

fn load_edgelist(file_path: &str) -> (Vec<(u32, u32)>, u32) {
    """
    从边缘列表文件加载边
    
    参数：
    - file_path: 文件路径（每行格式：source target）
    
    返回：
    - (edges, max_vertex): 边列表和最大顶点 ID
    """
    let file = File::open(file_path).expect("无法打开文件");
    let reader = BufReader::new(file);
    
    let mut edges = Vec::new();
    let mut max_vertex = 0;
    
    for line in reader.lines() {
        let line = line.expect("无法读取行");
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        
        if parts.len() >= 2 {
            let source: u32 = parts[0].parse().expect("无法解析源顶点");
            let target: u32 = parts[1].parse().expect("无法解析目标顶点");
            
            edges.push((source, target));
            max_vertex = max_vertex.max(source).max(target);
        }
    }
    
    (edges, max_vertex)
}

fn main() {
    // 测试数据集
    let datasets = vec![
        ("dataset_1k_10k.edgelist", 1000, 10000),
        ("dataset_10k_100k.edgelist", 10000, 100000),
        ("dataset_100k_1m.edgelist", 100000, 1000000),
    ];
    
    for (dataset, expected_vertices, expected_edges) in datasets {
        println!("\n############################################################");
        println!("# 测试数据集：{}", dataset);
        println!("############################################################");
        
        // 加载边
        println!("\n加载边：{}", dataset);
        let start = Instant::now();
        let (edges, max_vertex) = load_edgelist(dataset);
        let load_time = start.elapsed();
        
        println!("  加载完成：{} 条边，最大顶点 ID = {}", edges.len(), max_vertex);
        println!("  耗时：{:.2f} 秒", load_time.as_secs_f64());
        
        // 创建 CSR 图
        println!("\n创建 CSR 图...");
        let start = Instant::now();
        let mut csr = CSRGraph::new();
        
        // 添加顶点
        for i in 0..=max_vertex {
            let mut props = std::collections::HashMap::new();
            props.insert("id".to_string(), PropertyValue::Int(i as i64));
            csr.add_vertex(i, props);
        }
        
        // 添加边
        csr.build_csr(&edges);
        
        let csr_time = start.elapsed();
        println!("  创建完成：{} 个顶点，{} 条边", csr.vertex_count, csr.total_edges);
        println!("  耗时：{:.2f} 秒", csr_time.as_secs_f64());
        
        // 测试 PageRank（全量计算）
        println!("\n============================================================");
        println!("测试 PageRank：{}", dataset);
        println!("============================================================");
        
        let start = Instant::now();
        let pr = compute_pagerank_cpu(&csr, 100);  // 100 次迭代
        let pr_time = start.elapsed();
        
        println!("  全量计算 PageRank（100 次迭代）...");
        println!("    耗时：{:.2f} 秒", pr_time.as_secs_f64());
        
        // 打印统计信息
        let pr_sum: f32 = pr.iter().sum();
        let pr_min = pr.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let pr_max = pr.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        println!("    PR 值范围：{:.6f} - {:.6f}", pr_min, pr_max);
        println!("    PR 值之和：{:.6f}", pr_sum);
        
        // 测试 BFS（全量计算）
        println!("\n============================================================");
        println!("测试 BFS：{}", dataset);
        println!("============================================================");
        
        let source = 0;  // 源顶点
        println!("  源顶点：{}", source);
        
        let start = Instant::now();
        let distances = compute_bfs_cpu(&csr, source);
        let bfs_time = start.elapsed();
        
        println!("  全量计算 BFS...");
        println!("    耗时：{:.2f} 秒", bfs_time.as_secs_f64());
        
        // 打印统计信息
        let reachable = distances.iter().filter(|&&d| d < u32::MAX).count();
        println!("    可达顶点数：{}", reachable);
        
        // 测试 SSSP（全量计算，无权图）
        println!("\n============================================================");
        println!("测试 SSSP（无权图）：{}", dataset);
        println!("============================================================");
        
        let start = Instant::now();
        let sssp_distances = compute_sssp_cpu(&csr, source);
        let sssp_time = start.elapsed();
        
        println!("  全量计算 SSSP...");
        println!("    耗时：{:.2f} 秒", sssp_time.as_secs_f64());
        
        // 打印统计信息
        let sssp_reachable = sssp_distances.iter().filter(|&&d| d < u32::MAX).count();
        println!("    可达顶点数：{}", sssp_reachable);
        
        // 打印总结
        println!("\n============================================================");
        println!("测试结果总结：{}", dataset);
        println!("============================================================");
        println!("  数据加载：{:.2f} 秒", load_time.as_secs_f64());
        println!("  CSR 构建：{:.2f} 秒", csr_time.as_secs_f64());
        println!("  PageRank：{:.2f} 秒", pr_time.as_secs_f64());
        println!("  BFS：{:.2f} 秒", bfs_time.as_secs_f64());
        println!("  SSSP：{:.2f} 秒", sssp_time.as_secs_f64());
    }
}

/// CPU 版本的 PageRank（用于性能测试）
fn compute_pagerank_cpu(csr: &CSRGraph, iterations: usize) -> Vec<f32> {
    let vertex_count = csr.vertex_count as usize;
    let mut pr = vec![1.0 / vertex_count as f32; vertex_count];
    let damping = 0.85f32;
    
    for _ in 0..iterations {
        let mut new_pr = vec![0.0; vertex_count];
        
        // 使用反向 CSR 找出每个顶点的入边邻居
        for v in 0..vertex_count {
            let mut contribution = 0.0f32;
            let start = csr.reverse_offsets[v] as usize;
            let end = csr.reverse_offsets[v + 1] as usize;
            
            for i in start..end {
                let u = csr.reverse_targets[i] as usize;  // 有边从 u 指向 v
                let out_degree = (csr.offsets[u + 1] - csr.offsets[u]) as f32;
                if out_degree > 0.0 {
                    contribution += pr[u] / out_degree;
                }
            }
            
            new_pr[v] = (1.0 - damping) / vertex_count as f32 + damping * contribution;
        }
        
        pr = new_pr;
    }
    
    pr
}

/// CPU 版本的 BFS（用于性能测试）
fn compute_bfs_cpu(csr: &CSRGraph, source: u32) -> Vec<u32> {
    let vertex_count = csr.vertex_count as usize;
    let mut distances = vec![u32::MAX; vertex_count];
    distances[source as usize] = 0;
    
    let mut frontier = vec![source];
    
    while !frontier.is_empty() {
        let mut next_frontier = Vec::new();
        
        for &v in &frontier {
            let start = csr.offsets[v as usize] as usize;
            let end = csr.offsets[v as usize + 1] as usize;
            
            for i in start..end {
                let neighbor = csr.targets[i] as usize;
                if distances[neighbor] == u32::MAX {
                    distances[neighbor] = distances[v as usize] + 1;
                    next_frontier.push(neighbor as u32);
                }
            }
        }
        
        frontier = next_frontier;
    }
    
    distances
}

/// CPU 版本的 SSSP（无权图）
fn compute_sssp_cpu(csr: &CSRGraph, source: u32) -> Vec<u32> {
    // 无权图：SSSP = BFS
    compute_bfs_cpu(csr, source)
}
