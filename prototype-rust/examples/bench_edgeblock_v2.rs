// examples/bench_edgeblock_v2.rs
// EdgeBlock 统一内存重构性能对比
// 使用与 PEPRFORMANCE_COMPARISON_REPORT.md 相同的数据集
// 对比：旧架构(HashMap+CSR) vs 新架构(EdgeBlock native)

use axolotl_rs::*;
use axolotl_rs::gpu::GPUAccelerator;
use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;
use std::collections::HashMap;
use std::time::Instant;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {
    println!("# Axolotl EdgeBlock 统一内存重构：性能对比报告\n");
    println!("**日期**：2026-07-07\n");
    println!("**硬件**：Apple M4 统一内存\n");
    println!("**对比**：旧架构（PersistentGraph HashMap + CSR 转换）vs 新架构（GPUEdgeBlockGraph 原生格式）\n");

    let datasets = vec![
        ("dataset_1k_10k.edgelist",   1_000,   10_000),
        ("dataset_10k_100k.edgelist", 10_000, 100_000),
        ("dataset_100k_1m.edgelist", 100_000, 1_000_000),
    ];

    for (filename, expected_v, expected_e) in &datasets {
        println!("## {}", filename);
        println!("({} 顶点, {} 边)\n", expected_v, expected_e);

        let (edges, vertex_count) = load_edgelist(filename);
        println!("实际: {} 顶点, {} 边\n", vertex_count, edges.len());

        // ════════════════════════════════════════
        // 数据加载对比
        // ════════════════════════════════════════
        println!("### 数据加载\n");

        // 旧: HashMap 构建
        let (t_old_build, pg) = benchmark("HashMap 构建", || {
            let mut pg = persistence::PersistentGraph {
                vertices: HashMap::new(), edges: HashMap::new(),
                file_path: String::new(),
                index_manager: index::IndexManager::new(),
            };
            for v in 0..vertex_count as u64 {
                pg.add_vertex(v, HashMap::new());
            }
            for &(u, v) in &edges {
                pg.add_edge(u, v, 1.0, HashMap::new());
            }
            pg
        });

        // 旧: to_csr
        let (t_csr, csr) = benchmark("to_csr() 转换", || {
            pg.to_csr()
        });

        // 旧: from_csr → EdgeBlock
        let (t_old_to_eb, gpu_graph_old) = benchmark("from_csr → EdgeBlock", || {
            GPUEdgeBlockGraph::from_csr(&csr.offsets, &csr.targets, vertex_count as u32)
        });

        // 新: 直接构建 EdgeBlock
        let (t_new_build, eb_new) = benchmark("EdgeBlock 直接构建", || {
            let mut eb = GPUEdgeBlockGraph::new();
            for v in 0..vertex_count as u64 {
                eb.add_vertex(v, HashMap::new());
            }
            for &(u, v) in &edges {
                eb.add_edge(u, v, 1.0);
            }
            eb
        });

        println!("| 阶段 | 旧架构 | 新架构 | 节省 |");
        println!("|------|--------|--------|------|");
        {
            let total_old = t_old_build.as_secs_f64() + t_csr.as_secs_f64() + t_old_to_eb.as_secs_f64();
            let total_new = t_new_build.as_secs_f64();
            let saved = format_dur(t_old_build.as_secs_f64() + t_csr.as_secs_f64() + t_old_to_eb.as_secs_f64() - t_new_build.as_secs_f64());
            println!("| 构建+CSR+EdgeBlock | {:.2}ms | {:.2}ms | {} |",
                total_old * 1000., total_new * 1000., saved);
            println!("| - HashMap 构建 | {:.2}ms | — | — |", t_old_build.as_secs_f64() * 1000.);
            println!("| - to_csr() 转换 | {:.2}ms | — | — |", t_csr.as_secs_f64() * 1000.);
            println!("| - from_csr→EdgeBlock | {:.2}ms | — | — |", t_old_to_eb.as_secs_f64() * 1000.);
            println!("| - EdgeBlock 直接构建 | — | {:.2}ms | — |", t_new_build.as_secs_f64() * 1000.);
        }
        println!();

        // ════════════════════════════════════════
        // GPU PageRank 对比
        // ════════════════════════════════════════
        println!("### GPU PageRank\n");

        let gpu = match GPUAccelerator::new() {
            Ok(g) => g,
            Err(e) => { println!("GPU 不可用: {}\n", e); continue; }
        };

        let n = vertex_count as usize;
        let initial_pr = vec![1.0f32 / n as f32; n];
        let all_vertices: Vec<u32> = (0..n as u32).collect();
        let iterations = 100;

        // ── 旧架构：CSR → GPU full PR ──
        // 计算 out_degrees
        let mut out_degrees_csr = vec![0u32; n];
        for v in 0..n {
            let s = csr.offsets[v] as usize;
            let e = csr.offsets[v + 1] as usize;
            out_degrees_csr[v] = (e - s) as u32;
        }

        // 预热
        let mut pr = initial_pr.clone();
        let _ = gpu.compute_full_pagerank(&csr.reverse_offsets, &csr.reverse_targets,
            &out_degrees_csr, &pr, n as u32, 0.85);

        let t0 = Instant::now();
        for _ in 0..iterations {
            pr = gpu.compute_full_pagerank(&csr.reverse_offsets, &csr.reverse_targets,
                &out_degrees_csr, &pr, n as u32, 0.85);
        }
        let t_old_gpu_pr = t0.elapsed();

        let sum_old: f32 = pr.iter().sum();
        println!("| 实现 | 时间 (ms/iter) | PR 和 | 说明 |");
        println!("|------|---------------|-------|------|");
        println!("| GPU CSR full PR | {:.2} | {} | 旧架构 |",
            t_old_gpu_pr.as_secs_f64() * 1000.0 / iterations as f64,
            format_sum(sum_old));

        // ── 新架构：EdgeBlock → GPU (无缓存) ──
        let out_degrees_new = eb_new.compute_out_degrees();
        let mut pr2 = initial_pr.clone();
        let _ = gpu.compute_incremental_pagerank_edgeblock(
            &eb_new, &pr2, &all_vertices, &out_degrees_new, 0.85);

        let t0 = Instant::now();
        for _ in 0..iterations {
            pr2 = gpu.compute_incremental_pagerank_edgeblock(
                &eb_new, &pr2, &all_vertices, &out_degrees_new, 0.85);
        }
        let t_new_gpu_cold = t0.elapsed();
        let sum_new: f32 = pr2.iter().sum();
        println!("| GPU EdgeBlock (冷) | {:.2} | {} | 首次分配 buffer |",
            t_new_gpu_cold.as_secs_f64() * 1000.0 / iterations as f64,
            format_sum(sum_new));

        // ── 新架构：EdgeBlock → GPU (缓存) ──
        let mut cache = gpu::EdgeBlockCache::new();
        let mut pr3 = initial_pr.clone();
        let _ = gpu.compute_incremental_pagerank_edgeblock_cached(
            &eb_new, &pr3, &all_vertices, &out_degrees_new, 0.85,
            Some(&mut cache));

        let t0 = Instant::now();
        for _ in 0..iterations {
            pr3 = gpu.compute_incremental_pagerank_edgeblock_cached(
                &eb_new, &pr3, &all_vertices, &out_degrees_new, 0.85,
                Some(&mut cache));
        }
        let t_new_gpu_cached = t0.elapsed();
        let sum_cached: f32 = pr3.iter().sum();
        println!("| GPU EdgeBlock (缓存) | {:.2} | {} | buffer 复用 |",
            t_new_gpu_cached.as_secs_f64() * 1000.0 / iterations as f64,
            format_sum(sum_cached));

        // ── CPU f64 版本（基准）──
        let (t_cpu_pr, pr_cpu) = benchmark("CPU PageRank f64", || {
            let mut out_degs = vec![0u32; n];
            let mut in_edges = vec![Vec::new(); n];
            for (u, v) in &edges {
                out_degs[*u as usize] += 1;
                in_edges[*v as usize].push(*u);
            }
            let dangling: Vec<usize> = (0..n).filter(|&v| out_degs[v] == 0).collect();
            let mut pr = vec![1.0f64 / n as f64; n];
            for _ in 0..iterations {
                let ds: f64 = dangling.iter().map(|&v| pr[v]).sum();
                let dc = ds / n as f64;
                let mut np = vec![0.0f64; n];
                for v in 0..n {
                    let mut c = 0.0f64;
                    for &u in &in_edges[v] {
                        c += pr[u as usize] / out_degs[u as usize] as f64;
                    }
                    np[v] = (1.0 - 0.85) / n as f64 + 0.85 * (c + dc);
                }
                pr = np;
            }
            pr
        });
        let cpu_sum: f64 = pr_cpu.iter().sum();
        println!("| CPU f64 (基准) | {:.2} | {} | 精确 |",
            t_cpu_pr.as_secs_f64() * 1000.0 / iterations as f64,
            format_sum_f64(cpu_sum));

        // ── GPU BFS ──
        println!("\n### GPU BFS\n");

        let bfs_iter = 10;
        let source = 0u32;

        // 旧: EdgeBlock from CSR
        let t0 = Instant::now();
        for _ in 0..bfs_iter {
            let _ = gpu.compute_bfs_edgeblock(&gpu_graph_old, source);
        }
        let t_old_bfs = t0.elapsed();

        // 新: EdgeBlock native
        let t0 = Instant::now();
        for _ in 0..bfs_iter {
            let _ = gpu.compute_bfs_edgeblock(&eb_new, source);
        }
        let t_new_bfs = t0.elapsed();

        // CPU BFS
        let t0 = Instant::now();
        for _ in 0..bfs_iter {
            let mut visited = vec![false; n];
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(source as usize);
            visited[source as usize] = true;
            while let Some(u) = queue.pop_front() {
                for &v in &eb_new.out_neighbors_by_idx(u) {
                    let vi = eb_new.id_to_idx[&v];
                    if !visited[vi] {
                        visited[vi] = true;
                        queue.push_back(vi);
                    }
                }
            }
        }
        let t_cpu_bfs = t0.elapsed();

        println!("| 实现 | 时间 (ms/iter) | 说明 |");
        println!("|------|---------------|------|");
        println!("| GPU BFS (旧 EdgeBlock) | {:.2} | CSR→EdgeBlock |",
            t_old_bfs.as_secs_f64() * 1000.0 / bfs_iter as f64);
        println!("| GPU BFS (新 EdgeBlock) | {:.2} | 原生 EdgeBlock |",
            t_new_bfs.as_secs_f64() * 1000.0 / bfs_iter as f64);
        println!("| CPU BFS (EdgeBlock) | {:.2} | 原生 EdgeBlock |",
            t_cpu_bfs.as_secs_f64() * 1000.0 / bfs_iter as f64);
        println!();

        // ── 端到端汇总 ──
        println!("### 端到端耗时（构建+算法）\n");
        let old_end2end_pr = t_old_build.as_secs_f64() + t_csr.as_secs_f64() + t_old_to_eb.as_secs_f64()
            + t_old_gpu_pr.as_secs_f64() / iterations as f64;
        let new_end2end_pr = t_new_build.as_secs_f64()
            + t_new_gpu_cached.as_secs_f64() / iterations as f64;
        let speedup = old_end2end_pr / new_end2end_pr.max(1e-9);

        println!("| 算法 | 旧架构 | 新架构(缓存) | 加速比 |");
        println!("|------|--------|-------------|--------|");
        println!("| PageRank | {:.2}ms | {:.2}ms | **{:.1}x** |",
            old_end2end_pr * 1000., new_end2end_pr * 1000., speedup);

        let old_e2e_bfs = t_old_build.as_secs_f64() + t_csr.as_secs_f64() + t_old_to_eb.as_secs_f64()
            + t_old_bfs.as_secs_f64() / bfs_iter as f64;
        let new_e2e_bfs = t_new_build.as_secs_f64()
            + t_new_bfs.as_secs_f64() / bfs_iter as f64;
        println!("| BFS | {:.2}ms | {:.2}ms | **{:.1}x** |",
            old_e2e_bfs * 1000., new_e2e_bfs * 1000.,
            old_e2e_bfs / new_e2e_bfs.max(1e-9));
        println!();
    }

    // ── 汇总表 ──
    println!("# 汇总\n");
    println!("| 规模 | 算法 | 旧: 构建+CSR | 新: 构建 | GPU 冷 | GPU 缓存 | 端到端加速 |");
    println!("|------|------|-------------|----------|--------|---------|-----------|");

    // 简化：打印汇总行
    println!("\n*完整数据见上述各节。*\n");
    println!("---\n");
    println!("**测试时间**: 2026-07-07");
    println!("**测试者**: AI Assistant");
    println!("**代码位置**: `prototype-rust/examples/bench_edgeblock_v2.rs`");
}

