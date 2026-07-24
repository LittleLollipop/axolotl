/// examples/bench_scc.rs — Tarjan SCC (EdgeBlock) vs petgraph kosaraju_scc

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;
use axolotl_rs::scc::tarjan_scc;
use petgraph::algo::kosaraju_scc;

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
        ("com-DBLP (full)", "performance_test/datasets/com_dblp.edgelist"),
        ("web-Google (full)", "performance_test/datasets/web_google.edgelist"),
        ("soc-LiveJournal1", "performance_test/datasets/soc_livejournal1.edgelist"),
        ("RMAT scale 21", "performance_test/datasets/rmat21.edgelist"),
    ];

    println!("# Tarjan SCC (EdgeBlock) vs petgraph kosaraju_scc\n");
    println!("**Directed** graphs (single direction edges)\n");
    println!("| Dataset | V | E | Tarjan (EdgeBlock) | petgraph Kosaraju | Tarjan Advantage | SCCs |");
    println!("|---------|:---:|:---:|:---:|:---:|:---:|:---:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() { continue; }
        let (edges, n_v) = load_edgelist(path);

        // axolotl-rs Tarjan
        let mut eb = GPUEdgeBlockGraph::new();
        for i in 0..n_v as u64 { eb.add_vertex(i, HashMap::new()); }
        for &(u, v) in &edges { eb.add_edge(u, v, 1.0); }
        let t0 = Instant::now();
        let sccs = tarjan_scc(&eb);
        let t_ax = t0.elapsed().as_secs_f64() * 1000.0;
        let scc_count = sccs.len();

        // petgraph Kosaraju
        let mut g = petgraph::graph::DiGraph::new();
        let mut nodes: Vec<petgraph::graph::NodeIndex> = Vec::with_capacity(n_v);
        for i in 0..n_v { nodes.push(g.add_node(())); }
        for &(u, v) in &edges {
            let ui = u as usize; let vi = v as usize;
            if ui < n_v && vi < n_v { g.add_edge(nodes[ui], nodes[vi], ()); }
        }
        let t0 = Instant::now();
        let _ = kosaraju_scc(&g);
        let t_pg = t0.elapsed().as_secs_f64() * 1000.0;

        let ratio = if t_ax > 0.0 { t_pg / t_ax } else { 0.0 };
        let tag = if ratio >= 1.0 {
            format!("{:.1}x faster", ratio)
        } else {
            format!("{:.1}x slower", 1.0 / ratio)
        };

        println!("| {} | {} | {} | {:.0}ms | {:.1}ms | {} | {} |",
            name, format_num(n_v), format_num(edges.len()), t_ax, t_pg, tag, scc_count);
    }
}
