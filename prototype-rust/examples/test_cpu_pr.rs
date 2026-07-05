// examples/test_cpu_pr.rs
// 测试 CPU 版本的 PageRank 是否正确

use axolotl_rs::algorithms::GraphAlgorithms;

fn main() {
    println!("=== 测试 CPU 版本的 PageRank ===\n");
    
    // 创建一个简单的图
    // 5 个顶点：0->1, 0->2, 1->2, 2->0, 3->4
    let mut graph = axolotl_rs::GraphDB::new();
    
    // 添加顶点
    for i in 0..5 {
        graph.add_vertex(i, serde_json::json!({"id": i}));
    }
    
    // 添加边
    graph.add_edge(0, 1, serde_json::json!({}), 1.0);
    graph.add_edge(0, 2, serde_json::json!({}), 1.0);
    graph.add_edge(1, 2, serde_json::json!({}), 1.0);
    graph.add_edge(2, 0, serde_json::json!({}), 1.0);
    graph.add_edge(3, 4, serde_json::json!({}), 1.0);
    
    // 计算 PageRank
    let pr = graph.pagerank(50, 1e-6);
    
    println!("PageRank 结果：");
    for (vertex_id, &score) in &pr {
        println!("  顶点 {}: {}", vertex_id, score);
    }
    
    let sum: f64 = pr.values().sum();
    println!("\nPR 值之和：{}", sum);
    
    println!("\n=== 测试完成 ===");
}
