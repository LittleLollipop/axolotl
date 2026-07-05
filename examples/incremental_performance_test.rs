// examples/incremental_performance_test.rs
// 测试增量 PageRank 的性能

use std::collections::HashMap;
use std::time::Instant;
use rand::Rng;
use axolotl_rs::GraphAlgorithms;

fn main() {
    println!("增量 PageRank 性能测试");
    println!("================================================================================\n");
    
    // 创建测试图
    let n_vertices = 10000;
    let n_edges = 50000;
    
    println!("创建测试图（{} 顶点, {} 边）...", n_vertices, n_edges);
    
    let mut graph = axolotl_rs::GraphDB::new();
    
    // 添加顶点
    for i in 0..n_vertices {
        let mut props = HashMap::new();
        props.insert("id".to_string(), axolotl_rs::PropertyValue::Int(i));
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
            let mut props = HashMap::new();
            props.insert("weight".to_string(), axolotl_rs::PropertyValue::Int(1));
            graph.add_edge(u, v, props, 1.0).unwrap();
        }
    }
    
    println!("图创建完成！\n");
    
    // 测试 1：全量 PageRank
    println!("=== 测试 1：全量 PageRank ===");
    let start = Instant::now();
    let _cpu_pr = graph.pagerank(0.85, 100, 1e-6);
    let full_time = start.elapsed();
    println!("耗时: {:?}", full_time);
    
    // 测试 2：增量 PageRank（初始化）
    println!("\n=== 测试 2：增量 PageRank（初始化）===");
    let mut inc_pr = axolotl_rs::IncrementalPageRank::new(0.85, 100, 1e-6);
    let start = Instant::now();
    inc_pr.initialize(&graph);
    let init_time = start.elapsed();
    println!("耗时: {:?}", init_time);
    println!("加速比: {:.2}x", full_time.as_secs_f64() / init_time.as_secs_f64());
    
    // 测试 3：增量 PageRank（更新 - 添加 100 条新边）
    println!("\n=== 测试 3：增量 PageRank（增量更新）===");
    
    let mut new_edges = Vec::new();
    let mut added_count = 0;
    
    while added_count < 100 {
        let u = rng.gen_range(0..n_vertices) as u64;
        let v = rng.gen_range(0..n_vertices) as u64;
        if u != v {
            let mut props = HashMap::new();
            props.insert("weight".to_string(), axolotl_rs::PropertyValue::Int(1));
            if graph.add_edge(u, v, props, 1.0).is_ok() {
                new_edges.push((u, v));
                added_count += 1;
            }
        }
    }
    
    let start = Instant::now();
    inc_pr.update(&graph, &new_edges);
    let update_time = start.elapsed();
    println!("耗时: {:?}", update_time);
    println!("全量耗时: {:?}", full_time);
    
    if update_time.as_secs_f64() > 0.0 {
        println!("加速比: {:.2}x", full_time.as_secs_f64() / update_time.as_secs_f64());
    } else {
        println!("加速比: N/A (增量时间太短)");
    }
    
    // 验证增量更新的结果
    let inc_pr_values = inc_pr.get_pagerank();
    let sum: f64 = inc_pr_values.values().sum();
    println!("\n增量更新后 PR 值之和: {:.6} (应该接近 1.0)", sum);
    
    println!("\n================================================================================");
    println!("结论：");
    println!("- 初始化耗时 ≈ 全量计算耗时（预期）");
    println!("- 增量更新耗时应该远小于全量计算（如果实现正确）");
    println!("- 当前实现：增量更新 = 重新运行幂迭代（使用缓存作为初始值）");
    println!("- 真正的增量更新应该只计算受影响顶点（更复杂）");
}

#[test]
fn test_incremental_performance() {
    main();
}
