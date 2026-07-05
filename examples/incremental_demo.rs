// examples/incremental_demo.rs
// 增量 PageRank 演示（最小实现）

use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use rand::Rng; // 添加 Rng trait

/// 简化的图结构（只用于演示）
struct SimpleGraph {
    edges: Vec<Vec<usize>>, // 邻接表
}

impl SimpleGraph {
    fn new(n: usize) -> Self {
        SimpleGraph {
            edges: vec![Vec::new(); n],
        }
    }
    
    fn add_edge(&mut self, from: usize, to: usize) {
        self.edges[from].push(to);
    }
    
    fn get_neighbors(&self, v: usize) -> &[usize] {
        &self.edges[v]
    }
    
    fn n_vertices(&self) -> usize {
        self.edges.len()
    }
}

/// 全量 PageRank（传统实现）
fn full_pagerank(graph: &SimpleGraph, damping: f64, max_iter: u32) -> Vec<f64> {
    let n = graph.n_vertices();
    let mut pr = vec![1.0 / n as f64; n];
    
    for _ in 0..max_iter {
        let mut new_pr = vec![(1.0 - damping) / n as f64; n];
        
        for (v, &pr_v) in pr.iter().enumerate() {
            let neighbors = graph.get_neighbors(v);
            if neighbors.is_empty() {
                // dangling node
                let contrib = pr_v * damping / n as f64;
                for pr_val in new_pr.iter_mut() {
                    *pr_val += contrib;
                }
            } else {
                let contrib = pr_v * damping / neighbors.len() as f64;
                for &neighbor in neighbors {
                    new_pr[neighbor] += contrib;
                }
            }
        }
        
        pr = new_pr;
    }
    
    pr
}

/// 增量 PageRank（只更新受影响的顶点）
fn incremental_pagerank(
    graph: &SimpleGraph,
    old_pr: &[f64],
    affected: &[usize],
    damping: f64,
    max_iter: u32,
) -> Vec<f64> {
    let n = graph.n_vertices();
    let mut pr = old_pr.to_vec();
    
    // 只更新受影响的顶点及其邻居
    let mut queue: VecDeque<usize> = affected.iter().cloned().collect();
    let mut visited = vec![false; n];
    
    for &v in affected {
        visited[v] = true;
    }
    
    let mut iterations = 0;
    while !queue.is_empty() && iterations < max_iter {
        let v = queue.pop_front().unwrap();
        visited[v] = false;
        
        let old_pr_v = pr[v];
        let mut new_pr_v = (1.0 - damping) / n as f64;
        
        // 计算新的 PageRank 值
        for (u, &pr_u) in pr.iter().enumerate() {
            let neighbors = graph.get_neighbors(u);
            if neighbors.contains(&v) {
                if !neighbors.is_empty() {
                    new_pr_v += damping * pr_u / neighbors.len() as f64;
                }
            }
        }
        
        // 检查是否收敛
        if (new_pr_v - old_pr_v).abs() > 1e-6 {
            pr[v] = new_pr_v;
            
            // 将受影响的邻居加入队列
            let neighbors = graph.get_neighbors(v);
            for &neighbor in neighbors {
                if !visited[neighbor] {
                    queue.push_back(neighbor);
                    visited[neighbor] = true;
                }
            }
        }
        
        iterations += 1;
    }
    
    pr
}

fn main() {
    println!("增量 PageRank 演示");
    println!("{}", "=".repeat(80));
    
    // 创建测试图（1000 顶点，5000 边）
    let n = 1000;
    let mut graph = SimpleGraph::new(n);
    
    let mut rng = rand::thread_rng();
    let mut added_edges = std::collections::HashSet::new();
    
    while added_edges.len() < 5000 {
        let u = rng.gen_range(0..n);
        let v = rng.gen_range(0..n);
        
        if u != v && !added_edges.contains(&(u, v)) {
            added_edges.insert((u, v));
            graph.add_edge(u, v);
        }
    }
    
    println!("\n图规模: {} 顶点, {} 边", n, added_edges.len());
    
    // 全量 PageRank
    println!("\n1. 全量 PageRank（首次计算）:");
    let start = Instant::now();
    let full_pr = full_pagerank(&graph, 0.85, 100);
    let full_time = start.elapsed();
    println!("   耗时: {:?}", full_time);
    
    // 更新图（添加 10 条边）
    let mut affected_vertices = Vec::new();
    for _ in 0..10 {
        let u = rng.gen_range(0..n);
        let v = rng.gen_range(0..n);
        if u != v {
            graph.add_edge(u, v);
            affected_vertices.push(u);
            affected_vertices.push(v);
        }
    }
    
    println!("\n2. 图已更新（添加了 10 条边）");
    println!("   受影响的顶点数: {}", affected_vertices.len());
    
    // 增量 PageRank
    println!("\n3. 增量 PageRank（只更新受影响的顶点）:");
    let start = Instant::now();
    let _inc_pr = incremental_pagerank(&graph, &full_pr, &affected_vertices, 0.85, 100);
    let inc_time = start.elapsed();
    println!("   耗时: {:?}", inc_time);
    
    // 对比：如果重新运行全量 PageRank
    println!("\n4. 对比：重新运行全量 PageRank:");
    let start = Instant::now();
    let _full_pr2 = full_pagerank(&graph, 0.85, 100);
    let full_time2 = start.elapsed();
    println!("   耗时: {:?}", full_time2);
    
    // 加速比
    println!("\n{}", "=".repeat(80));
    println!("性能对比:");
    println!("  全量 PageRank: {:?}", full_time);
    println!("  增量 PageRank: {:?}", inc_time);
    println!("  加速比: {:.2}x", full_time2.as_secs_f64() / inc_time.as_secs_f64());
    println!("  (预期加速比: 50x ~ 200x)");
}
