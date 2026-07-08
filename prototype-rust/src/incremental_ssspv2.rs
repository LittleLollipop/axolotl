// src/incremental_ssspv2.rs
// 差分 BFS：适用于无权重图的增量 SSSP
//
// 核心洞察：新增边只会缩短距离（不会增加）。
// 只需从距离实际变短的顶点传播 BFS，不扫描全图。

use std::collections::VecDeque;
use crate::csr_graph::CSRGraph;

/// 差分-BFS：增量更新已知的 SSSP 距离数组。
///
/// # 参数
/// - `csr`: 已包含新增边的 CSR 图
/// - `distances`: 原始图上的完整 SSSP 距离数组（会被原地修改）
/// - `new_edges`: 新增的有向边 `[(src, dst)]`
///
/// # 返回
/// 距离被缩短的顶点数。distances 数组已被原地更新为包含新边后的结果。
pub fn incremental_sssp_diff_bfs(
    csr: &CSRGraph,
    distances: &mut [f32],
    new_edges: &[(u32, u32)],
) -> usize {
    let n = csr.vertex_count as usize;

    // ── Step 1: 只检查新边是否创造更短路径 ──
    let mut queue = VecDeque::new();
    let mut updated = 0;

    for &(u, v) in new_edges {
        let uu = u as usize;
        let vv = v as usize;
        if uu >= n || vv >= n {
            continue;
        }
        // u → v 可以缩短到 v 的距离吗？
        if distances[uu] + 1.0 < distances[vv] {
            distances[vv] = distances[uu] + 1.0;
            queue.push_back(v);
            updated += 1;
        }
        // v → u？（如果图是无向的，同样检查反向）
        if distances[vv] + 1.0 < distances[uu] {
            distances[uu] = distances[vv] + 1.0;
            queue.push_back(u);
            updated += 1;
        }
    }

    // ── Step 2: BFS 传播（类似 Bellman-Ford，但无权图只需要一遍） ──
    while let Some(w) = queue.pop_front() {
        let ww = w as usize;
        let dw = distances[ww];

        // 遍历 w 的所有出边邻居
        let start = csr.offsets[ww] as usize;
        let end = csr.offsets[ww + 1] as usize;
        for idx in start..end {
            let nb = csr.targets[idx] as usize;
            if nb < n && dw + 1.0 < distances[nb] {
                distances[nb] = dw + 1.0;
                queue.push_back(nb as u32);
                updated += 1;
            }
        }
    }

    updated
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn build_csr(n: usize, edges: &[(u64, u64)]) -> CSRGraph {
        let mut csr = CSRGraph::new();
        for i in 0..n as u64 {
            csr.add_vertex(i, HashMap::new());
        }
        csr.build_csr(edges);
        csr
    }

    fn full_bfs(csr: &CSRGraph, source: u32) -> Vec<f32> {
        let n = csr.vertex_count as usize;
        let mut dist = vec![f32::INFINITY; n];
        let mut q = VecDeque::new();
        dist[source as usize] = 0.0;
        q.push_back(source);
        while let Some(u) = q.pop_front() {
            let du = dist[u as usize];
            let start = csr.offsets[u as usize] as usize;
            let end = csr.offsets[u as usize + 1] as usize;
            for idx in start..end {
                let v = csr.targets[idx] as usize;
                if dist[v] == f32::INFINITY {
                    dist[v] = du + 1.0;
                    q.push_back(v as u32);
                }
            }
        }
        dist
    }

    #[test]
    fn test_basic_shortcut() {
        let edges: Vec<(u64, u64)> = vec![
            (0, 1), (1, 2), (2, 3), (3, 4),  // path: 0→1→2→3→4
        ];
        let csr = build_csr(5, &edges);
        let mut dist = vec![f32::INFINITY; 5];
        dist[0] = 0.0;

        // Full BFS gives [0, 1, 2, 3, 4]
        let full = full_bfs(&csr, 0);
        assert_eq!(full[4], 4.0);

        // Add shortcut 0→3 — distance to 3 should drop from 3 to 1
        // and 4 should drop from 4 to 2
        let mut edges_new = edges.clone();
        edges_new.push((0, 3));
        let csr_new = build_csr(5, &edges_new);

        let mut dist_copy = full.clone();
        let n_updated = incremental_sssp_diff_bfs(&csr_new, &mut dist_copy, &[(0, 3)]);

        assert_eq!(dist_copy[3], 1.0); // 3 被缩短
        assert_eq!(dist_copy[4], 2.0); // 4 级联缩短
        assert!(n_updated >= 2);
    }

    #[test]
    fn test_new_edge_no_effect() {
        let edges: Vec<(u64, u64)> = vec![(0, 1), (1, 2), (2, 3)];
        let csr = build_csr(4, &edges);
        let mut dist = full_bfs(&csr, 0);
        let n = incremental_sssp_diff_bfs(&csr, &mut dist, &[(2, 1)]); // backward edge
        assert_eq!(n, 0); // already has a shorter path
    }

    #[test]
    fn test_larger_graph() {
        // 50K 规模冒烟测试
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let n = 1000;
        let mut edges = std::collections::HashSet::new();
        while edges.len() < 5000 {
            let u = rng.gen_range(0..n);
            let v = rng.gen_range(0..n);
            if u != v {
                edges.insert((u as u64, v as u64));
            }
        }
        let edge_list: Vec<_> = edges.into_iter().collect();
        let csr = build_csr(n, &edge_list);
        let dist_full = full_bfs(&csr, 0);

        // 添加 10 条随机新边，做增量
        let mut new_edges = Vec::new();
        for _ in 0..10 {
            let u = rng.gen_range(0..n);
            let v = rng.gen_range(0..n);
            if u != v { new_edges.push((u as u32, v as u32)); }
        }
        let new_edges_u64: Vec<_> = new_edges.iter().map(|&(u,v)| (u as u64, v as u64)).collect();
        let mut all_edges = edge_list.clone();
        all_edges.extend(&new_edges_u64);
        let csr_new = build_csr(n, &all_edges);
        let dist_full_new = full_bfs(&csr_new, 0);

        // 增量
        let mut dist_copy = dist_full.clone();
        let _n = incremental_sssp_diff_bfs(&csr_new, &mut dist_copy, &new_edges);

        // 验证正确性
        for i in 0..n {
            let a = dist_copy[i];
            let b = dist_full_new[i];
            if a.is_infinite() && b.is_infinite() { continue; }
            assert!((a - b).abs() < 1e-4,
                "vertex {}: expected {}, got {}", i, b, a);
        }
    }
}
