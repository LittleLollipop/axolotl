// examples/bench_algorithms.rs
// 算法级性能对比：EdgeBlock 统一内存架构 vs 旧 HashMap+CSR 架构
//
// 测试算法：PageRank, BFS, SSSP, CC, Triangle Counting
// 测试规模：1K, 10K, 50K, 100K

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

fn main() {
    println!("# 算法性能对比：EdgeBlock 统一内存 vs HashMap+CSR\n");
    println!("**硬件**：Apple M4 统一内存\n");
    println!("**图类型**：随机图（平均 degree=5）\n");

    let scales = vec![
        ("1K",  1_000,     5_000),
        ("10K", 10_000,    50_000),
        ("50K", 50_000,   250_000),
    ];

    // ==== PageRank ====
    println!("## PageRank\n");
    println!("| 规模 | 旧构建 | 旧CSR | 旧PR | 新构建 | 新PR | 节省 |");
    println!("|------|--------|-------|------|--------|------|------|");
    for (label, n_v, n_e) in &scales {
        let (t_old_build, t_old_csr, t_old_pr, t_new_build, t_new_pr) =
            bench_pagerank(*n_v, *n_e);
        let saved = format_duration(Duration::from_secs_f64(
            (t_old_build + t_old_csr).as_secs_f64() - t_new_build.as_secs_f64()
        ));
        println!("| {} | {} | {} | {} | {} | {} | {} |",
            label, fd(t_old_build), fd(t_old_csr), fd(t_old_pr),
            fd(t_new_build), fd(t_new_pr), saved);
    }
    println!();

    // ==== BFS ====
    println!("## BFS\n");
    println!("| 规模 | 旧构建 | 旧CSR | 旧BFS | 新构建 | 新BFS | 节省 |");
    println!("|------|--------|-------|-------|--------|-------|------|");
    for (label, n_v, n_e) in &scales {
        let (t_old, t_csr, t_old_bfs, t_new, t_new_bfs) =
            bench_bfs(*n_v, *n_e);
        let saved = format_duration(Duration::from_secs_f64(
            (t_old + t_csr).as_secs_f64() - t_new.as_secs_f64()
        ));
        println!("| {} | {} | {} | {} | {} | {} | {} |",
            label, fd(t_old), fd(t_csr), fd(t_old_bfs),
            fd(t_new), fd(t_new_bfs), saved);
    }
    println!();

    // ==== Connected Components ====
    println!("## Connected Components\n");
    println!("| 规模 | 旧构建 | 旧CSR | 旧CC | 新构建 | 新CC | 节省 |");
    println!("|------|--------|-------|------|--------|------|------|");
    for (label, n_v, n_e) in &scales {
        let (t_old, t_csr, t_old_cc, t_new, t_new_cc) =
            bench_cc(*n_v, *n_e);
        let saved = format_duration(Duration::from_secs_f64(
            (t_old + t_csr).as_secs_f64() - t_new.as_secs_f64()
        ));
        println!("| {} | {} | {} | {} | {} | {} | {} |",
            label, fd(t_old), fd(t_csr), fd(t_old_cc),
            fd(t_new), fd(t_new_cc), saved);
    }
    println!();

    // ==== SSSP ====
    println!("## SSSP（单源最短路径）\n");
    println!("| 规模 | 旧构建 | 旧CSR | 旧SSSP | 新构建 | 新SSSP | 节省 |");
    println!("|------|--------|-------|--------|--------|--------|------|");
    for (label, n_v, n_e) in &scales {
        let (t_old, t_csr, t_old_s, t_new, t_new_s) =
            bench_sssp(*n_v, *n_e);
        let saved = format_duration(Duration::from_secs_f64(
            (t_old + t_csr).as_secs_f64() - t_new.as_secs_f64()
        ));
        println!("| {} | {} | {} | {} | {} | {} | {} |",
            label, fd(t_old), fd(t_csr), fd(t_old_s),
            fd(t_new), fd(t_new_s), saved);
    }
    println!();

    // ==== 汇总 ====
    println!("## 汇总\n");
    println!("| 算法 | 1K | 10K | 50K | 核心结论 |");
    println!("|------|-----|------|------|----------|");
    for algo in &["PageRank", "BFS", "CC", "SSSP"] {
        print!("| {} |", algo);
        for (label, n_v, n_e) in &scales {
            let (t_old, t_csr, t_old_a, t_new, _) = match *algo {
                "PageRank" => {
                    let (a,b,c,d,e) = bench_pagerank(*n_v, *n_e);
                    (a,b,c,d,e)
                }
                "BFS" => {
                    let (a,b,c,d,e) = bench_bfs(*n_v, *n_e);
                    (a,b,c,d,e)
                }
                "CC" => {
                    let (a,b,c,d,e) = bench_cc(*n_v, *n_e);
                    (a,b,c,d,e)
                }
                _ => {
                    let (a,b,c,d,e) = bench_sssp(*n_v, *n_e);
                    (a,b,c,d,e)
                }
            };
            let old_tot = t_old.as_secs_f64() + t_csr.as_secs_f64();
            let speedup = if t_new.as_secs_f64() > 0.0 {
                old_tot / t_new.as_secs_f64()
            } else { 1.0 };
            print!(" {:.1}x |", speedup);
        }
        println!(" - |");
    }
}

