// examples/bench_library_comparison.rs
// 横向对比: EdgeBlock (Rust native) vs Simple Vec Adjacency vs Python bindings
// 测试: 图构建 / BFS / PageRank

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

fn main() {
    let datasets = [
        ("soc-Epinions1", "performance_test/datasets/soc_epinions1_100K.edgelist"),
        ("com-DBLP", "performance_test/datasets/com_dblp_100K.edgelist"),
        ("web-Google", "performance_test/datasets/web_google_100K.edgelist"),
        ("RMAT", "performance_test/datasets/rmat_1M_100K.edgelist"),
    ];

    println!("# EdgeBlock Rust Native Performance\n");
    println!("**Hardware**: Apple M4 | **Test**: Graph build, BFS, PageRank (100 iterations)\n");

    println!("| Dataset | V | E | Library | Build | BFS | PageRank |");
    println!("|---------|:---:|:---:|---------|:-----:|:---:|:--------:|");

    for (name, path) in &datasets {
        if !Path::new(path).exists() { continue; }
        let (edges, n_v) = load_edgelist(path);
        let n_e = edges.len();
        let v_label = format!("{:.0}K", n_v as f64 / 1000.0);
        let e_label = format!("{:.0}K", n_e as f64 / 1000.0);

        // ═══ 1. EdgeBlock (GraphDB) ═══
        let t0 = Instant::now();
        let mut db = GraphDB::new(GraphMode::InMemory);
        for i in 0..n_v as u64 {
            let _ = db.add_vertex(i, HashMap::new());
        }
        for &(u, v) in &edges {
            let mut props = HashMap::new();
            props.insert("weight".to_string(), PropertyValue::Double(1.0));
            let _ = db.add_edge(u, v, 1.0, props);
        }
        let t_build_db = t0.elapsed().as_millis();
        let db = db; // freeze

        let t0 = Instant::now();
        let _result = db.walk(0, n_v, |_v, _d, _p| {});
        let t_bfs_db = t0.elapsed().as_micros() as f64 / 1000.0;

        // EdgeBlock PageRank via CSR conversion
        let csr = build_csr(&edges, n_v);
        let t0 = Instant::now();
        let _pr = pagerank_correct::compute_pagerank_cpu(&csr, 100);
        let t_pr_db = t0.elapsed().as_millis();

        println!("| {} | {} | {} | EdgeBlock | {}ms | {:.2}ms | {}ms |",
            name, v_label, e_label, t_build_db, t_bfs_db, t_pr_db);

        // ═══ 2. Simple Vec<Vec<usize>> ═══
        let t0 = Instant::now();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n_v];
        for &(u, v) in &edges { adj[u as usize].push(v as usize); }
        let t_build_vec = t0.elapsed().as_millis();

        let t0 = Instant::now();
        let mut q = VecDeque::new();
        let mut vis = vec![false; n_v];
        q.push_back(0);
        vis[0] = true;
        while let Some(u) = q.pop_front() {
            for &v in &adj[u] {
                if !vis[v] { vis[v] = true; q.push_back(v); }
            }
        }
        let t_bfs_vec = t0.elapsed().as_micros() as f64 / 1000.0;

        println!("| | | | Vec<Vec> | {}ms | {:.2}ms | — |",
            t_build_vec, t_bfs_vec);

        // ═══ 3. petgraph ═══
        let t0 = Instant::now();
        let mut pg = petgraph::graph::DiGraph::<usize, ()>::new();
        let mut nodes = Vec::with_capacity(n_v);
        for i in 0..n_v { nodes.push(pg.add_node(i)); }
        for &(u, v) in &edges {
            pg.add_edge(nodes[u as usize], nodes[v as usize], ());
        }
        let t_build_pg = t0.elapsed().as_millis();

        let t0 = Instant::now();
        let mut q = VecDeque::new();
        let mut vis = vec![false; n_v];
        q.push_back(0);
        vis[0] = true;
        while let Some(u_idx) = q.pop_front() {
            let u = nodes[u_idx];
            for v_node in pg.neighbors_directed(u, petgraph::Direction::Outgoing) {
                let v = pg[v_node];
                if !vis[v] { vis[v] = true; q.push_back(v); }
            }
        }
        let t_bfs_pg = t0.elapsed().as_micros() as f64 / 1000.0;

        // petgraph pagerank (manual, 100 iters)
        let n = n_v;
        let d = 0.85;
        let t0 = Instant::now();
        let mut pr = vec![1.0 / n as f64; n];
        let mut next = vec![0.0; n];
        for _ in 0..100 {
            for v in 0..n { next[v] = (1.0 - d) / n as f64; }
            for u in 0..n {
                let out_d = pg.neighbors_directed(nodes[u], petgraph::Direction::Outgoing).count();
                if out_d == 0 { continue; }
                let share = d * pr[u] / out_d as f64;
                for v_node in pg.neighbors_directed(nodes[u], petgraph::Direction::Outgoing) {
                    let v = pg[v_node];
                    next[v] += share;
                }
            }
            std::mem::swap(&mut pr, &mut next);
        }
        let t_pr_pg = t0.elapsed().as_millis();

        println!("| | | | petgraph | {}ms | {:.2}ms | {}ms |",
            t_build_pg, t_bfs_pg, t_pr_pg);

        println!();
    }
}
