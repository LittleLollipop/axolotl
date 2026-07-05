// examples/test_incremental_bfs_edgeblock.rs
// 测试增量 BFS 使用 EdgeBlock 格式

use std::time::Instant;

fn main() {
    println!("=== 测试增量 BFS (EdgeBlock 格式) ===\n");
    
    // 创建测试图（5 个顶点，4 条边）
    let vertex_count = 5;
    let edges = vec![(0, 1), (0, 2), (1, 3), (2, 3), (3, 4)];
    
    // 构建 CSR 图
    let mut csr = axolotl_rs::csr_graph::CSRGraph::new();
    for i in 0..vertex_count {
        let mut props = std::collections::HashMap::new();
        props.insert("id".to_string(), axolotl_rs::csr_graph::PropertyValue::Int(i as i64));
        csr.add_vertex(i as u64, props);
    }
    csr.build_csr(&edges);
    
    println!("CSR 图：{} 个顶点，{} 条边", csr.vertex_count, csr.total_edges);
    
    // 创建增量 BFS（使用 EdgeBlock）
    let bfs = axolotl_rs::incremental_bfs_edgeblock::IncrementalBFS_EdgeBlock::new(&csr);
    if bfs.is_err() {
        println!("⚠️  没有 GPU，跳过测试");
        return;
    }
    let bfs = bfs.unwrap();
    
    // 初始距离数组（从顶点 0 开始）
    let mut initial_distances = vec![u32::MAX; vertex_count];
    initial_distances[0] = 0;
    
    // 受影响顶点：0
    let affected_vertices = vec![0];
    
    // 计算增量 BFS
    println!("\n计算增量 BFS（从顶点 0 开始）...");
    let start = Instant::now();
    let distances = bfs.compute(&initial_distances, &affected_vertices);
    let bfs_time = start.elapsed();
    println!("  BFS 时间：{:?}", bfs_time);
    println!("  距离：{:?}", distances);
    
    // 验证结果
    println!("\n验证结果：");
    assert_eq!(distances[0], 0, "顶点 0 的距离应该是 0");
    assert_eq!(distances[1], 1, "顶点 1 的距离应该是 1");
    assert_eq!(distances[2], 1, "顶点 2 的距离应该是 1");
    assert_eq!(distances[3], 2, "顶点 3 的距离应该是 2");
    assert_eq!(distances[4], 3, "顶点 4 的距离应该是 3");
    println!("  ✅ 结果正确！");
    
    println!("\n=== 测试完成 ===");
}
