// examples/test_bfs_edgeblock.rs
// 测试 BFS 使用 EdgeBlock 格式

use std::time::Instant;

fn main() {
    println!("=== 测试 BFS (EdgeBlock 格式) ===\n");
    
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
    
    // 转换成 GPU EdgeBlock 格式
    println!("\n转换成 EdgeBlock 格式...");
    let start = Instant::now();
    let gpu_eb = axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph::from_csr(
        &csr.offsets,
        &csr.targets,
        csr.vertex_count,
    );
    let convert_time = start.elapsed();
    println!("  转换时间：{:?}", convert_time);
    println!("  EdgeBlock 数量：{}", gpu_eb.blocks.len() / axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph::BLOCK_SIZE_U32);
    
    // 创建 GPU 加速器
    let gpu = axolotl_rs::gpu::GPUAccelerator::new();
    if gpu.is_err() {
        println!("⚠️  没有 GPU，跳过测试");
        return;
    }
    let gpu = gpu.unwrap();
    
    // 测试 BFS（从顶点 0 开始）
    println!("\n测试 BFS（从顶点 0 开始）...");
    let start = Instant::now();
    let distances = gpu.compute_bfs_edgeblock(&gpu_eb, 0);
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
