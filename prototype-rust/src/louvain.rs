// louvain.rs — Louvain 社群检测 (EdgeBlock 块扫描原生实现)
//
// 两阶段循环:
//   Phase 1 (Local Move): 顶点逐个尝试搬到邻居社群, 选最大 ΔQ
//   Phase 2 (Aggregation): 将社区折叠为超级节点, 重建图
//   重复直到收敛或图不再变化
//
// ΔQ 公式 (移动 v 从 old 到 new):
//   ΔQ = (k_v_new - k_v_old)/m + k_v·(Σ_old - k_v - Σ_new)/(2·m²)

use crate::gpu_edge_block::GPUEdgeBlockGraph;
use std::collections::HashMap;

/// Internal: Phase 1 local move on flat community arrays
/// Returns (communities, moved_count, num_communities)
fn phase1_local_move(
    comm: &mut [usize],
    k_v: &[f64],
    m: f64,
    m2_sq: f64,
    blocks: &[u32],
    vertices: &[u32],
    block_counts: &[u32],
    bs: usize,
) -> (bool, usize, usize) {
    let n = comm.len();
    let mut cinfo: Vec<(f64, f64)> = vec![(0.0, 0.0); n];
    for v in 0..n { cinfo[comm[v]].0 += k_v[v]; } // tot per community

    let mut nbr_w: Vec<f64> = vec![0.0; n];
    let mut nbr_list: Vec<usize> = Vec::with_capacity(64);
    let mut moved = false;
    let mut moves = 0usize;

    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut order: Vec<usize> = (0..n).collect();
    for i in 0..n {
        let j = rng.gen_range(i..n);
        order.swap(i, j);
    }

    for &v in &order {
        let old_c = comm[v];
        nbr_list.clear();

        let fb = vertices[v] as usize;
        let nb = block_counts[v] as usize;
        for b in 0..nb {
            let base = (fb + b) * bs;
            let ec = blocks[base + 1] as usize;
            for j in 0..ec.min(32) {
                let w = blocks[base + 2 + j] as usize;
                if w >= n { continue; }
                let c = comm[w];
                if nbr_w[c] == 0.0 { nbr_list.push(c); }
                nbr_w[c] += 1.0;
            }
        }

        if nbr_list.is_empty() { continue; }

        let k_old = nbr_w[old_c];
        let mut best_dq = -1e-10f64;
        let mut best_c = old_c;

        for &c in &nbr_list {
            if c == old_c { continue; }
            let k_new = nbr_w[c];
            let dq = (k_new - k_old) / m
                + k_v[v] * (cinfo[old_c].0 - k_v[v] - cinfo[c].0) / m2_sq;
            if dq > best_dq {
                best_dq = dq;
                best_c = c;
            }
        }

        for &c in &nbr_list { nbr_w[c] = 0.0; }

        if best_c != old_c {
            moved = true;
            moves += 1;
            cinfo[old_c].0 -= k_v[v];
            cinfo[best_c].0 += k_v[v];
            comm[v] = best_c;
        }
    }

    // Count unique communities
    let mut seen = vec![false; n];
    let mut nc = 0usize;
    for &c in comm.iter() { if !seen[c] { seen[c] = true; nc += 1; } }

    (moved, moves, nc)
}

/// Renumber communities to compact IDs
fn renumber(comm: &mut [usize]) -> usize {
    let n = comm.len();
    let mut renum = vec![usize::MAX; n];
    let mut nc = 0usize;
    for v in 0..n {
        let c = comm[v];
        if renum[c] == usize::MAX {
            renum[c] = nc;
            nc += 1;
        }
        comm[v] = renum[c];
    }
    nc
}

