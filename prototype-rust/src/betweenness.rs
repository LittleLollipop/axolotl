// betweenness.rs — Brandes 介数中心性 (采样版, EdgeBlock 块扫描)
//
// Brandes 算法: 对每个源点 s
//   1. BFS 发现最短路 DAG (前驱列表 + σ计数)
//   2. 反向累加依赖值 δ
//
// 全量 O(V×E) 在大图上不可行, 改用采样近似:
//   ~ 默认 256 个随机源点
//   ~ 精度随 √samples 收敛

use crate::gpu_edge_block::GPUEdgeBlockGraph;
use rand::Rng;

pub fn brandes_betweenness(eb: &GPUEdgeBlockGraph, num_sources: Option<usize>) -> Vec<f64> {
    let n = eb.vertex_count as usize;
    if n == 0 { return Vec::new(); }

    let bs = GPUEdgeBlockGraph::BLOCK_SIZE_U32;
    let blocks = &eb.blocks;
    let vertices = &eb.vertices;
    let block_counts = &eb.block_counts;

    let k = num_sources.unwrap_or(256).min(n);

    // Collect source vertices (sample without replacement)
    let sources: Vec<usize> = if k >= n {
        (0..n).collect()
    } else {
        let mut rng = rand::thread_rng();
        let mut sources: Vec<usize> = (0..n).collect();
        for i in 0..k {
            let j = rng.gen_range(i..n);
            sources.swap(i, j);
        }
        sources[..k].to_vec()
    };

    let mut bc = vec![0.0f64; n];

    // Reusable buffers (allocated once)
    let mut dist = vec![u32::MAX; n];
    let mut sigma = vec![0u64; n];
    let mut delta = vec![0.0f64; n];
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut queue = std::collections::VecDeque::with_capacity(n);
    let mut order = Vec::with_capacity(n); // BFS vertex order (farthest first)

    for &s in &sources {

        // Reset
        queue.clear();
        order.clear();
        dist[s] = 0;
        sigma[s] = 1;
        queue.push_back(s);

        // Forward BFS: discover shortest paths
        while let Some(v) = queue.pop_front() {
            order.push(v);
            let dv = dist[v];
            let sv = sigma[v];

            let first_block = vertices[v] as usize;
            let num_blocks = block_counts[v] as usize;
            for b in 0..num_blocks {
                let base = (first_block + b) * bs;
                let ec = blocks[base + 1] as usize;
                for j in 0..ec.min(32) {
                    let w = blocks[base + 2 + j] as usize;
                    if w >= n || w == v { continue; }
                    if dist[w] == u32::MAX {
                        dist[w] = dv + 1;
                        sigma[w] = sv;
                        pred[w].push(v);
                        queue.push_back(w);
                    } else if dist[w] == dv + 1 {
                        sigma[w] += sv;
                        pred[w].push(v);
                    }
                }
            }
        }

        // Backward dependency accumulation (reverse BFS order)
        for &w in order.iter().rev() {
            if w == s { continue; }
            let dw = delta[w];
            let sw = sigma[w] as f64;
            for &p in &pred[w] {
                if sigma[p] > 0 {
                    delta[p] += (sigma[p] as f64 / sw) * (1.0 + dw);
                }
            }
            bc[w] += dw;
        }

        // Accumulate for source
        bc[s] += delta[s];

        // Reset per-source state
        for &v in &order {
            dist[v] = u32::MAX;
            sigma[v] = 0;
            delta[v] = 0.0;
            pred[v].clear();
        }
    }

    // Normalize: brandes scores / ((n-1)*(n-2))
    // For directed graph, the full formula divides by (n-1)*(n-2)
    // For sampled, scale up by n/k
    let scale = n as f64 / k as f64;
    let norm = if n > 2 { 1.0 / ((n - 1) as f64 * (n - 2) as f64) } else { 1.0 };
    for v in 0..n {
        bc[v] *= scale * norm;
    }

    bc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_edge_block::GPUEdgeBlockGraph;
    use std::collections::HashMap;

    fn build_graph(edges: &[(u32, u32)]) -> GPUEdgeBlockGraph {
        let mut g = GPUEdgeBlockGraph::new();
        let max_v = edges.iter().flat_map(|&(u,v)| [u,v]).max().unwrap_or(0) as u64;
        for i in 0..=max_v { g.add_vertex(i, HashMap::new()); }
        for &(u, v) in edges { g.add_edge(u as u64, v as u64, 1.0); }
        g
    }

    #[test]
    fn test_line() {
        // 0→1→2→3: vertices 1 and 2 have highest betweenness
        let g = build_graph(&[(0,1),(1,2),(2,3)]);
        // Use all vertices as sources for exact result
        let bc = brandes_betweenness(&g, Some(4));
        assert!(bc[1] > bc[0], "vertex 1 should have higher bc than 0");
        assert!(bc[2] >= bc[0], "vertex 2 should be >= 0");
    }

    #[test]
    fn test_star_center() {
        // center=0, leaves=1,2,3: center should have highest BC
        let g = build_graph(&[(0,1),(0,2),(0,3)]);
        let bc = brandes_betweenness(&g, Some(4));
        assert!(bc[0] > bc[1], "center should dominate");
        assert!(bc[0] > bc[2], "center should dominate");
        assert!(bc[0] > bc[3], "center should dominate");
    }

    #[test]
    fn test_empty() {
        let g = GPUEdgeBlockGraph::new();
        assert!(brandes_betweenness(&g, None).is_empty());
    }

    #[test]
    fn test_single() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(0, HashMap::new());
        assert_eq!(brandes_betweenness(&g, Some(1)), vec![0.0]);
    }
}
