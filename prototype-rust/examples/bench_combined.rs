// examples/bench_combined.rs
// 统一基准：增量算法 + 横向对比，边数按顶点比例缩放

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use axolotl_rs::graph_db::{GraphDB, GraphMode};
use axolotl_rs::PropertyValue;
use axolotl_rs::csr_graph::CSRGraph;
use axolotl_rs::pagerank_correct;
use axolotl_rs::incremental_ssspv2::incremental_sssp_diff_bfs;
use axolotl_rs::incremental_cc::IncrementalCC;

use rand::Rng;

fn load_edgelist(path: &str) -> (Vec<(u64, u64)>, usize) {
    let f = fs::File::open(path).expect("open");
    let mut edges = Vec::new();
    let mut max_v = 0u64;
    for line in BufReader::new(f).lines() {
        let line = line.unwrap();
        if line.is_empty() || line.starts_with('#') || line.starts_with('%') { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let (Ok(u), Ok(v)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                if u != v { edges.push((u, v)); if u > max_v { max_v = u; } if v > max_v { max_v = v; } }
            }
        }
    }
    (edges, (max_v + 1) as usize)
}

fn edges_to_add(n_v: usize) -> usize {
    (n_v as f64 * 0.001).max(10.0).round() as usize
}

fn gen_new(edges: &[(u64, u64)], n_v: usize, count: usize) -> Vec<(u64, u64)> {
    let mut rng = rand::thread_rng();
    let mut new = Vec::new();
    while new.len() < count {
        let u = rng.gen_range(0..n_v as u64);
        let v = rng.gen_range(0..n_v as u64);
        if u != v && !edges.iter().any(|&(a,b)| a==u&&b==v) {
            new.push((u, v));
        }
    }
    new
}

fn build_csr(edges: &[(u64, u64)], n_v: usize) -> CSRGraph {
    let mut csr = CSRGraph::new();
    for i in 0..n_v as u64 { csr.add_vertex(i, HashMap::new()); }
    csr.build_csr(edges);
    csr
}

fn full_bfs(edges: &[(u64, u64)], n_v: usize) -> (Vec<f32>, f64) {
    let t0 = Instant::now();
    let mut adj = vec![Vec::new(); n_v];
    for &(u, v) in edges { adj[u as usize].push(v as usize); }
    let mut dist = vec![f32::INFINITY; n_v];
    dist[0] = 0.0;
    let mut q = VecDeque::new();
    q.push_back(0u32);
    while let Some(u) = q.pop_front() {
        let du = dist[u as usize];
        for &v in &adj[u as usize] {
            if dist[v] == f32::INFINITY { dist[v] = du + 1.0; q.push_back(v as u32); }
        }
    }
    let elapsed = t0.elapsed().as_secs_f64();
    (dist, elapsed)
}

fn incr_bfs(edges: &[(u64, u64)], n_v: usize, new: &[(u32, u32)]) -> f64 {
    let (mut dist, _) = full_bfs(edges, n_v);
    let mut adj = vec![Vec::new(); n_v];
    for &(u, v) in edges { adj[u as usize].push(v as usize); }
    for &(u, v) in new { adj[u as usize].push(v as usize); }
    let t0 = Instant::now();
    let mut q = VecDeque::new();
    for &(u, v) in new {
        let uu = u as usize; let vv = v as usize;
        if uu < n_v && vv < n_v && dist[uu] + 1.0 < dist[vv] {
            dist[vv] = dist[uu] + 1.0; q.push_back(v);
        }
    }
    while let Some(w) = q.pop_front() {
        let dw = dist[w as usize];
        for &nb in &adj[w as usize] {
            if dw + 1.0 < dist[nb] { dist[nb] = dw + 1.0; q.push_back(nb as u32); }
        }
    }
    t0.elapsed().as_secs_f64()
}

fn format_num(n: usize) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 1_000 { format!("{}K", n / 1_000) }
    else { n.to_string() }
}

