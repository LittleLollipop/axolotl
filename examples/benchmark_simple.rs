// examples/benchmark_simple.rs
// Axolotl-RS 简单基准测试（用于跨语言对比）

use axolotl_rs::GraphDB;
use axolotl_rs::GraphAlgorithms;
use std::collections::HashMap;
use std::time::Instant;

fn main() {
    println!("Axolotl-RS 性能基准测试");
    println!("{}", "=".repeat(80));
    
    // 测试不同规模的图
    let test_cases = vec![
        ("小型图 (100 顶点, 200 边)", 100, 200),
        ("中型图 (500 顶点, 1000 边)", 500, 1000),
        ("大型图 (1000 顶点, 5000 边)", 1000, 5000),
    ];
    
    for (name, n_vertices, n_edges) in test_cases {
        println!("\n{}", name);
        println!("{}", "-".repeat(80));
        
        // 构建图
        let mut db = build_graph(n_vertices, n_edges);
        
        // 最短路径基准测试
        benchmark_shortest_path(&db);
        
        // PageRank 基准测试
        benchmark_pagerank(&mut db);
        
        // Betweenness Centrality 基准测试
        benchmark_betweenness(&db);
    }
}

/// 构建测试图
fn build_graph(n_vertices: usize, n_edges: usize) -> GraphDB {
    let mut db = GraphDB::new();
    
    // 添加顶点
    let mut vertex_ids = Vec::new();
    for i in 0..n_vertices {
        let mut props = HashMap::new();
        props.insert("id".to_string(), axolotl_rs::PropertyValue::Int(i as i64));
        let vertex_id = db.add_vertex(props).unwrap();
        vertex_ids.push(vertex_id);
    }
    
    // 添加边（随机）
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    let mut added_edges = std::collections::HashSet::new();
    
    while added_edges.len() < n_edges {
        let u = *vertex_ids.choose(&mut rng).unwrap();
        let v = *vertex_ids.choose(&mut rng).unwrap();
        
        if u != v && !added_edges.contains(&(u, v)) {
            added_edges.insert((u, v));
            let mut props = HashMap::new();
            props.insert("weight".to_string(), axolotl_rs::PropertyValue::Int(1));
            db.add_edge(u, v, props, 1.0).unwrap();
        }
    }
    
    db
}

/// 基准测试：最短路径
fn benchmark_shortest_path(db: &GraphDB) {
    // 测试 100 对随机顶点
    let start = Instant::now();
    
    let vertices: Vec<_> = db.vertices.keys().collect();
    let mut rng = rand::thread_rng();
    
    for _ in 0..100 {
        let s = vertices.choose(&mut rng).unwrap();
        let t = vertices.choose(&mut rng).unwrap();
        let _ = db.shortest_path(**s, **t);
    }
    
    let elapsed = start.elapsed();
    println!("{:<40} {:>15.4}{}", "最短路径 (100 次查询)", elapsed.as_secs_f64(), "s");
}

/// 基准测试：PageRank
fn benchmark_pagerank(db: &mut GraphDB) {
    // 首次计算
    let start = Instant::now();
    let _pr = db.pagerank(0.85, 100, 1e-6);
    let elapsed = start.elapsed();
    
    println!("{:<40} {:>15.4}{}", "PageRank (首次计算)", elapsed.as_secs_f64(), "s");
    
    // 使用缓存（增量更新）
    let start = Instant::now();
    let _pr = db.pagerank(0.85, 100, 1e-6);
    let elapsed = start.elapsed();
    
    println!("{:<40} {:>15.4}{}", "PageRank (增量缓存)", elapsed.as_secs_f64(), "s");
}

/// 基准测试：Betweenness Centrality
fn benchmark_betweenness(db: &GraphDB) {
    let start = Instant::now();
    let _bc = db.betweenness_centrality();
    let elapsed = start.elapsed();
    
    println!("{:<40} {:>15.4}{}", "Betweenness Centrality (近似)", elapsed.as_secs_f64(), "s");
}
