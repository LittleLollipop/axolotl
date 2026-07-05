// examples/test_incremental_pr.rs
// 测试增量 PageRank 的正确性（PR 值之和是否接近 1.0）

use std::collections::HashMap;
use axolotl_rs::graph::GraphDB;
use axolotl_rs::incremental_pagerank::IncrementalPageRank;

fn main() {
    println!("=== 测试增量 PageRank 的正确性 ===\n");
    
    // 创建一个简单的图
    let mut graph = GraphDB::new();
    
    // 添加 10 个顶点
    for i in 0..10 {
        let mut props = HashMap::new();
        props.insert("id".to_string(), axolotl_rs::PropertyValue::Int(i));
        graph.add_vertex(props).unwrap();
    }
    
    // 添加一些边（形成一个环）
    graph.add_edge(0, 1, HashMap::new(), 1.0).unwrap();
    graph.add_edge(1, 2, HashMap::new(), 1.0).unwrap();
    graph.add_edge(2, 3, HashMap::new(), 1.0).unwrap();
    graph.add_edge(3, 4, HashMap::new(), 1.0).unwrap();
    graph.add_edge(4, 0, HashMap::new(), 1.0).unwrap();
    
    // 添加一些额外的边（增加复杂度）
    graph.add_edge(5, 6, HashMap::new(), 1.0).unwrap();
    graph.add_edge(6, 7, HashMap::new(), 1.0).unwrap();
    graph.add_edge(7, 8, HashMap::new(), 1.0).unwrap();
    graph.add_edge(8, 9, HashMap::new(), 1.0).unwrap();
    graph.add_edge(9, 5, HashMap::new(), 1.0).unwrap();
    
    println!("图结构：");
    println!("  顶点数：{}", graph.vertex_count());
    println!("  边数：{}", graph.edge_count());
    
    // 初始化增量 PageRank
    let mut incremental_pr = IncrementalPageRank::new();
    incremental_pr.initialize(&graph);
    
    // 验证初始化后的 PR 值之和
    let pr_init = incremental_pr.get_pagerank();
    let sum_init: f64 = pr_init.values().sum();
    println!("\n初始化后 PR 值之和：{:.6}", sum_init);
    assert!((sum_init - 1.0).abs() < 0.01, "初始化后 PR 值之和应该接近 1.0");
    
    // 添加新边（触发增量更新）
    let added_edges = vec![(0, 5)];  // 连接两个环
    graph.add_edge(0, 5, HashMap::new(), 1.0).unwrap();
    
    println!("\n添加新边：{:?}", added_edges);
    println!("  新的边数：{}\n", graph.edge_count());
    
    incremental_pr.update(&graph, &added_edges);
    
    // 验证增量更新后的 PR 值之和
    let pr_updated = incremental_pr.get_pagerank();
    let sum_updated: f64 = pr_updated.values().sum();
    println!("\n增量更新后 PR 值之和：{:.6}", sum_updated);
    
    if (sum_updated - 1.0).abs() < 0.01 {
        println!("✅ 测试通过：PR 值之和接近 1.0");
    } else {
        println!("❌ 测试失败：PR 值之和不等于 1.0");
        println!("   差异：{:.6}", sum_updated - 1.0);
        
        // 打印每个顶点的 PR 值
        println!("\n每个顶点的 PR 值：");
        for (vertex_id, &pr_value) in &pr_updated {
            println!("  顶点 {}：{:.6}", vertex_id, pr_value);
        }
    }
}
