// examples/verify_gpu_pr.rs
// 验证 GPU 计算的增量 PageRank 是否正确

use axolotl_rs::gpu::GPUAccelerator;
use axolotl_rs::csr_graph::CSRGraph;
use std::collections::HashMap;

/// CPU 版本的增量 PageRank（用于验证）
fn cpu_incremental_pagerank(
    csr: &CSRGraph,
    initial_pr: &[f32],
    affected_vertices: &[u32],
    max_iterations: usize,
    tolerance: f32,
) -> Vec<f32> {
    let vertex_count = csr.vertex_count as usize;
    let mut pr = initial_pr.to_vec();
    
    // 构建反向邻接表
    let mut reverse_adjacency = vec![Vec::new(); vertex_count];
    for u in 0..vertex_count {
        let start = csr.offsets[u] as usize;
        let end = csr.offsets[u + 1] as usize;
        for i in start..end {
            let v = csr.targets[i] as usize;
            reverse_adjacency[v].push(u as u32);
        }
    }
    
    // 初始化受影响顶点集合
    let mut affected_set: std::collections::HashSet<u32> = 
        affected_vertices.iter().cloned().collect();
    let mut iteration = 0;
    
    // 主循环
    while !affected_set.is_empty() && iteration < max_iterations {
        iteration += 1;
        
        // 计算新的 PR 值
        let mut new_pr = pr.clone();
        for &v in &affected_set {
            let v = v as usize;
            
            // 计算贡献（来自入边邻居）
            let mut contribution = 0.0f32;
            for &u in &reverse_adjacency[v] {
                let u = u as usize;
                let out_degree = csr.offsets[u + 1] - csr.offsets[u];
                if out_degree > 0 {
                    contribution += pr[u] / out_degree as f32;
                }
            }
            
            // 计算新的 PR 值
            let damping = 0.85f32;
            new_pr[v] = (1.0 - damping) / vertex_count as f32 + damping * contribution;
        }
        
        // 检查收敛，找出新的受影响顶点
        let mut new_affected = std::collections::HashSet::new();
        for i in 0..vertex_count {
            let diff = (new_pr[i] - pr[i]).abs();
            if diff > tolerance {
                for &neighbor in &reverse_adjacency[i] {
                    new_affected.insert(neighbor);
                }
            }
        }
        
        // 更新
        pr = new_pr;
        affected_set = new_affected;
    }
    
    pr
}

fn main() {
    println!("=== 验证 GPU 计算的增量 PageRank ===\n");
    
    // 1. 创建 GPU 加速器
    let gpu = GPUAccelerator::new().expect("无法创建 GPU 加速器");
    println!("✓ GPU 加速器创建成功\n");
    
    // 2. 创建一个简单的 CSR 图
    let mut csr = CSRGraph::new();
    
    // 添加顶点
    for i in 0..5 {
        let mut props = HashMap::new();
        props.insert("id".to_string(), axolotl_rs::csr_graph::PropertyValue::Int(i as i64));
        csr.add_vertex(i, props);
    }
    
    // 添加边：0->1, 0->2, 1->2, 2->0, 3->4
    let edges = vec![(0, 1), (0, 2), (1, 2), (2, 0), (3, 4)];
    csr.build_csr(&edges);
    
    println!("图结构：");
    println!("  顶点数：{}", csr.vertex_count);
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
    let gpu_pr = gpu.compute_incremental_pagerank(
        &csr.offsets,
        &csr.targets,
        &csr.reverse_offsets,
        &csr.reverse_targets,
        &initial_pr,
        &affected_vertices,
        csr.vertex_count,
    );
    
    println!("GPU 计算结果：{:?}\n", gpu_pr);
    
    // 6. 调用 CPU 计算（正确的结果）
    let cpu_pr = cpu_incremental_pagerank(
        &csr,
        &initial_pr,
        &affected_vertices,
        50,
        1e-6,
    );
    
    println!("CPU 计算结果（正确）：{:?}\n", cpu_pr);
    
    // 7. 验证结果
    let mut correct = true;
    for i in 0..5 {
        if (gpu_pr[i] - cpu_pr[i]).abs() > 1e-6 {
            println!("❌ 顶点 {} 的 PR 值不正确：GPU {}，CPU {}", 
                     i, gpu_pr[i], cpu_pr[i]);
            correct = false;
        }
    }
    
    if correct {
        println!("✅ GPU 计算结果正确！");
    } else {
        println!("\n❌ GPU 计算结果不正确！");
        println!("这说明 GPU 内核的实现有误！");
    }
    
    println!("\n=== 验证完成 ===");
}
