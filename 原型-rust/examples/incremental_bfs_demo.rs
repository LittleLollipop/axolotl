// examples/incremental_bfs_demo.rs
// 增量 BFS 演示（对比全量 BFS）

use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use rand::Rng; // 添加 Rng trait

/// 简化的图结构
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

/// 全量 BFS（传统实现）
fn full_bfs(graph: &SimpleGraph, start: usize) -> Vec<i32> {
    let n = graph.n_vertices();
    let mut dist = vec![-1; n];
    let mut queue = VecDeque::new();
    
    dist[start] = 0;
    queue.push_back(start);
    
    while let Some(current) = queue.pop_front() {
        let current_dist = dist[current];
        
        for &neighbor in graph.get_neighbors(current) {
            if dist[neighbor] == -1 {
                dist[neighbor] = current_dist + 1;
                queue.push_back(neighbor);
            }
        }
    }
    
    dist
}

/// 增量 BFS（只更新受影响的顶点）
fn incremental_bfs(
    graph: &SimpleGraph,
    old_dist: &[i32],
    affected_vertices: &[usize],
) -> Vec<i32> {
    let n = graph.n_vertices();
    let mut dist = old_dist.to_vec();
    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut visited = vec![false; n];
    
    // 将受影响的顶点加入队列
    for &v in affected_vertices {
        if !visited[v] {
            queue.push_back(v);
            visited[v] = true;
        }
    }
    
    // BFS 从受影响的顶点开始
    while let Some(current) = queue.pop_front() {
        visited[current] = false;
        
        let new_dist = dist[current] + 1;
        
        // 检查邻居是否需要更新
        for &neighbor in graph.get_neighbors(current) {
            if dist[neighbor] == -1 || dist[neighbor] > new_dist {
                dist[neighbor] = new_dist;
                
                // 将邻居加入队列（如果还没在队列中）
                if !visited[neighbor] {
                    queue.push_back(neighbor);
                    visited[neighbor] = true;
                }
            }
        }
    }
    
    dist
}

fn main() {
    println!("增量 BFS 演示");
    println!("{}", "=".repeat(80));
    
    // 创建测试图（10000 顶点，50000 边）
    let n = 10000;
    let mut graph = SimpleGraph::new(n);
    
    let mut rng = rand::thread_rng();
    let mut added_edges = std::collections::HashSet::new();
    
    while added_edges.len() < 50000 {
        let u = rng.gen_range(0..n);
        let v = rng.gen_range(0..n);
        
        if u != v && !added_edges.contains(&(u, v)) {
            added_edges.insert((u, v));
            graph.add_edge(u, v);
        }
    }
    
    println!("\n图规模: {} 顶点, {} 边", n, added_edges.len());
    
    // 全量 BFS
    println!("\n1. 全量 BFS（首次计算）:");
    let start = Instant::now();
    let full_dist = full_bfs(&graph, 0);
    let full_time = start.elapsed();
    println!("   耗时: {:?}", full_time);
    
    // 更新图（添加 100 条边）
    let mut affected_vertices = Vec::new();
    for _ in 0..100 {
        let u = rng.gen_range(0..n);
        let v = rng.gen_range(0..n);
        if u != v {
            graph.add_edge(u, v);
            affected_vertices.push(u);
            affected_vertices.push(v);
        }
    }
    
    // 去重
    affected_vertices.sort();
    affected_vertices.dedup();
    
    println!("\n2. 图已更新（添加了 100 条边）");
    println!("   受影响的顶点数: {}", affected_vertices.len());
    
    // 增量 BFS
    println!("\n3. 增量 BFS（只更新受影响的顶点）:");
    let start = Instant::now();
    let _inc_dist = incremental_bfs(&graph, &full_dist, &affected_vertices);
    let inc_time = start.elapsed();
    println!("   耗时: {:?}", inc_time);
    
    // 对比：如果重新运行全量 BFS
    println!("\n4. 对比：重新运行全量 BFS:");
    let start = Instant::now();
    let _full_dist2 = full_bfs(&graph, 0);
    let full_time2 = start.elapsed();
    println!("   耗时: {:?}", full_time2);
    
    // 加速比
    println!("\n{}", "=".repeat(80));
    println!("性能对比:");
    println!("  全量 BFS: {:?}", full_time);
    println!("  增量 BFS: {:?}", inc_time);
    println!("  加速比: {:.2}x", full_time2.as_secs_f64() / inc_time.as_secs_f64());
    println!("\n预期加速比: 80x（基于之前的 Swift 实验）");
}
