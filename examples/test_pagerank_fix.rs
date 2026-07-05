// examples/test_pagerank_fix.rs
// 测试 PageRank 修复（验证 PR 值之和为 1.0）

use axolotl_rs::*;
use axolotl_rs::gpu::GPUAccelerator;
use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;

fn main() {
    println!("=== 测试 PageRank 修复（验证 PR 值之和为 1.0）===\n");
    
    // 创建测试图（使用 CSRGraph）
    let mut graph = CSRGraph::new();
    
    // 添加顶点
    for i in 0..5 {
        graph.add_vertex(i as u64, std::collections::HashMap::new());
    }
    
    // 添加边（创建一个有悬挂顶点的图）
    let edges = vec![
        (0, 1),
        (0, 2),
        (1, 2),
        (2, 0),
        (2, 1),
    ];
    
    graph.build_csr(&edges);
    
    let vertex_count = graph.vertex_count;
    
    println!("图结构：");
    println!("  顶点数: {}", vertex_count);
    println!("  边: 0->1, 0->2, 1->2, 2->0, 2->1");
    println!("  悬挂顶点: 3, 4（出度为 0）");
    println!();
    
    // 创建 GPU 加速器
    let gpu = GPUAccelerator::new().expect("Failed to create GPU accelerator");
    
    // 转换为 GPU EdgeBlock 格式
    let gpu_graph = GPUEdgeBlockGraph::from_csr(
        &graph.offsets,
        &graph.targets,
        vertex_count,
    );
    
    // 测试 1：增量更新版本
    println!("=== 测试 1：增量更新版本（compute_incremental_pagerank_edgeblock）===");
    test_incremental(&gpu, &gpu_graph, vertex_count);
    
    // 测试 2：全量并发版本
    println!("\n=== 测试 2：全量并发版本（compute_full_pagerank）===");
    test_full(&gpu, &graph);
}

fn test_incremental(gpu: &GPUAccelerator, gpu_graph: &GPUEdgeBlockGraph, vertex_count: u32) {
    let vertex_count_usize = vertex_count as usize;
    let damping_factor = 0.85;
    let iterations = 20;
    
    // 初始化 PR 值
    let mut pr = vec![1.0f32 / vertex_count as f32; vertex_count_usize];
    
    // 所有顶点都是受影响顶点（第一次迭代）
    let affected_vertices: Vec<u32> = (0..vertex_count).collect();
    
    // 计算 out_degrees（从 GPUEdgeBlockGraph 的 block_counts 和 blocks 计算）
    let mut out_degrees = vec![0u32; vertex_count_usize];
    for v in 0..vertex_count_usize {
        let mut degree = 0;
        let start = gpu_graph.vertices[v] as usize;
        let count = gpu_graph.block_counts[v] as usize;
        
        for b in 0..count {
            let block_idx = (start + b) * GPUEdgeBlockGraph::BLOCK_SIZE_U32;
            let edge_count = gpu_graph.blocks[block_idx + 1] as usize;
            degree += edge_count;
        }
        
        out_degrees[v] = degree as u32;
    }
    
    println!("out_degrees: {:?}", out_degrees);
    println!();
    
    println!("初始 PR 值: {:?}", pr);
    println!("PR 值之和: {:.6}", pr.iter().sum::<f32>());
    println!();
    
    // 迭代
    for iter in 0..iterations {
        let new_pr = gpu.compute_incremental_pagerank_edgeblock(
            gpu_graph,
            &pr,
            &affected_vertices,
            &out_degrees,
            damping_factor,
        );
        
        let pr_sum: f32 = new_pr.iter().sum();
        
        if iter % 5 == 0 || iter == iterations - 1 {
            println!("迭代 {}: PR 值之和 = {:.10}", iter + 1, pr_sum);
        }
        
        pr = new_pr;
    }
    
    println!("\n最终 PR 值: {:?}", pr);
    println!("最终 PR 值之和: {:.10}", pr.iter().sum::<f32>());
    println!("是否为 1.0: {}", (pr.iter().sum::<f32>() - 1.0).abs() < 1e-6);
}

fn test_full(gpu: &GPUAccelerator, graph: &CSRGraph) {
    let vertex_count = graph.vertex_count as usize;
    let damping_factor = 0.85;
    let iterations = 20;
    
    // 初始化 PR 值
    let mut pr = vec![1.0f32 / vertex_count as f32; vertex_count];
    
    // 计算 out_degrees
    let mut out_degrees = vec![0u32; vertex_count];
    for v in 0..vertex_count {
        let start = graph.offsets[v] as usize;
        let end = graph.offsets[v + 1] as usize;
        out_degrees[v] = (end - start) as u32;
    }
    
    println!("out_degrees: {:?}", out_degrees);
    println!();
    
    println!("初始 PR 值: {:?}", pr);
    println!("PR 值之和: {:.6}", pr.iter().sum::<f32>());
    println!();
    
    // 迭代
    for iter in 0..iterations {
        let new_pr = gpu.compute_full_pagerank(
            &graph.reverse_offsets,
            &graph.reverse_targets,
            &out_degrees,
            &pr,
            graph.vertex_count,
            damping_factor,
        );
        
        let pr_sum: f32 = new_pr.iter().sum();
        
        if iter % 5 == 0 || iter == iterations - 1 {
            println!("迭代 {}: PR 值之和 = {:.10}", iter + 1, pr_sum);
        }
        
        pr = new_pr;
    }
    
    println!("\n最终 PR 值: {:?}", pr);
    println!("最终 PR 值之和: {:.10}", pr.iter().sum::<f32>());
    println!("是否为 1.0: {}", (pr.iter().sum::<f32>() - 1.0).abs() < 1e-6);
}
