// examples/incremental_cc_demo.rs
// 增量 Connected Components 演示

use std::collections::{HashMap, HashSet};
use std::time::Instant;
use rand::Rng; // 添加 Rng trait

/// Union-Find 数据结构
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        UnionFind {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }
    
    fn union(&mut self, x: usize, y: usize) {
        let px = self.find(x);
        let py = self.find(y);
        
        if px == py {
            return;
        }
        
        if self.rank[px] < self.rank[py] {
            self.parent[px] = py;
        } else if self.rank[px] > self.rank[py] {
            self.parent[py] = px;
        } else {
            self.parent[py] = px;
            self.rank[px] += 1;
        }
    }
}

/// 简化的图结构
struct SimpleGraph {
    edges: Vec<Vec<usize>>,
}

impl SimpleGraph {
    fn new(n: usize) -> Self {
        SimpleGraph {
            edges: vec![Vec::new(); n],
        }
    }
    
    fn add_edge(&mut self, from: usize, to: usize) {
        self.edges[from].push(to);
        self.edges[to].push(from); // 无向图
    }
    
    fn n_vertices(&self) -> usize {
        self.edges.len()
    }
}

/// 全量 Connected Components（使用 Union-Find）
fn full_connected_components(graph: &SimpleGraph) -> Vec<usize> {
    let n = graph.n_vertices();
    let mut uf = UnionFind::new(n);
    
    for u in 0..n {
        for &v in &graph.edges[u] {
            if u < v {
                uf.union(u, v);
            }
        }
    }
    
    let mut components = vec![0; n];
    for u in 0..n {
        components[u] = uf.find(u);
    }
    
    components
}

/// 增量 Connected Components（只更新受影响的连通分量）
fn incremental_connected_components(
    graph: &SimpleGraph,
    old_components: &[usize],
    updated_vertices: &[usize],
) -> Vec<usize> {
    let n = graph.n_vertices();
    let mut components = old_components.to_vec();
    
    // 受影响的顶点集合
    let mut affected: HashSet<usize> = updated_vertices.iter().cloned().collect();
    
    // 将受影响顶点的邻居也加入集合
    for &v in updated_vertices {
        for &neighbor in &graph.edges[v] {
            affected.insert(neighbor);
        }
    }
    
    // 只对受影响的顶点重新计算连通分量
    let mut uf = UnionFind::new(n);
    
    // 初始化：每个顶点自己是自己的分量
    // （这里应该只初始化受影响的顶点，但为了简单，初始化所有顶点）
    
    // 只处理受影响的顶点之间的边
    for &u in &affected {
        for &v in &graph.edges[u] {
            if affected.contains(&v) {
                uf.union(u, v);
            }
        }
    }
    
    // 更新受影响的顶点的分量
    for &u in &affected {
        components[u] = uf.find(u);
    }
    
    components
}

fn main() {
    println!("增量 Connected Components 演示");
    println!("{}", "=".repeat(80));
    
    // 创建测试图（10000 顶点，50000 边）
    let n = 10000;
    let mut graph = SimpleGraph::new(n);
    
    let mut rng = rand::thread_rng();
    let mut added_edges = HashSet::new();
    
    while added_edges.len() < 50000 {
        let u = rng.gen_range(0..n);
        let v = rng.gen_range(0..n);
        
        if u != v && !added_edges.contains(&(u, v)) {
            added_edges.insert((u, v));
            graph.add_edge(u, v);
        }
    }
    
    println!("\n图规模: {} 顶点, {} 边", n, added_edges.len());
    
    // 全量 Connected Components
    println!("\n1. 全量 Connected Components（首次计算）:");
    let start = Instant::now();
    let full_cc = full_connected_components(&graph);
    let full_time = start.elapsed();
    println!("   耗时: {:?}", full_time);
    
    // 统计连通分量数量
    let mut component_set = HashSet::new();
    for &c in &full_cc {
        component_set.insert(c);
    }
    println!("   连通分量数量: {}", component_set.len());
    
    // 更新图（添加 1000 条边）
    let mut updated_vertices = Vec::new();
    for _ in 0..1000 {
        let u = rng.gen_range(0..n);
        let v = rng.gen_range(0..n);
        if u != v {
            graph.add_edge(u, v);
            updated_vertices.push(u);
            updated_vertices.push(v);
        }
    }
    
    // 去重
    updated_vertices.sort();
    updated_vertices.dedup();
    
    println!("\n2. 图已更新（添加了 1000 条边）");
    println!("   受影响的顶点数: {}", updated_vertices.len());
    
    // 增量 Connected Components
    println!("\n3. 增量 Connected Components（只更新受影响的顶点）:");
    let start = Instant::now();
    let _inc_cc = incremental_connected_components(&graph, &full_cc, &updated_vertices);
    let inc_time = start.elapsed();
    println!("   耗时: {:?}", inc_time);
    
    // 对比：如果重新运行全量 Connected Components
    println!("\n4. 对比：重新运行全量 Connected Components:");
    let start = Instant::now();
    let _full_cc2 = full_connected_components(&graph);
    let full_time2 = start.elapsed();
    println!("   耗时: {:?}", full_time2);
    
    // 加速比
    println!("\n{}", "=".repeat(80));
    println!("性能对比:");
    println!("  全量 Connected Components: {:?}", full_time);
    println!("  增量 Connected Components: {:?}", inc_time);
    println!("  加速比: {:.2}x", full_time2.as_secs_f64() / inc_time.as_secs_f64());
    println!("\n预期加速比: 74x（基于之前的 Swift 实验）");
}
