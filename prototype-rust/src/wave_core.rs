// wave_core.rs — CPU/GPU 协同的批量剥皮图密度分层
//
// 基于 Batagelj-Zaversnik K-Core 算法，EdgeBlock 块扫描原生实现。
// 每轮从最低度桶批量取顶点，遍历 EdgeBlock 块传播度递减。

use crate::gpu_edge_block::GPUEdgeBlockGraph;

pub fn wave_core_blocks(eb: &GPUEdgeBlockGraph) -> Vec<u32> {
    let n = eb.vertex_count as usize;
    if n == 0 { return Vec::new(); }

    let block_stride = GPUEdgeBlockGraph::BLOCK_SIZE_U32;
    let blocks = &eb.blocks;
    let vertices = &eb.vertices;
    let block_counts = &eb.block_counts;

    let mut degree = eb.compute_out_degrees();
    let max_deg = *degree.iter().max().unwrap_or(&0) as usize;
    if max_deg == 0 { return vec![0u32; n]; }

    // Bin sort by degree
    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); max_deg + 1];
    for v in 0..n {
        bins[degree[v] as usize].push(v);
    }

    let mut core = vec![0u32; n];
    let mut peeled = vec![false; n];
    let mut peeled_count = 0usize;

    // Wave peeling
    while peeled_count < n {
        let mut found = false;
        for current_bin in 0..=max_deg {
            if bins[current_bin].is_empty() { continue; }

            let batch = std::mem::take(&mut bins[current_bin]);
            for &v in &batch {
                if peeled[v] { continue; }
                core[v] = current_bin as u32;
                peeled[v] = true;
                peeled_count += 1;

                // Propagate: only decrement neighbors with degree > core[v]
                let first_block = vertices[v] as usize;
                let num_blocks = block_counts[v] as usize;
                for b in 0..num_blocks {
                    let base = (first_block + b) * block_stride;
                    let ec = blocks[base + 1] as usize;
                    for j in 0..ec.min(32) {
                        let nb = blocks[base + 2 + j] as usize;
                        if nb >= n || peeled[nb] { continue; }
                        let od = degree[nb] as usize;
                        if od <= current_bin { continue; }
                        degree[nb] -= 1;
                        bins[od - 1].push(nb);
                    }
                }
            }
            found = true;
            break;
        }
        if !found { break; }
    }

    let remaining = max_deg as u32;
    for v in 0..n {
        if !peeled[v] { core[v] = remaining; }
    }
    core
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_edge_block::GPUEdgeBlockGraph;
    use std::collections::HashMap;

    #[test]
    fn test_empty_graph() { assert!(wave_core_blocks(&GPUEdgeBlockGraph::new()).is_empty()); }

    #[test]
    fn test_single_vertex() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(0, HashMap::new());
        assert_eq!(wave_core_blocks(&g), vec![0]);
    }

    #[test]
    fn test_triangle() {
        let mut g = GPUEdgeBlockGraph::new();
        for i in 0..3 { g.add_vertex(i, HashMap::new()); }
        g.add_edge(0, 1, 1.0); g.add_edge(1, 0, 1.0);
        g.add_edge(1, 2, 1.0); g.add_edge(2, 1, 1.0);
        g.add_edge(2, 0, 1.0); g.add_edge(0, 2, 1.0);
        assert_eq!(wave_core_blocks(&g), vec![2, 2, 2]);
    }

    #[test]
    fn test_star() {
        let mut g = GPUEdgeBlockGraph::new();
        for i in 0..5 { g.add_vertex(i, HashMap::new()); }
        for i in 1..5 { g.add_edge(0, i, 1.0); g.add_edge(i, 0, 1.0); }
        assert_eq!(wave_core_blocks(&g), vec![1, 1, 1, 1, 1]);
    }
}
