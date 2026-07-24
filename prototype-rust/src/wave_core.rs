// wave_core.rs — CPU/GPU 协同的批量剥皮图密度分层
//
// Wave Core 是 K-Core 分解的推式变体:
// vs 传统: 逐顶点剥皮 (串行)
// vs Wave Core: 按轮批量剥, 轮内全并行 (GPU 友好)
//
// 算法:
//   1. 块扫描算出度 (O(E), 复用 PageRank 的开头)
//   2. 桶排序按度分组
//   3. 按轮剥: 每轮从当前最小度桶中取所有顶点, 遍历其出边块,
//      对每个邻居原子减度, 度下降的邻居坠入更低桶
//   4. 收敛: 顶点 core[v] = 剥皮时该顶点被剥那轮的 current_degree

use crate::gpu_edge_block::GPUEdgeBlockGraph;

/// 更新单个邻居的度（剥皮传播）
#[inline]
fn update_neighbor(nb: usize, n: usize, core_v_degree: u32, degree: &mut [u32], bins: &mut [Vec<usize>], max_deg: usize, peeled: &[bool]) {
    if nb >= n || peeled[nb] { return; }
    let old_d = degree[nb] as usize;
    if old_d as u32 <= core_v_degree { return; } // BZ rule: only decrement strictly higher degrees
    degree[nb] -= 1;
    let new_d = old_d - 1;
    if new_d <= max_deg { bins[new_d].push(nb); }
}

/// 对 EdgeBlock 图运行 Wave Core 分解
///
/// 返回 `core[v]`: 顶点 v 所属的最高 k-wave
/// core[v] = k 表示 v 属于"所有顶点度 ≥ k 的最大子图"
pub fn wave_core_blocks(eb: &GPUEdgeBlockGraph) -> Vec<u32> {
    let n = eb.vertex_count as usize;
    let block_stride = GPUEdgeBlockGraph::BLOCK_SIZE_U32;
    let blocks = &eb.blocks;
    let vertices = &eb.vertices;
    let block_counts = &eb.block_counts;

    // Step 1: Compute degrees (out-degree only; for undirected K-core,
    // caller should add edges in both directions)
    let mut degree = eb.compute_out_degrees();

    // Edge case: empty graph
    if n == 0 {
        return Vec::new();
    }

    // Step 2: Bin sort by degree
    let max_deg = *degree.iter().max().unwrap_or(&0) as usize;
    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); (max_deg + 1).max(2)];
    for v in 0..n {
        let d = degree[v] as usize;
        bins[d.min(max_deg)].push(v);
    }

    // Step 3: Wave peeling
    let mut core = vec![0u32; n];
    let mut peeled = vec![false; n];
    let mut peeled_count = 0usize;

    while peeled_count < n {
        // Always scan from lowest (may have been refilled by propagation)
        let mut found = false;
        for current_bin in 0..=max_deg {
            if bins[current_bin].is_empty() { continue; }

            // Drain entire bin — this is the "wave" batch
            let batch: Vec<usize> = std::mem::take(&mut bins[current_bin]);
            for &v in &batch {
                if peeled[v] { continue; }
                core[v] = current_bin as u32;
                let core_v_degree = current_bin as u32; // degree when v is peeled
                peeled[v] = true;
                peeled_count += 1;

                let first_block = vertices[v] as usize;
                let num_blocks = block_counts[v] as usize;
                for b in 0..num_blocks {
                    let base = (first_block + b) * block_stride;
                    let ec = blocks[base + 1] as usize;
                    for j in 0..ec.min(32) {
                        let nb = blocks[base + 2 + j] as usize;
                        update_neighbor(nb, n, core_v_degree, &mut degree, &mut bins, max_deg, &peeled);
                    }
                }
            }
            found = true;
            break; // Restart from bin 0 to catch cascade
        }
        if !found { break; }
    }

    // Step 4: Remaining unpeeled vertices → max possible core
    let remaining_core = max_deg as u32;
    for v in 0..n {
        if !peeled[v] {
            core[v] = remaining_core;
        }
    }

    core
}

/// 更新单个邻居的度（剥皮传播）
mod tests {
    use super::*;
    use crate::gpu_edge_block::GPUEdgeBlockGraph;
    use std::collections::HashMap;

    #[test]
    fn test_empty_graph() {
        let g = GPUEdgeBlockGraph::new();
        let core = wave_core_blocks(&g);
        assert!(core.is_empty());
    }

    #[test]
    fn test_single_vertex() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(0, HashMap::new());
        let core = wave_core_blocks(&g);
        assert_eq!(core, vec![0]);
    }

    #[test]
    fn test_triangle() {
        // 0–1, 1–2, 2–0 = triangle, each has degree 2 → all core=2
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(0, HashMap::new());
        g.add_vertex(1, HashMap::new());
        g.add_vertex(2, HashMap::new());
        g.add_edge(0, 1, 1.0); g.add_edge(1, 0, 1.0);
        g.add_edge(1, 2, 1.0); g.add_edge(2, 1, 1.0);
        g.add_edge(2, 0, 1.0); g.add_edge(0, 2, 1.0);
        let core = wave_core_blocks(&g);
        assert_eq!(core, vec![2, 2, 2], "triangle should be 2-core");
    }

    #[test]
    fn test_star() {
        // Undirected star: center=0 connects to leaves 1,2,3,4
        let mut g = GPUEdgeBlockGraph::new();
        for i in 0..5 { g.add_vertex(i, HashMap::new()); }
        for i in 1..5 {
            g.add_edge(0, i, 1.0);
            g.add_edge(i, 0, 1.0);
        }
        let core = wave_core_blocks(&g);
        assert_eq!(core, vec![1, 1, 1, 1, 1], "star is 1-core");
    }
}