fn load_edgelist(filename: &str) -> (Vec<(u64, u64)>, u32) {
    let path = format!("examples/{}", filename);
    let file = File::open(&path).expect("Failed to open file");
    let reader = BufReader::new(file);

    let mut edges = Vec::new();
    let mut max_vertex = 0u64;

    for line in reader.lines() {
        let line = line.expect("Failed to read line");
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let u: u64 = parts[0].parse().unwrap_or(0);
            let v: u64 = parts[1].parse().unwrap_or(0);
            edges.push((u, v));
            max_vertex = max_vertex.max(u).max(v);
        }
    }
    (edges, (max_vertex + 1) as u32)
}

fn benchmark<T, F: FnOnce() -> T>(label: &str, f: F) -> (std::time::Duration, T) {
    let t0 = Instant::now();
    let result = f();
    let elapsed = t0.elapsed();
    (elapsed, result)
}

fn format_dur(secs: f64) -> String {
    if secs > 1.0 { format!("{:.2}s", secs) }
    else if secs > 0.001 { format!("{:.2}ms", secs * 1000.) }
    else { format!("{:.0}µs", secs * 1_000_000.) }
}

fn format_sum(s: f32) -> String {
    let err = (s - 1.0).abs();
    if err < 1e-4 { format!("1.000 (Δ={:.1e})", err) }
    else { format!("{:.6} ✗", s) }
}

fn format_sum_f64(s: f64) -> String {
    let err = (s - 1.0).abs();
    format!("{:.10} (Δ={:.1e})", s, err)
}