fn main() {
    let datasets: Vec<(&str, &str)> = vec![
        ("soc-Epinions1", "performance_test/datasets/soc_epinions1_100K.edgelist"),
        ("com-DBLP", "performance_test/datasets/com_dblp_100K.edgelist"),
        ("web-Google", "performance_test/datasets/web_google_100K.edgelist"),
        ("RMAT scale 20", "performance_test/datasets/rmat_1M_100K.edgelist"),
        ("com-DBLP (full)", "performance_test/datasets/com_dblp.edgelist"),
        ("web-Google (full)", "performance_test/datasets/web_google.edgelist"),
        ("RMAT scale 21", "performance_test/datasets/rmat21.edgelist"),
    ];

    println!("# Combined Benchmark: Incremental + Library Comparison\n");
    println!("**Rule**: +0.1% of vertices as new edges | **Hardware**: Apple M4\n");

    // ── Part A: Raw Throughput ──
    println!("## Raw Throughput\n");
    println!("| Dataset | V | E | +Edges | Build | BFS | PageRank |");
    println!("|---------|:---:|:---:|:---:|:-----:|:---:|:--------:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() { continue; }
        let (edges, n_v) = load_edgelist(path);
        let n_e = edges.len();

        let t0 = Instant::now();
        let mut db = GraphDB::new(GraphMode::InMemory);
        for i in 0..n_v as u64 { let _ = db.add_vertex(i, HashMap::new()); }
        for &(u, v) in &edges { let mut p = HashMap::new(); p.insert("w".into(), PropertyValue::Double(1.0)); let _ = db.add_edge(u, v, 1.0, p); }
        let t_build = t0.elapsed().as_secs_f64();

        let t0 = Instant::now();
        let _r = db.walk(0, n_v, |_, _, _| {});
        let t_bfs = t0.elapsed().as_secs_f64();

        let csr = build_csr(&edges, n_v);
        let t0 = Instant::now();
        let _pr = pagerank_correct::compute_pagerank_cpu(&csr, 100);
        let t_pr = t0.elapsed().as_secs_f64();

        println!("| {} | {} | {} | — | {:.2}s | {:.2}s | {}ms |",
            name, format_num(n_v), format_num(n_e),
            t_build, t_bfs, (t_pr * 1000.0) as usize);
    }

    // ── Part B: Incremental Algorithms ──
    println!("\n## Incremental Algorithm Speedup\n");
    println!("| Dataset | V | +E | BFS | CC | PageRank | SSSP |");
    println!("|---------|:---:|:---:|:---:|:---:|:--------:|:----:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() { continue; }
        let (edges, n_v) = load_edgelist(path);
        let add = edges_to_add(n_v);
        let e_label = format!("+{}", add);

        // BFS
        let (_, bfs_full) = full_bfs(&edges, n_v);
        let new_u32: Vec<(u32, u32)> = gen_new(&edges, n_v, add).iter().map(|&(u,v)| (u as u32, v as u32)).collect();
        let bfs_inc = incr_bfs(&edges, n_v, &new_u32);
        let bfs_x = if bfs_inc > 0.0 { bfs_full / bfs_inc } else { 0.0 };

        // CC
        let t0 = Instant::now();
        let mut uf = IncrementalCC::new(n_v);
        uf.compute_full(&edges);
        let cc_full = t0.elapsed().as_secs_f64();
        let new_cc = gen_new(&edges, n_v, add);
        let t0 = Instant::now();
        uf.update_incremental(&new_cc);
        let cc_inc = t0.elapsed().as_secs_f64();
        let cc_x = if cc_inc > 0.0 { cc_full / cc_inc } else { 0.0 };

        // PageRank
        let csr = build_csr(&edges, n_v);
        let t0 = Instant::now();
        let _pr = pagerank_correct::compute_pagerank_cpu(&csr, 100);
        let pr_full = t0.elapsed().as_secs_f64();
        let pr_new = gen_new(&edges, n_v, add);
        let mut all = edges.clone(); all.extend_from_slice(&pr_new);
        let csr2 = build_csr(&all, n_v);
        let t0 = Instant::now();
        let _pr2 = pagerank_correct::compute_pagerank_cpu(&csr2, 10);
        let pr_inc = t0.elapsed().as_secs_f64();
        let pr_x = if pr_inc > 0.0 { pr_full / pr_inc } else { 0.0 };

        // SSSP
        let (dist, sssp_full) = full_bfs(&edges, n_v);
        let sssp_new: Vec<(u32, u32)> = gen_new(&edges, n_v, add).iter().map(|&(u,v)| (u as u32, v as u32)).collect();
        let mut sssp_all = edges.clone();
        for &(u, v) in &sssp_new { sssp_all.push((u as u64, v as u64)); }
        let csr3 = build_csr(&sssp_all, n_v);
        let t0 = Instant::now();
        let mut d = dist.clone();
        let _ = incremental_sssp_diff_bfs(&csr3, &mut d, &sssp_new);
        let sssp_inc = t0.elapsed().as_secs_f64();
        let sssp_x = if sssp_inc > 0.0 { sssp_full / sssp_inc } else { 0.0 };

        println!("| {} | {} | {} | {:.0}x | {:.0}x | {:.1}x | {:.0}x |",
            name, format_num(n_v), e_label,
            bfs_x, cc_x, pr_x, sssp_x);
    }

    // ── Part C: Library Comparison (EdgeBlock vs petgraph) ──
    println!("\n## Library Comparison (EdgeBlock vs petgraph)\n");
    println!("| Dataset | V | Library | Build | BFS | PageRank |");
    println!("|---------|:---:|---------|:-----:|:---:|:--------:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() { continue; }
        let (edges, n_v) = load_edgelist(path);

        // EdgeBlock
        let t0 = Instant::now();
        let mut db = GraphDB::new(GraphMode::InMemory);
        for i in 0..n_v as u64 { let _ = db.add_vertex(i, HashMap::new()); }
        for &(u, v) in &edges { let mut p = HashMap::new(); p.insert("w".into(), PropertyValue::Double(1.0)); let _ = db.add_edge(u, v, 1.0, p); }
        let t_build_eb = t0.elapsed().as_secs_f64() * 1000.0;
        let t0 = Instant::now(); let _ = db.walk(0, n_v, |_, _, _| {}); let t_bfs_eb = t0.elapsed().as_secs_f64() * 1000.0;
        let csr = build_csr(&edges, n_v);
        let t0 = Instant::now(); let _ = pagerank_correct::compute_pagerank_cpu(&csr, 100); let t_pr_eb = t0.elapsed().as_secs_f64() * 1000.0;

        println!("| {} | {} | EdgeBlock | {:.0}ms | {:.1}ms | {:.0}ms |",
            name, format_num(n_v), t_build_eb, t_bfs_eb, t_pr_eb);

        // petgraph
        let t0 = Instant::now();
        let mut pg = petgraph::graph::DiGraph::<usize, ()>::new();
        let mut nodes = Vec::with_capacity(n_v);
        for i in 0..n_v { nodes.push(pg.add_node(i)); }
        for &(u, v) in &edges { pg.add_edge(nodes[u as usize], nodes[v as usize], ()); }
        let t_build_pg = t0.elapsed().as_secs_f64() * 1000.0;

        let t0 = Instant::now();
        let mut q = VecDeque::new(); let mut vis = vec![false; n_v];
        q.push_back(0); vis[0] = true;
        while let Some(u_idx) = q.pop_front() {
            for v_node in pg.neighbors_directed(nodes[u_idx], petgraph::Direction::Outgoing) {
                let v = pg[v_node]; if !vis[v] { vis[v] = true; q.push_back(v); }
            }
        }
        let t_bfs_pg = t0.elapsed().as_secs_f64() * 1000.0;

        let n = n_v; let d = 0.85;
        let t0 = Instant::now();
        let mut pr = vec![1.0 / n as f64; n]; let mut next = vec![0.0; n];
        for _ in 0..100 {
            for v in 0..n { next[v] = (1.0 - d) / n as f64; }
            for u in 0..n {
                let out_d = pg.neighbors_directed(nodes[u], petgraph::Direction::Outgoing).count();
                if out_d == 0 { continue; }
                let share = d * pr[u] / out_d as f64;
                for v_node in pg.neighbors_directed(nodes[u], petgraph::Direction::Outgoing) {
                    next[pg[v_node]] += share;
                }
            }
            std::mem::swap(&mut pr, &mut next);
        }
        let t_pr_pg = t0.elapsed().as_secs_f64() * 1000.0;

        println!("| | | petgraph | {:.0}ms | {:.1}ms | {:.0}ms |",
            t_build_pg, t_bfs_pg, t_pr_pg);
    }
}