/// Louvain community detection with Phase 2 aggregation
pub fn louvain_communities(eb: &GPUEdgeBlockGraph) -> (Vec<usize>, usize) {
    let n = eb.vertex_count as usize;
    if n == 0 { return (Vec::new(), 0); }

    let bs = GPUEdgeBlockGraph::BLOCK_SIZE_U32;
    let blocks = &eb.blocks;
    let vertices = &eb.vertices;
    let block_counts = &eb.block_counts;

    let degree = eb.compute_out_degrees();
    let k_v: Vec<f64> = degree.iter().map(|&d| d as f64).collect();
    let m2: f64 = k_v.iter().sum();
    let m = m2 / 2.0;
    let m2_sq = 2.0 * m2 * m2;

    let mut comm: Vec<usize> = (0..n).collect();
    let mut phase = 0usize;
    const MAX_PHASES: usize = 10;

    loop {
        phase += 1;
        if phase > MAX_PHASES { break; }

        // Phase 1: local move on current graph
        let curr_n = comm.len();
        let (moved, moves, _nc) = phase1_local_move(
            &mut comm, &k_v, m, m2_sq, blocks, vertices, block_counts, bs,
        );

        // Check convergence
        if !moved || moves < (curr_n as f64).sqrt() as usize { break; }

        // Renumber for clean community IDs
        let nc = renumber(&mut comm);

        // Phase 2: aggregate into super-graph
        let mut super_edges: HashMap<(usize, usize), f64> = HashMap::new();
        let mut internal_weight: Vec<f64> = vec![0.0; nc];

        for v in 0..curr_n {
            let fbv = vertices[v] as usize;
            let nb = block_counts[v] as usize;
            for b in 0..nb {
                let base = (fbv + b) * bs;
                let ec = blocks[base + 1] as usize;
                for j in 0..ec.min(32) {
                    let w = blocks[base + 2 + j] as usize;
                    if w >= curr_n || w == v { continue; }
                    let cu = comm[v];
                    let cv = comm[w];
                    if cu == cv {
                        internal_weight[cu] += 1.0;
                    } else {
                        let key = (cu.min(cv), cu.max(cv));
                        *super_edges.entry(key).or_insert(0.0) += 1.0;
                    }
                }
            }
        }

        // Build new graph: super-nodes
        let mut super_g = GPUEdgeBlockGraph::new();
        for c in 0..nc {
            super_g.add_vertex(c as u64, HashMap::new());
            if internal_weight[c] > 0.0 {
                super_g.add_edge(c as u64, c as u64, internal_weight[c] as f32);
            }
        }
        for ((u, v), w) in super_edges {
            super_g.add_edge(u as u64, v as u64, w as f32);
            super_g.add_edge(v as u64, u as u64, w as f32);
        }

        // Map original vertex degrees to super-nodes
        let mut super_kv: Vec<f64> = vec![0.0; nc];
        for v in 0..curr_n {
            super_kv[comm[v]] += k_v[v];
        }

        // Run Phase 1 on super-graph (use ORIGINAL m)
        let super_n = nc;
        let mut super_comm: Vec<usize> = (0..super_n).collect();
        let (_, _, _) = phase1_local_move(
            &mut super_comm,
            &super_kv,
            m,     // ORIGINAL m — NOT recomputed
            m2_sq, // ORIGINAL m2_sq
            &super_g.blocks,
            &super_g.vertices,
            &super_g.block_counts,
            bs,
        );

        // Map super-communities back to original vertices
        for v in 0..curr_n {
            comm[v] = super_comm[comm[v]];
        }

        // If super-graph barely changed, stop
        let mut super_seen = vec![false; super_n];
        let mut super_nc = 0usize;
        for &c in super_comm.iter() { if !super_seen[c] { super_seen[c] = true; super_nc += 1; } }
        if super_nc == super_n { break; } // No change in super-graph
    }

    renumber(&mut comm);
    (comm, phase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_edge_block::GPUEdgeBlockGraph;

    fn build(edges: &[(u32, u32)]) -> GPUEdgeBlockGraph {
        let mut g = GPUEdgeBlockGraph::new();
        let max_v = edges.iter().flat_map(|&(u,v)| [u,v]).max().unwrap_or(0) as u64;
        for i in 0..=max_v { g.add_vertex(i, HashMap::new()); }
        for &(u, v) in edges { g.add_edge(u as u64, v as u64, 1.0); g.add_edge(v as u64, u as u64, 1.0); }
        g
    }

    #[test]
    fn test_two_cliques() {
        let g = build(&[(0,1),(1,2),(2,0),(3,4),(4,5),(5,3),(2,3)]);
        let (r, _) = louvain_communities(&g);
        assert!(r[0] != r[3], "separate cliques");
        assert_eq!(r[0], r[1]); assert_eq!(r[0], r[2]);
        assert_eq!(r[3], r[4]); assert_eq!(r[3], r[5]);
    }

    #[test]
    fn test_single() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(0, HashMap::new());
        assert_eq!(louvain_communities(&g).0, vec![0]);
    }

    #[test]
    fn test_triangle() {
        let g = build(&[(0,1),(1,2),(2,0)]);
        let (r, _) = louvain_communities(&g);
        assert_eq!(r[0], r[1]);
        assert_eq!(r[1], r[2]);
    }
}