fn fd(d: Duration) -> String { format_duration(d) }

fn format_duration(d: Duration) -> String {
    if d.as_secs() > 0 {
        format!("{:.1}s", d.as_secs_f64())
    } else if d.as_millis() > 1 {
        format!("{:.1}ms", d.as_secs_f64() * 1000.0)
    } else {
        format!("{:.0}µs", d.as_secs_f64() * 1_000_000.0)
    }
}

// ── 图数据生成 ─────────────────────────

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

// ── 旧架构辅助 ─────────────────────────

fn build_old_graph(edges: &[(u64, u64)], n_v: usize) 
    -> (axolotl_rs::persistence::PersistentGraph, Duration) 
{
    let t0 = Instant::now();
    let mut pg = axolotl_rs::persistence::PersistentGraph {
        vertices: HashMap::new(),
        edges: HashMap::new(),
        file_path: String::new(),
        index_manager: axolotl_rs::index::IndexManager::new(),
    };
    for v in 0..n_v as u64 {
        pg.add_vertex(v, HashMap::new());
    }
    for &(u, v) in edges {
        pg.add_edge(u, v, 1.0, HashMap::new());
    }
    let t = t0.elapsed();
    (pg, t)
}

fn build_new_graph(edges: &[(u64, u64)], n_v: usize) 
    -> (axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph, Duration) 
{
    let t0 = Instant::now();
    let mut eb = axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph::new();
    for v in 0..n_v as u64 {
        eb.add_vertex(v, HashMap::new());
    }
    for &(u, v) in edges {
        eb.add_edge(u, v, 1.0);
    }
    let t = t0.elapsed();
    (eb, t)
}

// ── PageRank ─────────────────────────

fn bench_pagerank(n_v: usize, n_e: usize)
    -> (Duration, Duration, Duration, Duration, Duration)
{
    let edges = generate_edges(n_v, n_e);

    // 旧架构
    let (pg, t_old) = build_old_graph(&edges, n_v);
    let t0 = Instant::now();
    let csr = pg.to_csr();
    let t_csr = t0.elapsed();

    let t0 = Instant::now();
    let pr = axolotl_rs::pagerank_correct::compute_pagerank_cpu(&csr, 100);
    let t_old_pr = t0.elapsed();
    let _ = pr.iter().sum::<f32>();

    // 新架构
    let (eb, t_new) = build_new_graph(&edges, n_v);
    let t0 = Instant::now();
    // 用 EdgeBlock 构建 CSR 然后跑 PageRank（模拟 GPU 数据路径）
    let csr2 = edgeblock_to_csr(&eb);
    let pr2 = axolotl_rs::pagerank_correct::compute_pagerank_cpu(&csr2, 100);
    let t_new_pr = t0.elapsed();
    let _ = pr2.iter().sum::<f32>();

    (t_old, t_csr, t_old_pr, t_new, t_new_pr)
}

fn edgeblock_to_csr(eb: &axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph) 
    -> axolotl_rs::CSRGraph
{
    let mut csr = axolotl_rs::CSRGraph::new();
    for i in 0..eb.vertex_count as usize {
        let id = eb.idx_to_id[i];
        let props = HashMap::new();
        csr.add_vertex(id, props);
    }
    let mut all_edges = Vec::new();
    for i in 0..eb.vertex_count as usize {
        let from_id = eb.idx_to_id[i];
        for to_id in eb.out_neighbors_by_idx(i) {
            all_edges.push((from_id, to_id));
        }
    }
    csr.build_csr(&all_edges);
    csr.weights = vec![1.0f32; all_edges.len()];
    csr
}

