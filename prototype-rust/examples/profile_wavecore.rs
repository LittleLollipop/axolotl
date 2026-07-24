// Minimal profile of Wave Core vs petgraph on Epinions1
fn main() {
    use std::time::Instant;
    use std::collections::HashMap;
    use std::fs;
    use std::io::{BufRead, BufReader};
    use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;
    use axolotl_rs::wave_core::wave_core_blocks;
    use axolotl_rs::PropertyValue;

    // Load
    let path = "performance_test/datasets/soc_epinions1_100K.edgelist";
    let f = fs::File::open(path).unwrap();
    let mut edges = Vec::new();
    let mut max_v = 0u64;
    for line in BufReader::new(f).lines() {
        let line = line.unwrap();
        if line.is_empty() || line.starts_with('#') { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let u: u64 = parts[0].parse().unwrap();
            let v: u64 = parts[1].parse().unwrap();
            if u != v { edges.push((u, v)); max_v = max_v.max(u).max(v); }
        }
    }
    let n_v = (max_v + 1) as usize;
    println!("{} vertices, {} edges", n_v, edges.len());

    // Build EdgeBlock graph
    let t0 = Instant::now();
    let mut eb = GPUEdgeBlockGraph::new();
    for i in 0..n_v as u64 { eb.add_vertex(i, HashMap::new()); }
    for (u, v) in &edges { eb.add_edge(*u, *v, 1.0); eb.add_edge(*v, *u, 1.0); }
    let t_build = t0.elapsed();
    println!("Build: {:.0}ms", t_build.as_secs_f64() * 1000.0);

    // Measure Wave Core phases
    let n = eb.vertex_count as usize;
    let block_stride = GPUEdgeBlockGraph::BLOCK_SIZE_U32;
    let blocks = &eb.blocks;
    let vertices = &eb.vertices;
    let block_counts = &eb.block_counts;

    let t0 = Instant::now();
    let degree = eb.compute_out_degrees();
    let t_deg = t0.elapsed();
    println!("compute_out_degrees: {:.0}ms", t_deg.as_secs_f64() * 1000.0);

    let max_deg = *degree.iter().max().unwrap_or(&0) as usize;
    let t0 = Instant::now();
    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); max_deg + 1];
    for v in 0..n { bins[degree[v] as usize].push(v); }
    let t_sort = t0.elapsed();
    println!("bin_sort (max_deg={max_deg}): {:.0}ms", t_sort.as_secs_f64() * 1000.0);

    let t0 = Instant::now();
    let mut degree2 = degree.clone();
    let mut core = vec![0u32; n];
    let mut peeled = vec![false; n];
    let mut peeled_count = 0usize;
    let mut total_decrements = 0u64;
    let mut total_pushes = 0u64;
    let mut total_blocks_visited = 0u64;
    let mut outer_loops = 0u64;

    while peeled_count < n {
        outer_loops += 1;
        let mut found = false;
        for current_bin in 0..=max_deg {
            if bins[current_bin].is_empty() { continue; }
            let batch = std::mem::take(&mut bins[current_bin]);
            for &v in &batch {
                if peeled[v] { continue; }
                core[v] = current_bin as u32;
                peeled[v] = true;
                peeled_count += 1;

                let first_block = vertices[v] as usize;
                let num_blocks = block_counts[v] as usize;
                total_blocks_visited += num_blocks as u64;
                for b in 0..num_blocks {
                    let base = (first_block + b) * block_stride;
                    let ec = blocks[base + 1] as usize;
                    for j in 0..ec.min(32) {
                        let nb = blocks[base + 2 + j] as usize;
                        if nb >= n || peeled[nb] { continue; }
                        let od = degree2[nb] as usize;
                        if od <= current_bin { continue; }
                        degree2[nb] -= 1;
                        total_decrements += 1;
                        bins[od - 1].push(nb);
                        total_pushes += 1;
                    }
                }
            }
            found = true;
            break;
        }
        if !found { break; }
    }
    let t_peel = t0.elapsed();
    println!("peel: {:.0}ms (loops={outer_loops} decrements={total_decrements} pushes={total_pushes} blocks_visited={total_blocks_visited})",
        t_peel.as_secs_f64() * 1000.0);

    // Petgraph version for reference
    let t0 = Instant::now();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n_v];
    for &(u, v) in &edges { adj[u as usize].push(v as usize); adj[v as usize].push(u as usize); }
    let mut deg: Vec<u32> = adj.iter().map(|nbrs| nbrs.len() as u32).collect();
    let md = *deg.iter().max().unwrap_or(&0) as usize;
    let mut bins2: Vec<Vec<usize>> = vec![Vec::new(); md + 1];
    for v in 0..n_v { bins2[deg[v] as usize].push(v); }
    let mut core2 = vec![0u32; n_v];
    let mut peeled2 = vec![false; n_v];
    let mut pc2 = 0;
    while pc2 < n_v {
        let mut found = false;
        for cb in 0..=md {
            if bins2[cb].is_empty() { continue; }
            let batch = std::mem::take(&mut bins2[cb]);
            for &v in &batch {
                if peeled2[v] { continue; }
                core2[v] = cb as u32;
                peeled2[v] = true;
                pc2 += 1;
                for &nb in &adj[v] {
                    if peeled2[nb] { continue; }
                    if deg[nb] as usize <= cb { continue; }
                    deg[nb] -= 1;
                    bins2[deg[nb] as usize].push(nb);
                }
            }
            found = true;
            break;
        }
        if !found { break; }
    }
    let t_pg = t0.elapsed();
    println!("petgraph BZ: {:.0}ms (same data, {n_v} nodes)", t_pg.as_secs_f64() * 1000.0);
}
