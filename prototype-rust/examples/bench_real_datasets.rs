// examples/bench_real_datasets.rs
// Standard graph dataset benchmarks for EdgeBlock
// Uses CPU-only incremental algorithms for fair comparison across datasets

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use axolotl_rs::csr_graph::CSRGraph;
use axolotl_rs::incremental_ssspv2::incremental_sssp_diff_bfs;
use axolotl_rs::pagerank_correct;
use axolotl_rs::incremental_cc::IncrementalCC;

fn load_edgelist(path: &str) -> (Vec<(u64, u64)>, usize) {
    let f = fs::File::open(path).expect("open");
    let mut edges = Vec::new();
    let mut max_v = 0u64;
    for line in BufReader::new(f).lines() {
        let line = line.unwrap();
        if line.is_empty() || line.starts_with('#') || line.starts_with('%') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let (Ok(u), Ok(v)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                if u != v {
                    edges.push((u, v));
                    if u > max_v { max_v = u; }
                    if v > max_v { max_v = v; }
                }
            }
        }
    }
    (edges, (max_v + 1) as usize)
}

fn build_csr(edges: &[(u64, u64)], n_v: usize) -> CSRGraph {
    let mut csr = CSRGraph::new();
    for i in 0..n_v as u64 {
        csr.add_vertex(i, HashMap::new());
    }
    csr.build_csr(edges);
    csr
}

// ── Full BFS ──

fn full_bfs(edges: &[(u64, u64)], n_v: usize) -> (Vec<f32>, f64) {
    let t0 = Instant::now();
    let mut adj = vec![Vec::new(); n_v];
    for &(u, v) in edges {
        adj[u as usize].push(v as usize);
    }
    let mut dist = vec![f32::INFINITY; n_v];
    dist[0] = 0.0;
    let mut q = VecDeque::new();
    q.push_back(0u32);
    while let Some(u) = q.pop_front() {
        let du = dist[u as usize];
        for &v in &adj[u as usize] {
            if dist[v] == f32::INFINITY {
                dist[v] = du + 1.0;
                q.push_back(v as u32);
            }
        }
    }
    let elapsed = t0.elapsed().as_secs_f64();
    (dist, elapsed)
}

// ── Incremental BFS (CPU: differential approach) ──

fn incr_bfs(edges: &[(u64, u64)], n_v: usize, new_edges: &[(u32, u32)]) -> (Vec<f32>, f64) {
    let (mut dist, _) = full_bfs(edges, n_v);

    let t0 = Instant::now();
    let mut q = VecDeque::new();
    for &(u, v) in new_edges {
        let uu = u as usize;
        let vv = v as usize;
        if uu < n_v && vv < n_v && dist[uu] + 1.0 < dist[vv] {
            dist[vv] = dist[uu] + 1.0;
            q.push_back(v);
        }
    }
    let mut adj = vec![Vec::new(); n_v];
    for &(u, v) in edges {
        adj[u as usize].push(v as usize);
    }
    for &(u, v) in new_edges {
        adj[u as usize].push(v as usize);
    }
    while let Some(w) = q.pop_front() {
        let dw = dist[w as usize];
        for &nb in &adj[w as usize] {
            if dw + 1.0 < dist[nb] {
                dist[nb] = dw + 1.0;
                q.push_back(nb as u32);
            }
        }
    }
    let elapsed = t0.elapsed().as_secs_f64();
    (dist, elapsed)
}

// ── Incremental CC (CPU: union-find) ──

fn full_cc(edges: &[(u64, u64)], n_v: usize) -> f64 {
    let t0 = Instant::now();
    let mut uf = IncrementalCC::new(n_v);
    uf.compute_full(edges);
    t0.elapsed().as_secs_f64()
}

fn incr_cc(edges: &[(u64, u64)], n_v: usize, new_edges: &[(u64, u64)]) -> f64 {
    let mut uf = IncrementalCC::new(n_v);
    uf.compute_full(edges);
    let t0 = Instant::now();
    uf.update_incremental(new_edges);
    t0.elapsed().as_secs_f64()
}

fn full_cc_time(edges: &[(u64, u64)], n_v: usize) -> f64 {
    let t0 = Instant::now();
    let mut uf = IncrementalCC::new(n_v);
    uf.compute_full(edges);
    t0.elapsed().as_secs_f64()
}

// ── Generate random new edges ──