// ── BFS ─────────────────────────

fn bench_bfs(n_v: usize, n_e: usize)
    -> (Duration, Duration, Duration, Duration, Duration)
{
    let edges = generate_edges(n_v, n_e);

    // 旧架构
    let (pg, t_old) = build_old_graph(&edges, n_v);
    let t0 = Instant::now();
    let csr = pg.to_csr();
    let t_csr = t0.elapsed();

    let t0 = Instant::now();
    csr_bfs(&csr, 0);
    let t_old_bfs = t0.elapsed();

    // 新架构
    let (eb, t_new) = build_new_graph(&edges, n_v);
    let t0 = Instant::now();
    edgeblock_bfs(&eb, 0);
    let t_new_bfs = t0.elapsed();

    (t_old, t_csr, t_old_bfs, t_new, t_new_bfs)
}

fn csr_bfs(csr: &axolotl_rs::CSRGraph, start: u64) -> Vec<u32> {
    let n = csr.vertex_count as usize;
    let mut dist = vec![u32::MAX; n];
    let mut queue = VecDeque::new();

    if let Some(&idx) = csr.vertex_to_idx.get(&start) {
        dist[idx as usize] = 0;
        queue.push_back(idx);
    }

    while let Some(v_idx) = queue.pop_front() {
        let d = dist[v_idx as usize] + 1;
        let start = csr.offsets[v_idx as usize] as usize;
        let end = csr.offsets[v_idx as usize + 1] as usize;
        for i in start..end {
            let nbr = csr.targets[i] as usize;
            if dist[nbr] == u32::MAX {
                dist[nbr] = d;
                queue.push_back(nbr as u32);
            }
        }
    }
    dist
}

fn edgeblock_bfs(eb: &axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph, start_id: u64) -> Vec<u32> {
    let n = eb.vertex_count as usize;
    let mut dist = vec![u32::MAX; n];
    let mut queue = VecDeque::new();

    if let Some(&start_idx) = eb.id_to_idx.get(&start_id) {
        dist[start_idx] = 0;
        queue.push_back(start_idx);
    }

    while let Some(v_idx) = queue.pop_front() {
        let d = dist[v_idx] + 1;
        let first = eb.vertices[v_idx] as usize;
        let count = eb.block_counts[v_idx] as usize;
        for b in 0..count {
            let off = (first + b) * axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph::BLOCK_SIZE_U32;
            let ec = eb.blocks[off + 1] as usize;
            for e in 0..ec {
                let nbr = eb.blocks[off + 2 + e] as usize;
                if dist[nbr] == u32::MAX {
                    dist[nbr] = d;
                    queue.push_back(nbr);
                }
            }
        }
    }
    dist
}

// ── Connected Components ─────────────────────────

fn bench_cc(n_v: usize, n_e: usize)
    -> (Duration, Duration, Duration, Duration, Duration)
{
    let edges = generate_edges(n_v, n_e);

    let (pg, t_old) = build_old_graph(&edges, n_v);
    let t0 = Instant::now();
    let csr = pg.to_csr();
    let t_csr = t0.elapsed();

    let t0 = Instant::now();
    let cc = csr_cc(&csr);
    let t_old_cc = t0.elapsed();

    let (eb, t_new) = build_new_graph(&edges, n_v);
    let t0 = Instant::now();
    let cc2 = edgeblock_cc(&eb);
    let t_new_cc = t0.elapsed();

    assert_eq!(cc.len(), cc2.len());
    (t_old, t_csr, t_old_cc, t_new, t_new_cc)
}

fn csr_cc(csr: &axolotl_rs::CSRGraph) -> Vec<u32> {
    let n = csr.vertex_count as usize;
    let mut parent: Vec<u32> = (0..n as u32).collect();

    fn find(p: &mut [u32], x: u32) -> u32 {
        if p[x as usize] != x {
            p[x as usize] = find(p, p[x as usize]);
        }
        p[x as usize]
    }
    fn union(p: &mut [u32], a: u32, b: u32) {
        let ra = find(p, a);
        let rb = find(p, b);
        if ra != rb {
            p[ra as usize] = rb;
        }
    }

    for v in 0..n {
        let s = csr.offsets[v] as usize;
        let e = csr.offsets[v + 1] as usize;
        for i in s..e {
            union(&mut parent, v as u32, csr.targets[i]);
        }
    }

    for i in 0..n {
        parent[i] = find(&mut parent, i as u32);
    }
    parent
}

