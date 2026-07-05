// examples/test_pagerank_simple.rs
// 简化的 PageRank 性能测试

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;
use axolotl_rs::CSRGraph;

fn load_edgelist(file_path: &str) -> (Vec<(u64, u64)>, u64) {
    let file = File::open(file_path).expect("无法打开文件");
    let reader = BufReader::new(file);
    
    let mut edges = Vec::new();
    let mut max_vertex = 0;
    
    for line in reader.lines() {
        let line = line.expect("无法读取行");
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        
        if parts.len() >= 2 {
            let source: u64 = parts[0].parse().expect("无法解析源顶点");
            let target: u64 = parts[1].parse().expect("无法解析目标顶点");
            
            edges.push((source, target));
            max_vertex = max_vertex.max(source).max(target);
        }
    }
    
    (edges, max_vertex)
}

fn compute_pagerank_cpu(csr: &CSRGraph, iterations: usize) -> Vec<f32> {
    let vertex_count = csr.vertex_count as usize;
    let mut pr = vec![1.0 / vertex_count as f32; vertex_count];
    let damping = 0.85f32;
    
    for _ in 0..iterations {
        let mut new_pr = vec![0.0; vertex_count];
        
        for v in 0..vertex_count {
            let mut contribution = 0.0f32;
            let start = csr.reverse_offsets[v] as usize;
            let end = csr.reverse_offsets[v + 1] as usize;
            
            for i in start..end {
                let u = csr.reverse_targets[i] as usize;
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

fn main() {
    let dataset = "dataset_1k_10k.edgelist";
    
    println!("加载数据集：{}", dataset);
    let start = Instant::now();
    let (edges, max_vertex) = load_edgelist(dataset);
    println!("  加载 {} 条边，耗时：{:.2f} 秒", edges.len(), start.elapsed().as_secs_f64());
    
    println!("\n创建 CSR 图...");
    let start = Instant::now();
    let mut csr = CSRGraph::new();
    
    // 添加顶点
    for i in 0..=max_vertex {
        let mut props = std::collections::HashMap::new();
        props.insert("id".to_string(), axolotl_rs::PropertyValue::Int(i as i64));
        csr.add_vertex(i, props);
    }
    
    // 添加边
    csr.build_csr(&edges);
    
    println!("  创建完成：{} 个顶点，{} 条边", csr.vertex_count, csr.total_edges);
    println!("  耗时：{:.2f} 秒", start.elapsed().as_secs_f64());
    
    println!("\n计算 PageRank（100 次迭代）...");
    let start = Instant::now();
    let pr = compute_pagerank_cpu(&csr, 100);
    let elapsed = start.elapsed();
    
    println!("  完成，耗时：{:.2f} 秒", elapsed.as_secs_f64());
    
    // 打印统计信息
    let pr_sum: f32 = pr.iter().sum();
    let pr_min = pr.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let pr_max = pr.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    println!("\nPR 值统计：");
    println!("  最小值：{:.6f}", pr_min);
    println!("  最大值：{:.6f}", pr_max);
    println!("  总和：{:.6f}", pr_sum);
    
    println!("\n✅ 测试完成！");
}
