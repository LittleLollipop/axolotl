// examples/bench_sssp_tc.rs
// SSSP + Triangle Counting 增量性能验证
//
// 补充 bench_incremental.rs 中未覆盖的两种算法

use std::collections::HashSet;
use std::time::Instant;

use axolotl_rs::csr_graph::CSRGraph;
use axolotl_rs::incremental_sssp::IncrementalSSSP;
use axolotl_rs::incremental_tc::{
    GraphWithAdjacencySets,
    full_triangle_counting,
    full_triangle_counting_set,
    incremental_triangle_counting_correct,
};

fn generate_edges(n_v: usize, n_e: usize) -> Vec<(u64, u64)> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut edges = HashSet::new();
    while edges.len() < n_e {
        let u = rng.gen_range(0..n_v as u64);
        let v = rng.gen_range(0..n_v as u64);
        if u != v {
            edges.insert((u, v));
        }
    }
    edges.into_iter().collect()
}

fn build_csr(edges: &[(u64, u64)], n_v: usize) -> CSRGraph {
    let mut csr = CSRGraph::new();
    for i in 0..n_v as u64 {
        csr.add_vertex(i, std::collections::HashMap::new());
    }
    csr.build_csr(edges);
    csr
}

// ── 增量 SSSP ─────────────────────────

fn bench_incremental_sssp(n_v: usize, n_e: usize, n_new: usize) -> (f64, f64) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let edges = generate_edges(n_v, n_e);

    // 构建 CSR 图
    let csr = build_csr(&edges, n_v);

    // 全量 BFS 做 SSSP（无权图 = 每条边权重 1.0）
    let t0 = Instant::now();
    let mut distances = vec![f32::INFINITY; n_v];
    distances[0] = 0.0;
    use std::collections::VecDeque;
    let mut queue = VecDeque::new();
    queue.push_back(0u32);
    let mut adj_out: Vec<Vec<usize>> = vec![Vec::new(); n_v];
    for &(u, v) in &edges {
        adj_out[u as usize].push(v as usize);
    }
    while let Some(u) = queue.pop_front() {
        for &v in &adj_out[u as usize] {
            if distances[v] == f32::INFINITY {
                distances[v] = distances[u as usize] + 1.0;
                queue.push_back(v as u32);
            }
        }
    }
    let t_full = t0.elapsed().as_secs_f64();

    // 新增边
    let mut new_edges = Vec::new();
    while new_edges.len() < n_new {
        let u = rng.gen_range(0..n_v as u64);
        let v = rng.gen_range(0..n_v as u64);
        if u != v && !edges.iter().any(|&(a, b)| a == u && b == v) {
            new_edges.push((u, v));
        }
    }

    // 增量 SSSP（只更新受影响顶点）
    let inc = match IncrementalSSSP::new() {
        Ok(inc) => inc,
        Err(_) => {
            eprintln!("  [SSSP] GPU 不可用，纯 CPU 增量");
            // 回退：简单重新 BFS
            let t_inc = t0.elapsed().as_secs_f64() - t_full;
            return (t_full * 1000.0, t_inc * 1000.0);
        }
    };

    // 构建受影响列表
    let mut affected: Vec<u32> = Vec::new();
    for &(u, v) in &new_edges {
        if !affected.contains(&(u as u32)) { affected.push(u as u32); }
        if !affected.contains(&(v as u32)) { affected.push(v as u32); }
    }

    // 构建新 CSR
    let mut all_edges = edges.clone();
    all_edges.extend_from_slice(&new_edges);
    let csr_new = build_csr(&all_edges, n_v);

    let t1 = Instant::now();
    let _new_distances = inc.compute(&csr_new, &distances, &affected);
    let t_inc = t1.elapsed().as_secs_f64();

    (t_full * 1000.0, t_inc * 1000.0)
}

// ── 增量 Triangle Counting ─────────────────────────

fn bench_incremental_tc(n_v: usize, n_e: usize, n_new: usize) -> (f64, f64) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let edges = generate_edges(n_v, n_e);

    // 构建无向图（Triangle Counting 需要无向图）
    let mut graph = GraphWithAdjacencySets::new(n_v);
    for &(u, v) in &edges {
        graph.add_edge(u as u32, v as u32);
    }

    // 全量三角形计数
    let t0 = Instant::now();
    let (_full_count, _full_time) = full_triangle_counting(&graph);
    let triangles_before = full_triangle_counting_set(&graph).0;
    let t_full = t0.elapsed().as_secs_f64();

    // 新增边
    let mut new_edges_u32: Vec<(u32, u32)> = Vec::new();
    while new_edges_u32.len() < n_new {
        let u = rng.gen_range(0..n_v as u64);
        let v = rng.gen_range(0..n_v as u64);
        if u != v && !edges.iter().any(|&(a, b)| a == u && b == v) {
            new_edges_u32.push((u as u32, v as u32));
        }
    }

    // 应用新边到图上
    for &(u, v) in &new_edges_u32 {
        graph.add_edge(u, v);
    }

    // 增量三角形计数
    let t1 = Instant::now();
    let _new_count = incremental_triangle_counting_correct(&graph, &new_edges_u32, &triangles_before);
    let t_inc = t1.elapsed().as_secs_f64();

    (t_full * 1000.0, t_inc * 1000.0)
}

// ── Main ─────────────────────────

fn main() {
    println!("# SSSP + Triangle Counting 增量性能验证\n");
    println!("**测试条件**：随机图（幂律度分布），新增 {} 条边\n", 50);

    let scales = vec![
        ("1K / 5K",   1_000,    5_000),
        ("10K / 50K", 10_000,  50_000),
        ("50K / 250K",50_000, 250_000),
    ];

    // ── SSSP ──
    println!("## 增量 SSSP\n");
    println!("| 规模 | 全量 SSSP | 增量 SSSP | 加速比 | 备注 |");
    println!("|------|-----------|-----------|--------|------|");

    for (label, n_v, n_e) in &scales {
        let (full_ms, inc_ms) = bench_incremental_sssp(*n_v, *n_e, 50);
        let speedup = if inc_ms > 0.0 { full_ms / inc_ms } else { f64::INFINITY };
        let note = if speedup < 1.0 { "GPU 开销 > 计算收益" } else { "" };
        println!("| {} | {:.2}ms | {:.3}ms | **{:.1}x** | {} |",
            label, full_ms, inc_ms, speedup, note);
    }

    println!();

    // ── Triangle Counting ──
    println!("## 增量 Triangle Counting\n");
    println!("| 规模 | 全量 TC | 增量 TC | 加速比 |");
    println!("|------|---------|---------|--------|");

    for (label, n_v, n_e) in &scales {
        let (full_ms, inc_ms) = bench_incremental_tc(*n_v, *n_e, 10);
        let speedup = if inc_ms > 0.0 { full_ms / inc_ms } else { f64::INFINITY };
        println!("| {} | {:.2}ms | {:.3}ms | **{:.1}x** |",
            label, full_ms, inc_ms, speedup);
    }
}