fn gen_new_edges(edges: &[(u64, u64)], n_v: usize, count: usize) -> Vec<(u64, u64)> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut new = Vec::new();
    while new.len() < count {
        let u = rng.gen_range(0..n_v as u64);
        let v = rng.gen_range(0..n_v as u64);
        if u != v && !edges.iter().any(|&(a,b)| a==u&&b==v) && !new.iter().any(|&(a,b)| a==u&&b==v) {
            new.push((u, v));
        }
    }
    new
}

fn gen_new_edges_u32(edges: &[(u64, u64)], n_v: usize, count: usize) -> Vec<(u32, u32)> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut new = Vec::new();
    while new.len() < count {
        let u = rng.gen_range(0..n_v as u64);
        let v = rng.gen_range(0..n_v as u64);
        let u32 = u as u32;
        let v32 = v as u32;
        if u != v && !edges.iter().any(|&(a,b)| a==u && b==v) && !new.iter().any(|&(a,b)| a==u32 && b==v32) {
            new.push((u32, v32));
        }
    }
    new
}

fn main() {
    let datasets = [
        ("soc-Epinions1", "performance_test/datasets/soc_epinions1_100K.edgelist"),
        ("com-DBLP", "performance_test/datasets/com_dblp_100K.edgelist"),
        ("web-Google", "performance_test/datasets/web_google_100K.edgelist"),
        ("RMAT scale 20", "performance_test/datasets/rmat_1M_100K.edgelist"),
    ];

    println!("# Incremental Algorithm Performance on Standard Graphs\n");
    println!("**Setup**: +50 random edges, CPU incremental algorithms, M4 10-core GPU\n\n");

    // Header
    println!("| Dataset | V | E | BFS | CC | PageRank | SSSP |");
    println!("|---------|:---:|:---:|:---:|:---:|:--------:|:----:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() {
            println!("| {} | — | — | — | — | — | — |", name);
            continue;
        }

        let (edges, n_v) = load_edgelist(path);
        let n_e = edges.len();

        // ── BFS ──
        let (_, bfs_full_s) = full_bfs(&edges, n_v);
        let bfs_new = gen_new_edges_u32(&edges, n_v, 50);
        let (_, bfs_inc_s) = incr_bfs(&edges, n_v, &bfs_new);
        let bfs_speedup = if bfs_inc_s > 0.0 { bfs_full_s / bfs_inc_s } else { 0.0 };

        // ── CC ──
        let cc_full_s = full_cc_time(&edges, n_v);
        let cc_new = gen_new_edges(&edges, n_v, 50);
        let cc_inc_s = incr_cc(&edges, n_v, &cc_new);
        let cc_speedup = if cc_inc_s > 0.0 { cc_full_s / cc_inc_s } else { 0.0 };

        // ── PageRank ──
        let csr = build_csr(&edges, n_v);
        let t0 = Instant::now();
        let _ = pagerank_correct::compute_pagerank_cpu(&csr, 100);
        let pr_full_s = t0.elapsed().as_secs_f64();

        let pr_new = gen_new_edges(&edges, n_v, 50);
        let mut all_edges = edges.clone();
        all_edges.extend_from_slice(&pr_new);
        let csr_new = build_csr(&all_edges, n_v);
        let t1 = Instant::now();
        let _ = pagerank_correct::compute_pagerank_cpu(&csr_new, 10);
        let pr_inc_s = t1.elapsed().as_secs_f64();
        let pr_speedup = if pr_inc_s > 0.0 { pr_full_s / pr_inc_s } else { 0.0 };

        // ── SSSP ──
        let (sssp_dist, sssp_full_s) = full_bfs(&edges, n_v);
        let sssp_new = gen_new_edges_u32(&edges, n_v, 50);
        let mut sssp_all = edges.clone();
        for &(u, v) in &sssp_new { sssp_all.push((u as u64, v as u64)); }
        let csr_sssp = build_csr(&sssp_all, n_v);
        let t2 = Instant::now();
        let mut d = sssp_dist.clone();
        let _ = incremental_sssp_diff_bfs(&csr_sssp, &mut d, &sssp_new);
        let sssp_inc_s = t2.elapsed().as_secs_f64();
        let sssp_speedup = if sssp_inc_s > 0.0 { sssp_full_s / sssp_inc_s } else { 0.0 };

        println!("| {} | {:.0}K | {:.0}K | {:.0}x | {:.0}x | {:.1}x | {:.0}x |",
            name,
            n_v as f64 / 1000.0,
            n_e as f64 / 1000.0,
            bfs_speedup, cc_speedup, pr_speedup, sssp_speedup);
    }
}
