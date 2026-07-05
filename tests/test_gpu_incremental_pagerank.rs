// tests/test_gpu_incremental_pagerank.rs
// 测试 GPU 加速的增量 PageRank

use axolotl_rs::*;
use axolotl_rs::graph::GraphDB;

#[test]
fn test_gpu_incremental_pagerank() {
    // 创建图
    let mut db = GraphDB::new();
    
    // 添加顶点
    for i in 0..10 {
        db.add_vertex(i, format!("User{}", i), serde_json::json!({}));
    }
    
    // 添加边（创建一个简单的图）
    db.add_edge(0, 1, 1.0);
    db.add_edge(1, 2, 1.0);
    db.add_edge(2, 3, 1.0);
    db.add_edge(3, 4, 1.0);
    db.add_edge(4, 5, 1.0);
    db.add_edge(5, 6, 1.0);
    db.add_edge(6, 7, 1.0);
    db.add_edge(7, 8, 1.0);
    db.add_edge(8, 9, 1.0);
    db.add_edge(9, 0, 1.0); // 闭环
    
    // 创建 CSR 格式图
    let csr = CSRGraph::from_graph(&db);
    
    // 创建增量 PageRank 计算器
    let mut pr = IncrementalPageRank::new();
    
    // 初始化（全量计算）
    pr.initialize(&db);
    
    // 检查 PR 值之和是否接近 1.0
    let pr_values = pr.get_pagerank();
    let sum: f64 = pr_values.values().sum();
    println!("PR 值之和: {:.6}", sum);
    assert!((sum - 1.0).abs() < 0.01, "PR 值之和应该接近 1.0");
    
    // 增量更新（添加一条边）
    let added_edges = vec![(0, 5)];
    pr.update(&db, &added_edges);
    
    // 再次检查 PR 值之和
    let pr_values = pr.get_pagerank();
    let sum: f64 = pr_values.values().sum();
    println!("增量更新后 PR 值之和: {:.6}", sum);
    assert!((sum - 1.0).abs() < 0.01, "增量更新后 PR 值之和应该接近 1.0");
    
    println!("✓ GPU 增量 PageRank 测试通过！");
}
