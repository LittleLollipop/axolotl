// examples/bench_edgeblock.rs
// 性能对比测试：EdgeBlock 统一内存架构 vs 旧 HashMap→CSR 架构

use std::collections::HashMap;
use std::time::Instant;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     EdgeBlock 统一内存架构 性能对比测试                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let scales = vec![
        ("1K顶点 / 5K边",     1_000,    5_000),
        ("10K顶点 / 50K边",   10_000,   50_000),
        ("50K顶点 / 200K边",  50_000,  200_000),
    ];

    for (label, n_v, n_e) in &scales {
        println!("━━━ {} ━━━", label);
        bench_scale(*n_v, *n_e);
        println!();
    }

    println!("━━━ 100K顶点 / 500K边 ━━━");
    bench_scale(100_000, 500_000);
}

fn bench_scale(n_vertices: usize, n_edges: usize) {
    use rand::Rng;
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();

    let mut edges: Vec<(u64, u64)> = Vec::with_capacity(n_edges);
    let mut all_vertices: Vec<u64> = (0..n_vertices as u64).collect();
    all_vertices.shuffle(&mut rng);

    while edges.len() < n_edges {
        let u = all_vertices[rng.gen_range(0..n_vertices)];
        let v = all_vertices[rng.gen_range(0..n_vertices)];
        if u != v {
            edges.push((u, v));
        }
    }

    // ═══════════════════════════════════════════════════
    // 架构 A: PersistentGraph (HashMap)
    // ═══════════════════════════════════════════════════
    println!("  ┌─ 架构 A: PersistentGraph (HashMap) ─────────┐");

    let t0 = Instant::now();
    let mut pg = axolotl_rs::persistence::PersistentGraph {
        vertices: HashMap::new(),
        edges: HashMap::new(),
        file_path: String::new(),
        index_manager: axolotl_rs::index::IndexManager::new(),
    };
    for v in 0..n_vertices as u64 {
        pg.add_vertex(v, HashMap::new());
    }
    for &(u, v) in &edges {
        pg.add_edge(u, v, 1.0, HashMap::new());
    }
    let t_build_a = t0.elapsed();
    println!("  │  构建 {}/{}:  {:>10.2?}",
        n_vertices, n_edges, t_build_a);

    let t0 = Instant::now();
    let csr_a = pg.to_csr();
    let t_csr = t0.elapsed();
    println!("  │  to_csr():   {:>10.2?}", t_csr);

    // GPU PageRank (macOS only)
    #[cfg(target_os = "macos")]
    {
        let vc = csr_a.vertex_count as usize;
        let mut out_degrees = vec![0u32; vc];
        for v in 0..vc {
            let s = csr_a.offsets[v] as usize;
            let e = csr_a.offsets[v + 1] as usize;
            out_degrees[v] = (e - s) as u32;
        }
        let pr = vec![1.0 / vc as f32; vc];

        match axolotl_rs::gpu::GPUAccelerator::new() {
            Ok(gpu) => {
                let t0 = Instant::now();
                let r = gpu.compute_full_pagerank(
                    &csr_a.reverse_offsets,
                    &csr_a.reverse_targets,
                    &out_degrees, &pr,
                    vc as u32, 0.85,
                );
                let t_gpu = t0.elapsed();
                let s: f32 = r.iter().sum();
                println!("  │  GPU PageRank: {:>10.2?}  sum={:.4}", t_gpu, s);
            }
            Err(e) => println!("  │  GPU: {}", e),
        }
    }

    // HashMap out_neighbors scan (O(V*E))
    let t0 = Instant::now();
    let mut cnt = 0usize;
    for v in 0..n_vertices as u64 {
        cnt += pg.edges.keys().filter(|(f, _)| *f == v).count();
    }
    let t_scan_a = t0.elapsed();
    println!("  │  neighbors scan: {:>10.2?}", t_scan_a);
    println!("  └──────────────────────────────────────────────────┘");

    // ═══════════════════════════════════════════════════
    // 架构 B: GPUEdgeBlockGraph (Flat Array)
    // ═══════════════════════════════════════════════════
    println!("  ┌─ 架构 B: GPUEdgeBlockGraph (Flat Array) ──────┐");

    let t0 = Instant::now();
    let mut eb = axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph::new();
    for v in 0..n_vertices as u64 {
        eb.add_vertex(v, HashMap::new());
    }
    for &(u, v) in &edges {
        eb.add_edge(u, v, 1.0);
    }
    let t_build_b = t0.elapsed();
    println!("  │  构建 {}/{}:  {:>10.2?}",
        n_vertices, n_edges, t_build_b);

    // EdgeBlock out_neighbors (O(V+E))
    let t0 = Instant::now();
    let mut cnt_b = 0usize;
    for v in 0..eb.vertex_count as usize {
        cnt_b += eb.out_neighbors_by_idx(v).len();
    }
    let t_scan_b = t0.elapsed();
    println!("  │  neighbors:   {:>10.2?}", t_scan_b);

    // GPU PageRank (EdgeBlock native + cached)
    #[cfg(target_os = "macos")]
    {
        let out_degrees = eb.compute_out_degrees();
        let vc = eb.vertex_count as usize;
        let pr = vec![1.0 / vc as f32; vc];
        let affected: Vec<u32> = (0..vc as u32).collect();

        match axolotl_rs::gpu::GPUAccelerator::new() {
            Ok(gpu) => {
                // 冷启动（无缓存，首次分配 buffer）
                let t0 = Instant::now();
                let r = gpu.compute_incremental_pagerank_edgeblock(
                    &eb, &pr, &affected, &out_degrees, 0.85,
                );
                let t_cold = t0.elapsed();
                println!("  │  GPU PR(冷): {:>10.2?}  sum={:.4}",
                    t_cold, r.iter().sum::<f32>());

                // 热启动（缓存复用）
                let mut cache = axolotl_rs::gpu::EdgeBlockCache::new();
                let t0 = Instant::now();
                let _ = gpu.compute_incremental_pagerank_edgeblock_cached(
                    &eb, &pr, &affected, &out_degrees, 0.85,
                    Some(&mut cache),
                );
                let t_hot = t0.elapsed();

                let t0 = Instant::now();
                let _ = gpu.compute_incremental_pagerank_edgeblock_cached(
                    &eb, &pr, &affected, &out_degrees, 0.85,
                    Some(&mut cache),
                );
                let t_cached = t0.elapsed();
                println!("  │  GPU PR(热): {:>10.2?}", t_hot);
                println!("  │  GPU PR(缓存): {:>10.2?}", t_cached);
            }
            Err(e) => println!("  │  GPU: {}", e),
        }
    }

    // ── 对比 ─────────────────────────
    let sp_build = t_build_a.as_secs_f64() / t_build_b.as_secs_f64().max(1e-9);
    let sp_scan = t_scan_a.as_secs_f64() / t_scan_b.as_secs_f64().max(1e-9);
    println!("  └──────────────────────────────────────────────────┘");
    println!("  ▶ 构建加速: {:.1}x", sp_build);
    println!("  ▶ 邻居扫描加速: {:.1}x", sp_scan);

    #[cfg(target_os = "macos")]
    {
        let old_total = t_build_a.as_secs_f64() + t_csr.as_secs_f64();
        println!("  ▶ 旧架构总成本(构建+CSR): {:.2?}", Duration::from_secs_f64(old_total));
        println!("  ▶ 新架构总成本(构建):     {:.2?}", t_build_b);
        println!("  ▶ 节省的 CSR 转换:      {:.2?}", t_csr);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let old_total = t_build_a.as_secs_f64() + t_csr.as_secs_f64();
        println!("  ▶ 旧架构总成本(构建+CSR): {:.2?}", Duration::from_secs_f64(old_total));
        println!("  ▶ 新架构总成本(构建):     {:.2?}", t_build_b);
        println!("  ▶ 节省的 CSR 转换:      {:.2?}", t_csr);
    }
}

use std::time::Duration;
