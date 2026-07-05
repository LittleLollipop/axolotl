// examples/gpu_pagerank_test.rs
// 测试 GPU PageRank 的正确性和性能

#[cfg(target_os = "macos")]
fn main() {
    use std::collections::HashMap;
    use std::time::Instant;
    use rand::Rng;
    use axolotl_rs::GraphAlgorithms;
    
    println!("GPU PageRank 测试");
    println!("================================================================================\n");
    
    // 创建测试图
    let mut graph = axolotl_rs::GraphDB::new();
    let n_vertices = 1000;
    let n_edges = 5000;
    
    println!("创建测试图（{} 顶点, {} 边）...", n_vertices, n_edges);
    
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
    
    // 测试 CPU PageRank
    println!("=== CPU PageRank ===");
    let start = Instant::now();
    let cpu_pr = graph.pagerank(0.85, 100, 1e-6);
    let cpu_time = start.elapsed();
    println!("耗时: {:?}", cpu_time);
    println!("PR 值数量: {}", cpu_pr.len());
    
    // 验证 CPU 结果
    let cpu_sum: f64 = cpu_pr.values().sum();
    println!("PR 值之和: {:.6} (应该接近 1.0)", cpu_sum);
    
    // 测试 GPU PageRank
    println!("\n=== GPU PageRank ===");
    
    let gpu_pr_obj = match axolotl_rs::GPUPageRank::new() {
        Ok(gpu) => gpu,
        Err(e) => {
            println!("❌ 创建 GPU PageRank 失败: {}", e);
            return;
        }
    };
    
    let start = Instant::now();
    let gpu_pr = gpu_pr_obj.compute(&graph, 0.85, 100, 1e-6);
    let gpu_time = start.elapsed();
    println!("耗时: {:?}", gpu_time);
    println!("PR 值数量: {}", gpu_pr.len());
    
    // 验证 GPU 结果
    let gpu_sum: f64 = gpu_pr.values().sum();
    println!("PR 值之和: {:.6} (应该接近 1.0)", gpu_sum);
    
    // 对比结果
    println!("\n=== 结果对比 ===");
    
    let mut diff_sum = 0.0f64;
    let mut max_diff = 0.0f64;
    let mut diff_count = 0;
    
    for (&vertex_id, &cpu_value) in &cpu_pr {
        if let Some(&gpu_value) = gpu_pr.get(&vertex_id) {
            let diff: f64 = (cpu_value - gpu_value).abs();
            diff_sum += diff;
            diff_count += 1;
            
            if diff > max_diff {
                max_diff = diff;
            }
        }
    }
    
    let avg_diff = if diff_count > 0 { diff_sum / diff_count as f64 } else { 0.0 };
    
    println!("平均差异: {:.10}", avg_diff);
    println!("最大差异: {:.10}", max_diff);
    println!("差异 > 0.001 的顶点数: {}", 
        cpu_pr.iter().filter(|(&v, &cpu_val)| {
            if let Some(&gpu_val) = gpu_pr.get(&v) {
                (cpu_val - gpu_val).abs() > 0.001
            } else {
                true
            }
        }).count()
    );
    
    // 性能对比
    println!("\n=== 性能对比 ===");
    println!("CPU 耗时: {:?}", cpu_time);
    println!("GPU 耗时: {:?}", gpu_time);
    
    if gpu_time.as_secs_f64() > 0.0 {
        println!("加速比: {:.2}x", cpu_time.as_secs_f64() / gpu_time.as_secs_f64());
    } else {
        println!("加速比: N/A (GPU 时间太短)");
    }
    
    // 判断结果是否正确
    if max_diff < 0.01 {
        println!("\n✅ GPU PageRank 结果正确！（与 CPU 差异很小）");
    } else {
        println!("\n⚠️  GPU PageRank 结果与 CPU 有差异，可能需要调试。");
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("❌ GPU 支持仅在 macOS 上可用（需要 Metal）");
    println!("   当前系统不支持 GPU 加速。");
}

#[test]
#[cfg(target_os = "macos")]
fn test_gpu_pagerank() {
    main();
}
