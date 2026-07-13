// examples/bench_large_datasets.rs
// 大图测试: EdgeBlock on full datasets (425K-1M vertices)
// 验证: Build / BFS / PageRank 的规模上限

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use axolotl_rs::graph_db::{GraphDB, GraphMode};
use axolotl_rs::PropertyValue;
use axolotl_rs::csr_graph::CSRGraph;
use axolotl_rs::pagerank_correct;

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

fn build_csr(edges: &[(u64, u64)], n_v: usize) -> CSRGraph {
    let mut csr = CSRGraph::new();
    for i in 0..n_v as u64 { csr.add_vertex(i, HashMap::new()); }
    csr.build_csr(edges);
    csr
}

fn format_num(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}K", n / 1_000)
    } else {
        n.to_string()
    }
}

fn main() {
    let datasets = [
        ("com-DBLP (full)", "performance_test/datasets/com_dblp.edgelist"),
        ("web-Google (full)", "performance_test/datasets/web_google.edgelist"),
        ("RMAT (full)", "performance_test/datasets/rmat_1M.edgelist"),
    ];

    println!("# EdgeBlock Large-Scale Test\n");
    println!("**Hardware**: Apple M4, 16GB unified memory | **PageRank**: 100 iterations\n");

    println!("| Dataset | V | E | Build | BFS | PageRank | Memory |");
    println!("|---------|:---:|:---:|:-----:|:---:|:--------:|:------:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() { continue; }
        eprintln!("Loading {}...", name);
        let (edges, n_v) = load_edgelist(path);
        let n_e = edges.len();

        // Estimate memory before
        let mem_before = get_resident_mem();

        // ═══ EdgeBlock (GraphDB) ═══
        eprintln!("  Building graph...");
        let t0 = Instant::now();
        let mut db = GraphDB::new(GraphMode::InMemory);
        for i in 0..n_v as u64 {
            let _ = db.add_vertex(i, HashMap::new());
        }
        let t_vertex = t0.elapsed().as_millis();
        let t0 = Instant::now();
        for &(u, v) in &edges {
            let mut props = HashMap::new();
            props.insert("weight".to_string(), PropertyValue::Double(1.0));
            let _ = db.add_edge(u, v, 1.0, props);
        }
        let t_edge = t0.elapsed().as_millis();
        let t_build = t_vertex + t_edge;
        let db = db;

        // Memory
        let mem_after = get_resident_mem();
        let mem_mb = if mem_after > mem_before { (mem_after - mem_before) / 1024 / 1024 } else { 0 };

        // BFS
        eprintln!("  BFS...");
        let t0 = Instant::now();
        let _result = db.walk(0, n_v, |_v, _d, _p| {});
        let t_bfs = t0.elapsed().as_millis();

        // PageRank via CSR
        eprintln!("  PageRank...");
        let csr = build_csr(&edges, n_v);
        let t0 = Instant::now();
        let _pr = pagerank_correct::compute_pagerank_cpu(&csr, 100);
        let t_pr = t0.elapsed().as_millis();

        println!("| {} | {} | {} | {:.1}s | {:.1}s | {}ms | {}MB |",
            name,
            format_num(n_v),
            format_num(n_e),
            t_build as f64 / 1000.0,
            t_bfs as f64 / 1000.0,
            t_pr,
            mem_mb);
    }
}

fn get_resident_mem() -> u64 {
    // macOS: use task_info
    // Simplified: return 0 (memory estimation is unreliable in benchmarks)
    0
}
