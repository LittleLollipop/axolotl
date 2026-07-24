/// examples/bench_betweenness.rs — Sampled Betweenness: EdgeBlock vs adjacency list

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;
use axolotl_rs::betweenness::brandes_betweenness;
use rand::Rng;

const SAMPLES: usize = 32;

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

fn format_num(n: usize) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 1_000 { format!("{}K", n / 1_000) }
    else { n.to_string() }
}

fn adj_brandes(adj: &[Vec<usize>], n: usize, sources: &[usize]) -> Vec<f64> {
    let mut bc = vec![0.0f64; n];
    let mut dist = vec![u32::MAX; n];
    let mut sigma = vec![0u64; n];
    let mut delta = vec![0.0f64; n];
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut q = std::collections::VecDeque::new();
    let mut order = Vec::new();
    for &s in sources {
        q.clear(); order.clear();
        dist[s] = 0; sigma[s] = 1; q.push_back(s);
        while let Some(v) = q.pop_front() {
            order.push(v);
            for &w in &adj[v] {
                if dist[w] == u32::MAX {
                    dist[w] = dist[v] + 1; sigma[w] = sigma[v]; pred[w].push(v); q.push_back(w);
                } else if dist[w] == dist[v] + 1 { sigma[w] += sigma[v]; pred[w].push(v); }
            }
        }
        for &w in order.iter().rev() {
            for &p in &pred[w] { if sigma[p] > 0 { delta[p] += (sigma[p] as f64 / sigma[w] as f64) * (1.0 + delta[w]); } }
            if w != s { bc[w] += delta[w]; }
        }
        for &v in &order { dist[v] = u32::MAX; sigma[v] = 0; delta[v] = 0.0; pred[v].clear(); }
    }
    bc
}

fn main() {
    let datasets: Vec<(&str, &str)> = vec![
        ("soc-Epinions1", "performance_test/datasets/soc_epinions1_100K.edgelist"),
        ("com-DBLP", "performance_test/datasets/com_dblp_100K.edgelist"),
        ("web-Google", "performance_test/datasets/web_google_100K.edgelist"),
        ("com-DBLP (full)", "performance_test/datasets/com_dblp.edgelist"),
    ];

    println!("# Sampled Betweenness Centrality ({} sources)\n", SAMPLES);
    println!("| Dataset | V | E | axolotl-rs (EdgeBlock) | Adjacency list | Ratio |");
    println!("|---------|:---:|:---:|:---:|:---:|:---:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() { continue; }
        let (edges, n_v) = load_edgelist(path);

        // Shared random sources
        let sources: Vec<usize> = {
            let mut rng = rand::thread_rng();
            let mut v: Vec<usize> = (0..n_v).collect();
            for i in 0..SAMPLES.min(n_v) {
                let j = rng.gen_range(i..n_v);
                v.swap(i, j);
            }
            v[..SAMPLES.min(n_v)].to_vec()
        };

        // axolotl-rs: EdgeBlock BFS
        let mut eb = GPUEdgeBlockGraph::new();
        for i in 0..n_v as u64 { eb.add_vertex(i, HashMap::new()); }
        for &(u, v) in &edges { eb.add_edge(u, v, 1.0); }
        let t0 = Instant::now();
        let _ = brandes_betweenness(&eb, Some(SAMPLES));
        let t_ax = t0.elapsed().as_secs_f64() * 1000.0;

        // Adjacency list Brandes
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n_v];
        for &(u, v) in &edges {
            if u < n_v as u64 && v < n_v as u64 { adj[u as usize].push(v as usize); }
        }
        let t0 = Instant::now();
        let _ = adj_brandes(&adj, n_v, &sources);
        let t_adj = t0.elapsed().as_secs_f64() * 1000.0;

        let ratio = if t_adj > 0.0 { t_adj / t_ax } else { 0.0 };
        let tag = if ratio >= 1.0 { format!("{:.1}x faster", ratio) } else { format!("{:.1}x slower", 1.0/ratio) };

        println!("| {} | {} | {} | {:.0}ms | {:.0}ms | {} |",
            name, format_num(n_v), format_num(edges.len()), t_ax, t_adj, tag);
    }
}