fn edgeblock_cc(eb: &axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph) -> Vec<u32> {
    let n = eb.vertex_count as usize;
    let mut parent: Vec<u32> = (0..n as u32).collect();

    fn find(p: &mut [u32], x: u32) -> u32 {
        if p[x as usize] != x {
            p[x as usize] = find(p, p[x as usize]);
        }
        p[x as usize]
    }
    fn union(p: &mut [u32], a: u32, b: u32) {
        let ra = find(p, a);
        let rb = find(p, b);
        if ra != rb {
            p[ra as usize] = rb;
        }
    }

    for v in 0..n {
        let first = eb.vertices[v] as usize;
        let count = eb.block_counts[v] as usize;
        for b in 0..count {
            let off = (first + b) * axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph::BLOCK_SIZE_U32;
            let ec = eb.blocks[off + 1] as usize;
            for e in 0..ec {
                union(&mut parent, v as u32, eb.blocks[off + 2 + e]);
            }
        }
    }

    for i in 0..n {
        parent[i] = find(&mut parent, i as u32);
    }
    parent
}

// ── SSSP ─────────────────────────

fn bench_sssp(n_v: usize, n_e: usize)
    -> (Duration, Duration, Duration, Duration, Duration)
{
    let edges = generate_edges(n_v, n_e);

    let (pg, t_old) = build_old_graph(&edges, n_v);
    let t0 = Instant::now();
    let csr = pg.to_csr();
    let t_csr = t0.elapsed();

    let t0 = Instant::now();
    let d = csr_sssp(&csr, 0);
    let t_old_sssp = t0.elapsed();

    let (eb, t_new) = build_new_graph(&edges, n_v);
    let t0 = Instant::now();
    let d2 = edgeblock_sssp(&eb, 0);
    let t_new_sssp = t0.elapsed();

    assert!(!d.is_empty() && !d2.is_empty());

    (t_old, t_csr, t_old_sssp, t_new, t_new_sssp)
}

fn csr_sssp(csr: &axolotl_rs::CSRGraph, start: u64) -> Vec<u32> {
    use std::collections::BinaryHeap;
    use std::cmp::Reverse;

    let n = csr.vertex_count as usize;
    let mut dist = vec![u32::MAX; n];

    if let Some(&idx) = csr.vertex_to_idx.get(&start) {
        dist[idx as usize] = 0;
    } else { return dist; }

    let mut pq = BinaryHeap::new();
    if let Some(&idx) = csr.vertex_to_idx.get(&start) {
        pq.push(Reverse((0u32, idx)));
    }

    while let Some(Reverse((d, v))) = pq.pop() {
        if d > dist[v as usize] { continue; }
        let s = csr.offsets[v as usize] as usize;
        let e = csr.offsets[v as usize + 1] as usize;
        for i in s..e {
            let nbr = csr.targets[i] as u32;
            let nd = d + 1;
            if nd < dist[nbr as usize] {
                dist[nbr as usize] = nd;
                pq.push(Reverse((nd, nbr)));
            }
        }
    }
    dist
}

fn edgeblock_sssp(eb: &axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph, start_id: u64) -> Vec<u32> {
    use std::collections::BinaryHeap;
    use std::cmp::Reverse;

    let n = eb.vertex_count as usize;
    let mut dist = vec![u32::MAX; n];

    if let Some(&idx) = eb.id_to_idx.get(&start_id) {
        dist[idx] = 0;
    } else { return dist; }

    let mut pq = BinaryHeap::new();
    if let Some(&idx) = eb.id_to_idx.get(&start_id) {
        pq.push(Reverse((0u32, idx)));
    }

    while let Some(Reverse((d, v))) = pq.pop() {
        if d > dist[v] { continue; }
        let nd = d + 1;
        let first = eb.vertices[v] as usize;
        let count = eb.block_counts[v] as usize;
        for b in 0..count {
            let off = (first + b) * axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph::BLOCK_SIZE_U32;
            let ec = eb.blocks[off + 1] as usize;
            for e in 0..ec {
                let nbr = eb.blocks[off + 2 + e] as usize;
                if nd < dist[nbr] {
                    dist[nbr] = nd;
                    pq.push(Reverse((nd, nbr)));
                }
            }
        }
    }
    dist
}
