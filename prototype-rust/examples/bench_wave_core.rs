/// examples/bench_wave_core.rs — Wave Core vs petgraph K-Core (BZ)
///
/// petgraph 没有内置 K-Core，在 benchmark 中实现标准 Batagelj-Zaversnik 算法进行公平对比。

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;
use axolotl_rs::wave_core::wave_core_blocks;
use axolotl_rs::PropertyValue;

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

/// Standard Batagelj-Zaversnik K-Core on petgraph
fn petgraph_k_core(edges: &[(u64, u64)], n_v: usize) -> Vec<u32> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n_v];
    for &(u, v) in edges {
        let ui = u as usize;
        let vi = v as usize;
        if ui < n_v && vi < n_v && ui != vi {
            adj[ui].push(vi);
            adj[vi].push(ui); // undirected
        }
    }

    let mut degree: Vec<u32> = adj.iter().map(|nbrs| nbrs.len() as u32).collect();
    let max_deg = *degree.iter().max().unwrap_or(&0) as usize;

    // Bin sort
    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); max_deg + 1];
    for v in 0..n_v {
        bins[degree[v] as usize].push(v);
    }

    let mut core = vec![0u32; n_v];
    let mut peeled = vec![false; n_v];
    let mut peeled_count = 0;

    while peeled_count < n_v {
        let mut found = false;
        for current_bin in 0..=max_deg {
            if bins[current_bin].is_empty() { continue; }

            let batch = std::mem::take(&mut bins[current_bin]);
            for &v in &batch {
                if peeled[v] { continue; }
                core[v] = current_bin as u32;
                peeled[v] = true;
                peeled_count += 1;

                for &nb in &adj[v] {
                    if peeled[nb] { continue; }
                    if degree[nb] as usize <= current_bin { continue; }
                    degree[nb] -= 1;
                    bins[degree[nb] as usize].push(nb);
                }
            }
            found = true;
            break;
        }
        if !found { break; }
    }

    let remaining = max_deg as u32;
    for v in 0..n_v {
        if !peeled[v] { core[v] = remaining; }
    }
    core
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

    println!("# Wave Core vs petgraph K-Core (BZ)\n");
    println!("**Undirected** (bidirectional for Wave Core, mirrored for petgraph)\n");
    println!("| Dataset | V | E | Wave Core (EdgeBlock) | petgraph K-Core (BZ) | Wave Core Advantage |");
    println!("|---------|:---:|:---:|:---:|:---:|:---:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() { continue; }
        let (edges, n_v) = load_edgelist(path);
        let n_e = edges.len();

        // Wave Core on EdgeBlock
        let mut eb = GPUEdgeBlockGraph::new();
        for i in 0..n_v as u64 { eb.add_vertex(i, HashMap::new()); }
        for &(u, v) in &edges {
            eb.add_edge(u, v, 1.0);
            eb.add_edge(v, u, 1.0); // undirected
        }
        let t0 = Instant::now();
        let _wc = wave_core_blocks(&eb);
        let t_wc = t0.elapsed().as_secs_f64() * 1000.0;

        // petgraph K-Core
        let t0 = Instant::now();
        let _pk = petgraph_k_core(&edges, n_v);
        let t_pg = t0.elapsed().as_secs_f64() * 1000.0;

        let ratio = if t_pg > 0.0 { t_pg / t_wc } else { 0.0 };

        print!("| {} | {} | {} | {:.0}ms | {:.0}ms | ", name, format_num(n_v), format_num(n_e), t_wc, t_pg);
        if ratio >= 1.0 {
            println!("{:.1}x |", ratio);
        } else {
            println!("< 1x ({:.1}x slower) |", 1.0 / ratio);
        }
    }
}
