// scc.rs — Tarjan 强连通分量 (EdgeBlock 块扫描原生实现)
//
// Tarjan 算法: 单次 DFS, O(V+E)
// - index[v]: 发现时间戳
// - lowlink[v]: 可追溯到的最早祖先
// - 当 lowlink[v] == index[v], v 是 SCC 根 → 弹出栈顶到 v 的所有顶点 = 一个 SCC

use crate::gpu_edge_block::GPUEdgeBlockGraph;

pub fn tarjan_scc(eb: &GPUEdgeBlockGraph) -> Vec<Vec<usize>> {
    let n = eb.vertex_count as usize;
    if n == 0 { return Vec::new(); }

    let bs = GPUEdgeBlockGraph::BLOCK_SIZE_U32;
    let blocks = &eb.blocks;
    let vertices = &eb.vertices;
    let block_counts = &eb.block_counts;

    let mut index = vec![0u32; n];
    let mut lowlink = vec![0u32; n];
    let mut on_stack = vec![false; n];
    let mut scc_stack: Vec<usize> = Vec::new();
    let mut result: Vec<Vec<usize>> = Vec::new();
    let mut timer = 1u32;
    let mut dfs_stack: Vec<(usize, usize)> = Vec::new(); // (vertex, next_neighbor_pos)

    for start in 0..n {
        if index[start] != 0 { continue; }

        // Push start vertex with neighbor position = 0
        dfs_stack.push((start, 0));
        while let Some(&(v, mut pos)) = dfs_stack.last() {
            if pos == 0 {
                // First time visiting v
                index[v] = timer;
                lowlink[v] = timer;
                timer += 1;
                scc_stack.push(v);
                on_stack[v] = true;
                pos = 0;
            }

            // Advance through neighbors via EdgeBlock blocks
            let first_block = vertices[v] as usize;
            let num_blocks = block_counts[v] as usize;
            let mut advanced = false;

            while pos < num_blocks * 32 {
                let block_idx = pos / 32;
                let sub_idx = pos % 32;
                if block_idx < num_blocks {
                    let base = (first_block + block_idx) * bs;
                    let ec = blocks[base + 1] as usize;
                    if sub_idx < ec.min(32) {
                        let w = blocks[base + 2 + sub_idx] as usize;
                        if w < n && w != v {
                            if index[w] == 0 {
                                // Tree edge
                                dfs_stack.last_mut().unwrap().1 = pos + 1;
                                dfs_stack.push((w, 0));
                                advanced = true;
                                break;
                            } else if on_stack[w] {
                                // Back edge
                                lowlink[v] = lowlink[v].min(index[w]);
                            }
                        }
                    }
                }
                pos += 1;
            }

            if advanced { continue; }

            // All neighbors processed — backtrack
            dfs_stack.pop();
            if let Some(&(parent, _)) = dfs_stack.last() {
                lowlink[parent] = lowlink[parent].min(lowlink[v]);
            }

            // Root check
            if lowlink[v] == index[v] {
                let mut scc = Vec::new();
                loop {
                    let w = scc_stack.pop().unwrap();
                    on_stack[w] = false;
                    scc.push(w);
                    if w == v { break; }
                }
                result.push(scc);
            }
        }
    }

    result
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

    fn sort_sccs(mut sccs: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
        for s in &mut sccs { s.sort(); }
        sccs.sort_by_key(|s| s[0]);
        sccs
    }

    #[test]
    fn test_single_vertex() {
        let g = build_graph(&[]);
        let sccs = sort_sccs(tarjan_scc(&g));
        assert_eq!(sccs, vec![vec![0]]);
    }

    #[test]
    fn test_two_cycle() {
        // 0→1, 1→0 = one SCC
        let g = build_graph(&[(0,1), (1,0)]);
        let sccs = sort_sccs(tarjan_scc(&g));
        assert_eq!(sccs, vec![vec![0, 1]]);
    }

    #[test]
    fn test_three_cycle() {
        // 0→1→2→0 = one SCC
        let g = build_graph(&[(0,1), (1,2), (2,0)]);
        let sccs = sort_sccs(tarjan_scc(&g));
        assert_eq!(sccs, vec![vec![0, 1, 2]]);
    }

    #[test]
    fn test_dag() {
        // 0→1, 1→2, 0→2 (DAG, 3 singletons)
        let g = build_graph(&[(0,1), (1,2), (0,2)]);
        let sccs = sort_sccs(tarjan_scc(&g));
        assert_eq!(sccs, vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn test_two_components() {
        // 0→1→0 (SCC), 2→3→2 (SCC), no connection between
        let g = build_graph(&[(0,1), (1,0), (2,3), (3,2)]);
        let sccs = sort_sccs(tarjan_scc(&g));
        assert_eq!(sccs, vec![vec![0, 1], vec![2, 3]]);
    }

    #[test]
    fn test_bridge() {
        // 0→1→2→0 (SCC) → 3 → 4 → 3 (SCC), connected by 2→3
        let g = build_graph(&[(0,1), (1,2), (2,0), (2,3), (3,4), (4,3)]);
        let sccs = sort_sccs(tarjan_scc(&g));
        assert_eq!(sccs, vec![vec![0, 1, 2], vec![3, 4]]);
    }
}
