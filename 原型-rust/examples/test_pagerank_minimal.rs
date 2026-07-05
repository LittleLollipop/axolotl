// examples/test_pagerank_minimal.rs
// 最简化的 PageRank 测试（无格式化字符串）

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;
use axolotl_rs::csr_graph::{CSRGraph, PropertyValue};

fn main() {
    println!("开始测试...");
    
    // 加载数据
    let file = File::open("dataset_1k_10k.edgelist").expect("无法打开文件");
    let reader = BufReader::new(file);
    
    let mut edges = Vec::new();
    let mut max_vertex = 0;
    
    for line in reader.lines() {
        let line = line.expect("无法读取行");
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        
        if parts.len() >= 2 {
            let source: u64 = parts[0].parse().expect("无法解析");
            let target: u64 = parts[1].parse().expect("无法解析");
            
            edges.push((source, target));
            max_vertex = max_vertex.max(source).max(target);
        }
    }
    
    println!("加载完成，边数：{}", edges.len());
    
    // 创建 CSR 图
    let mut csr = CSRGraph::new();
    
    for i in 0..=max_vertex {
        let mut props = std::collections::HashMap::new();
        props.insert("id".to_string(), PropertyValue::Int(i as i64));
        csr.add_vertex(i, props);
    }
    
    csr.build_csr(&edges);
    
    println!("CSR 构建完成，顶点数：{}", csr.vertex_count);
    
    // 计算 PageRank
    let start = Instant::now();
    let vertex_count = csr.vertex_count as usize;
    let mut pr = vec![1.0 / vertex_count as f32; vertex_count];
    let damping = 0.85f32;
    
    for _ in 0..100 {
        let mut new_pr = vec![0.0; vertex_count];
        
        for v in 0..vertex_count {
            let mut contribution = 0.0f32;
            let start_idx = csr.reverse_offsets[v] as usize;
            let end_idx = csr.reverse_offsets[v + 1] as usize;
            
            for i in start_idx..end_idx {
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
    
    let elapsed = start.elapsed();
    
    println!("PageRank 计算完成");
    println!("耗时（秒）：{}", elapsed.as_secs_f64());
    
    // 打印统计信息
    let pr_sum: f32 = pr.iter().sum();
    println!("PR 值之和：{}", pr_sum);
    
    println!("测试完成！");
}
