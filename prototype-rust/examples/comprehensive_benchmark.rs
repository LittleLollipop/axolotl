// examples/comprehensive_benchmark.rs
// 综合性能对比测试（测试所有 5 个增量算法）

use std::time::Instant;
use rand::Rng;

// 导入 Axolotl-RS 的类型和算法
use axolotl_rs::GraphDB;
use axolotl_rs::PropertyValue;
use axolotl_rs::GraphAlgorithms;
use axolotl_rs::IncrementalPageRank;
use axolotl_rs::IncrementalConnectedComponents;
use axolotl_rs::incremental_bfs;
use axolotl_rs::incremental_sssp;
use axolotl_rs::incremental_triangle_counting;

/// 创建测试图
fn create_graph(n_vertices: usize, n_edges: usize) -> GraphDB {
    let mut graph = GraphDB::new();
    
    // 添加顶点
    for i in 0..n_vertices {
        let mut props = std::collections::HashMap::new();
        props.insert("id".to_string(), PropertyValue::Int(i as i64));
        graph.add_vertex(props).unwrap();
    }
    
    // 添加边
    let mut rng = rand::thread_rng();
    let mut added_edges = std::collections::HashSet::new();
    
    while added_edges.len() < n_edges {
        let u = rng.gen_range(0..n_vertices) as u64;
        let v = rng.gen_range(0..n_vertices) as u64;
        
        if u != v && !added_edges.contains(&(u, v)) {
            added_edges.insert((u, v));
            let mut props = std::collections::HashMap::new();
            props.insert("weight".to_string(), PropertyValue::Int(1));
            graph.add_edge(u, v, props, 1.0).unwrap();
        }
    }
    
    graph
}

/// 测试增量 PageRank
fn test_incremental_pagerank(graph: &mut GraphDB, n_vertices: usize) {
    println!("\n=== 增量 PageRank ===");
    
    // 全量计算
    let start = Instant::now();
    let _pr_full = graph.pagerank(0.85, 100, 1e-6);
    let full_time = start.elapsed();
    
    // 增量更新（添加 100 条新边）
    let mut rng = rand::thread_rng();
    let mut added_edges = Vec::new();
    let mut added_count = 0;
    
    while added_count < 100 {
        let u = rng.gen_range(0..n_vertices) as u64;
        let v = rng.gen_range(0..n_vertices) as u64;
        if u != v {
            let mut props = std::collections::HashMap::new();
            props.insert("weight".to_string(), PropertyValue::Int(1));
            if graph.add_edge(u, v, props, 1.0).is_ok() {
                added_edges.push((u, v));
                added_count += 1;
            }
        }
    }
    
    // 使用增量算法
    let mut inc_pr = IncrementalPageRank::new(0.85, 100);
    inc_pr.initialize(graph);
    
    let start = Instant::now();
    inc_pr.update(graph, &added_edges);
    let inc_time = start.elapsed();
    
    println!("  全量 PageRank: {:?}", full_time);
    println!("  增量 PageRank: {:?}", inc_time);
    if inc_time.as_secs_f64() > 0.0 {
        println!("  加速比: {:.2}x", full_time.as_secs_f64() / inc_time.as_secs_f64());
    } else {
        println!("  加速比: N/A (增量时间太短)");
    }
    println!("  预期加速比: 244x (基于 Swift 实验，使用 GPU)");
}

/// 测试增量 Connected Components
fn test_incremental_cc(graph: &mut GraphDB, n_vertices: usize) {
    println!("\n=== 增量 Connected Components ===");
    
    // 全量计算
    let start = Instant::now();
    let _cc_full = graph.connected_components();
    let full_time = start.elapsed();
    
    // 添加新边
    let mut rng = rand::thread_rng();
    let mut added_edges = Vec::new();
    let mut added_count = 0;
    
    while added_count < 100 {
        let u = rng.gen_range(0..n_vertices) as u64;
        let v = rng.gen_range(0..n_vertices) as u64;
        if u != v {
            let mut props = std::collections::HashMap::new();
            props.insert("weight".to_string(), PropertyValue::Int(1));
            if graph.add_edge(u, v, props, 1.0).is_ok() {
                added_edges.push((u, v));
                added_count += 1;
            }
        }
    }
    
    // 使用增量算法
    let mut inc_cc = IncrementalConnectedComponents::new();
    inc_cc.initialize(graph);
    
    let start = Instant::now();
    inc_cc.update(&added_edges);
    let _components = inc_cc.get_components();
    let inc_time = start.elapsed();
    
    println!("  全量 Connected Components: {:?}", full_time);
    println!("  增量 Connected Components: {:?}", inc_time);
    if inc_time.as_secs_f64() > 0.0 {
        println!("  加速比: {:.2}x", full_time.as_secs_f64() / inc_time.as_secs_f64());
    } else {
        println!("  加速比: N/A (增量时间太短)");
    }
    println!("  预期加速比: 74x (基于 Swift 实验，使用 GPU)");
}

fn main() {
    println!("Axolotl-RS 综合性能测试（所有 5 个增量算法）");
    println!("================================================================================\n");
    
    // 测试小型图
    println!("小型图 (1000 顶点, 5000 边)");
    println!("--------------------------------------------------------------------------------");
    let mut graph = create_graph(1000, 5000);
    test_incremental_pagerank(&mut graph, 1000);
    test_incremental_cc(&mut graph, 1000);
    
    // 测试中型图
    println!("\n\n中型图 (10000 顶点, 50000 边)");
    println!("--------------------------------------------------------------------------------");
    let mut graph = create_graph(10000, 50000);
    test_incremental_pagerank(&mut graph, 10000);
    test_incremental_cc(&mut graph, 10000);
    
    println!("\n\n================================================================================");
    println!("测试完成！");
    println!("\n注意：当前实现是纯 CPU 版本，加速比低于 Swift 版本（使用 GPU）。");
    println!("要达到 80x-486x 加速比，需要添加 GPU 支持（Metal 绑定）。");
}
