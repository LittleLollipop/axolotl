// louvain.rs — Louvain 社群检测 (EdgeBlock 块扫描原生实现)
//
// 两阶段循环:
//   Phase 1 (Local Move): 顶点逐个尝试搬到邻居社群, 选 ΔQ 最大的
//   Phase 2 (Renumber): 压缩社区编号, 准备下一轮(全量版含图折叠)
//
// ΔQ 公式 (移动 v 从 old 到 new):
//   ΔQ = (k_v_new - k_v_old)/m + k_v·(Σ_old - k_v - Σ_new)/(2·m²)
//
// 设计要点:
//   ~ 社区统计用紧凑数组 (下标 = 社区 id)
//   ~ 邻居社区聚合用小 Vec 原地清 (避免 HashMap)

use crate::gpu_edge_block::GPUEdgeBlockGraph;
use rand::Rng;

#[derive(Clone, Debug)]
struct CommInfo {
    tot: f64,
}

/// 返回 (communities, passes) — passes 为 Phase 1 迭代次数
pub fn louvain_communities(eb: &GPUEdgeBlockGraph) -> (Vec<usize>, usize) {
    let n = eb.vertex_count as usize;
    if n == 0 { return (Vec::new(), 0); }

    let bs = GPUEdgeBlockGraph::BLOCK_SIZE_U32;
    let blocks = &eb.blocks;
    let vertices = &eb.vertices;
    let block_counts = &eb.block_counts;

    let degree = eb.compute_out_degrees();
    let k_v: Vec<f64> = degree.iter().map(|&d| d as f64).collect();
    let m2: f64 = k_v.iter().sum(); // Σ degrees = 2m
    let m = m2 / 2.0;
    let m2_sq = 2.0 * m2 * m2; // 2·(2m)²

    let mut comm: Vec<usize> = (0..n).collect();
    let mut cinfo: Vec<CommInfo> = (0..n).map(|v| CommInfo { tot: k_v[v] }).collect();
    // Neighbor community weights: flat array indexed by comm id + visited list for fast reset
    let mut nbr_w: Vec<f64> = vec![0.0; n];
    let mut nbr_list: Vec<usize> = Vec::with_capacity(64);
    let mut rng = rand::thread_rng();
    let mut pass = 0usize;
    const MAX_PASSES: usize = 20;

    loop {
        pass += 1;
        if pass > MAX_PASSES { break; }
        let mut moved = false;
        let mut moves_this_pass = 0usize;
        let mut order: Vec<usize> = (0..n).collect();
        for i in 0..n {
            let j = rng.gen_range(i..n);
            order.swap(i, j);
        }

        for &v in &order {
            let old_c = comm[v];
            nbr_list.clear();

            // O(1) community weight aggregation via flat array
            let fb = vertices[v] as usize;
            let nb = block_counts[v] as usize;
            for b in 0..nb {
                let base = (fb + b) * bs;
                let ec = blocks[base + 1] as usize;
                for j in 0..ec.min(32) {
                    let w = blocks[base + 2 + j] as usize;
                    if w >= n || w == v { continue; }
                    let c = comm[w];
                    if nbr_w[c] == 0.0 { nbr_list.push(c); }
                    nbr_w[c] += 1.0;
                }
            }

            // Special case: isolated vertex (no neighbors)
            if nbr_list.is_empty() {
                continue;
            }

            let k_old = nbr_w[old_c];

            let mut best_dq = -1e-10f64;
            let mut best_c = old_c;

            for &c in &nbr_list {
                if c == old_c { continue; }
                let k_new = nbr_w[c];
                let dq = (k_new - k_old) / m
                    + k_v[v] * (cinfo[old_c].tot - k_v[v] - cinfo[c].tot) / m2_sq;
                if dq > best_dq {
                    best_dq = dq;
                    best_c = c;
                }
            }

            // Reset for next vertex
            for &c in &nbr_list { nbr_w[c] = 0.0; }

            if best_c != old_c {
                moved = true;
                moves_this_pass += 1;
                cinfo[old_c].tot -= k_v[v];
                cinfo[best_c].tot += k_v[v];
                comm[v] = best_c;
            }
        }

        // Early stop: if very few vertices move, convergence reached
        if moves_this_pass < (n as f64).sqrt() as usize { break; }
        if !moved { break; }

        // Renumber communities to compact IDs
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
        let mut new_info: Vec<CommInfo> = vec![CommInfo { tot: 0.0 }; nc];
        for v in 0..n {
            new_info[comm[v]].tot += k_v[v];
        }
        cinfo = new_info;
        nbr_w.resize(n, 0.0); // keep flat array large enough
    }

    (comm, pass.min(MAX_PASSES))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_edge_block::GPUEdgeBlockGraph;
    use std::collections::HashMap;

    fn build(edges: &[(u32, u32)]) -> GPUEdgeBlockGraph {
        let mut g = GPUEdgeBlockGraph::new();
        let max_v = edges.iter().flat_map(|&(u,v)| [u,v]).max().unwrap_or(0) as u64;
        for i in 0..=max_v { g.add_vertex(i, HashMap::new()); }
        for &(u, v) in edges { g.add_edge(u as u64, v as u64, 1.0); g.add_edge(v as u64, u as u64, 1.0); }
        g
    }

    #[test]
    fn test_two_cliques() {
        // Clique A: 0-1-2, Clique B: 3-4-5, bridge 2-3
        let g = build(&[(0,1),(1,2),(2,0),(3,4),(4,5),(5,3),(2,3)]);
        let (r, _) = louvain_communities(&g);
        let a = r[0]; let b = r[3];
        assert!(a != b, "separate cliques");
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
