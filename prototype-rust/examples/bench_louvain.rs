/// examples/bench_louvain.rs — Louvain (EdgeBlock) benchmark

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;
use axolotl_rs::louvain::louvain_communities;

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

fn main() {
    let datasets: Vec<(&str, &str)> = vec![
        ("soc-Epinions1", "performance_test/datasets/soc_epinions1_100K.edgelist"),
        ("com-DBLP", "performance_test/datasets/com_dblp_100K.edgelist"),
        ("web-Google", "performance_test/datasets/web_google_100K.edgelist"),
        ("RMAT scale 20", "performance_test/datasets/rmat_1M_100K.edgelist"),
    ];

    println!("# Louvain Community Detection (axolotl-rs EdgeBlock)\n");
    println!("| Dataset | V | E | Time | Communities | Passes |");
    println!("|---------|:---:|:---:|:---:|:---:|:---:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() { continue; }
        let (edges, n_v) = load_edgelist(path);

        let mut eb = GPUEdgeBlockGraph::new();
        for i in 0..n_v as u64 { eb.add_vertex(i, HashMap::new()); }
        for &(u, v) in &edges { eb.add_edge(u, v, 1.0); eb.add_edge(v, u, 1.0); }

        let t0 = Instant::now();
        let (result, passes) = louvain_communities(&eb);
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        let n_comms = result.iter().max().map(|&x| x + 1).unwrap_or(0);

        println!("| {} | {} | {} | {:.0}ms | {} | {} |",
            name, format_num(n_v), format_num(edges.len()), elapsed, n_comms, passes);
    }
}
