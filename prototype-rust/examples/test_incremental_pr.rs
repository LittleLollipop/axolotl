// examples/test_incremental_pr.rs
// 测试增量 PageRank 的 GPU 计算是否正确

use axolotl_rs::gpu::GPUAccelerator;
use axolotl_rs::csr_graph::CSRGraph;
use std::collections::HashMap;

fn main() {
    println!("=== 测试增量 PageRank ===\n");
    
    // 1. 创建 GPU 加速器
    let gpu = GPUAccelerator::new().expect("无法创建 GPU 加速器");
    println!("✓ GPU 加速器创建成功\n");
    
    // 2. 创建一个简单的 CSR 图
    // 5 个顶点：0->1, 0->2, 1->2, 2->0, 3->4
    let mut csr = CSRGraph::new();
    
    // 添加顶点
    for i in 0..5 {
        let mut props = HashMap::new();
        props.insert("id".to_string(), axolotl_rs::PropertyValue::Int(i as i64));
        csr.add_vertex(i, props);
    }
    
    // 添加边
    let edges = vec![(0, 1), (0, 2), (1, 2), (2, 0), (3, 4)];
    csr.build_csr(&edges);
    
    println!("图结构：");
    println!("  顶点数：{}", csr.vertex_count);
    println!("  边数：{}", csr.total_edges);
    println!("  CSR offsets：{:?}", csr.offsets);
    println!("  CSR targets：{:?}", csr.targets);
    println!();
    
    // 3. 初始 PR 值
    let initial_pr = vec![0.2, 0.2, 0.2, 0.2, 0.2];
    println!("初始 PR 值：{:?}\n", initial_pr);
    
    // 4. 受影响顶点：0, 3
    let affected_vertices = vec![0, 3];
    println!("受影响顶点：{:?}\n", affected_vertices);
    
    // 5. 调用 GPU 计算
    let new_pr = gpu.compute_incremental_pagerank(
        &csr.offsets,
        &csr.targets,
        &csr.reverse_offsets,
        &csr.reverse_targets,
        &initial_pr,
        &affected_vertices,
        csr.vertex_count as u32,
    );
    
    println!("GPU 计算结果：");
    println!("  新的 PR 值：{:?}", new_pr);
    println!();
    
    // 6. 手动计算期望的结果
    // 对于顶点 0：
    //   - 入边：2->0
    //   - PR(2) = 0.2，出度 = 1（2->0）
    //   - 贡献 = 0.85 * 0.2 / 1 = 0.17
    //   - teleportation = (1 - 0.85) / 5 = 0.03
    //   - 新的 PR(0) = 0.03 + 0.17 = 0.20
    
    // 对于顶点 3：
    //   - 入边：无
    //   - 贡献 = 0
    //   - teleportation = (1 - 0.85) / 5 = 0.03
    //   - 新的 PR(3) = 0.03
    
    // 但是，等等！GPU 内核的公式是：
    //   contribution = sum(PR[neighbor] / out_degree(neighbor))
    //   new_value = (1 - damping) / vertex_count + damping * contribution
    
    // 对于顶点 0：
    //   - neighbor = 2
    //   - PR[2] = 0.2
    //   - out_degree(2) = 1
    //   - contribution = 0.2 / 1 = 0.2
    //   - new_value = 0.03 + 0.85 * 0.2 = 0.03 + 0.17 = 0.20
    
    // 对于顶点 3：
    //   - 没有入边
    //   - contribution = 0
    //   - new_value = 0.03 + 0.85 * 0 = 0.03
    
    let expected_pr = vec![0.20, 0.2, 0.2, 0.03, 0.2];
    println!("期望的 PR 值：{:?}", expected_pr);
    println!();
    
    // 7. 验证结果
    let mut correct = true;
    for i in 0..5 {
        if (new_pr[i] - expected_pr[i]).abs() > 1e-6 {
            println!("❌ 顶点 {} 的 PR 值不正确：期望 {}，实际 {}", 
                     i, expected_pr[i], new_pr[i]);
            correct = false;
        }
    }
    
    if correct {
        println!("✅ GPU 计算结果正确！");
    }
    
    println!("\n=== 测试完成 ===");
}
