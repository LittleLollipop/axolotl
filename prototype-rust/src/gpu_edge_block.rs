// src/gpu_edge_block.rs
// GPU EdgeBlock 统一内存图存储
//
// EdgeBlock 的核心设计：
// ────────────────────
// 固定大小的 Block（34 个 u32/block = 136 bytes）直铺在内存中，
// CPU 和 GPU 共享同一份数据，Metal buffer 分配一次、复用到底。
//
// 正向 Block（出边）：
//   blocks[i]   = ownerVertex  (header)
//   blocks[i+1] = edgeCount     (header)
//   blocks[i+2..i+34] = 目标顶点（32 个槽位，不足填 0）
//
// 反向 Block（入边）：同样结构，用于 PageRank 等需要入边的算法。
//
// 新增边时直接追加到顶点最后一个 block 的尾部（O(1)）。
// 满 32 条边则创建新 block（O(1) 摊销）。

use std::collections::HashMap;
use crate::PropertyValue;

/// GPU EdgeBlock 统一内存图存储
///
/// 同时作为 CPU 的主存储和 GPU 的数据源。
/// 扁平化的 Vec<u32> 数组可以直接映射到 Metal buffer。
#[derive(Debug, Clone)]
pub struct GPUEdgeBlockGraph {
    // ── 正向 EdgeBlock（出边）──
    /// 每个顶点的第一个 EdgeBlock 索引
    pub vertices: Vec<u32>,
    /// 每个顶点的 EdgeBlock 数量
    pub block_counts: Vec<u32>,
    /// 扁平化 EdgeBlock 数据（出边）
    /// 每个 block = 34 个 u32: [ownerVertex, edgeCount, edge0..edge31]
    pub blocks: Vec<u32>,

    // ── 反向 EdgeBlock（入边）──
    pub reverse_vertices: Vec<u32>,
    pub reverse_block_counts: Vec<u32>,
    pub reverse_blocks: Vec<u32>,

    // ── 顶点元数据 ──
    pub vertex_count: u32,
    pub block_capacity: usize,

    /// 内部索引 → 外部用户 ID
    pub idx_to_id: Vec<u64>,
    /// 外部用户 ID → 内部索引
    pub id_to_idx: HashMap<u64, usize>,

    /// 顶点属性（内部索引 → 属性）
    pub vertex_props: Vec<HashMap<String, PropertyValue>>,

    // ── 边计数 ──
    pub total_edges: u64,

    // ── Dirty 标记（GPU 同步用）──
    pub blocks_dirty: bool,
    pub reverse_blocks_dirty: bool,
    pub structure_dirty: bool, // vertices/block_counts 发生变化
}

impl GPUEdgeBlockGraph {
    pub const BLOCK_CAPACITY: usize = 32;
    pub const BLOCK_SIZE_U32: usize = 2 + Self::BLOCK_CAPACITY; // = 34

    /// 创建空图
    pub fn new() -> Self {
        GPUEdgeBlockGraph {
            vertices: Vec::new(),
            block_counts: Vec::new(),
            blocks: Vec::new(),

            reverse_vertices: Vec::new(),
            reverse_block_counts: Vec::new(),
            reverse_blocks: Vec::new(),

            vertex_count: 0,
            block_capacity: Self::BLOCK_CAPACITY,

            idx_to_id: Vec::new(),
            id_to_idx: HashMap::new(),
            vertex_props: Vec::new(),

            total_edges: 0,

            blocks_dirty: true,
            reverse_blocks_dirty: true,
            structure_dirty: true,
        }
    }

    // ── ID 映射 ─────────────────────────

    /// 外部 ID → 内部索引。如果不存在则自动创建新顶点
    fn get_or_create_idx(&mut self, id: u64) -> usize {
        if let Some(&idx) = self.id_to_idx.get(&id) {
            return idx;
        }
        let idx = self.idx_to_id.len();
        self.idx_to_id.push(id);
        self.id_to_idx.insert(id, idx);

        // 扩展数组
        self.vertices.push(0);
        self.block_counts.push(0);
        self.reverse_vertices.push(0);
        self.reverse_block_counts.push(0);
        self.vertex_props.push(HashMap::new());

        self.vertex_count += 1;
        self.structure_dirty = true;
        idx
    }

    /// 根据 idx 获取外部 ID
    pub fn get_id(&self, idx: usize) -> Option<u64> {
        self.idx_to_id.get(idx).copied()
    }

    // ── 顶点操作 ─────────────────────────

    /// 添加顶点（显式提供 ID 和属性）
    pub fn add_vertex(
        &mut self,
        id: u64,
        properties: HashMap<String, PropertyValue>,
    ) -> usize {
        if let Some(&idx) = self.id_to_idx.get(&id) {
            // 顶点已存在，更新属性
            self.vertex_props[idx] = properties;
            return idx;
        }
        let idx = self.idx_to_id.len();
        self.idx_to_id.push(id);
        self.id_to_idx.insert(id, idx);

        self.vertices.push(0);
        self.block_counts.push(0);
        self.reverse_vertices.push(0);
        self.reverse_block_counts.push(0);
        self.vertex_props.push(properties);

        self.vertex_count += 1;
        self.structure_dirty = true;
        idx
    }

    /// 获取顶点属性
    pub fn get_vertex(&self, id: u64) -> Option<&HashMap<String, PropertyValue>> {
        self.id_to_idx.get(&id).map(|&idx| &self.vertex_props[idx])
    }

    /// 获取顶点属性（可变）
    pub fn get_vertex_mut(&mut self, id: u64) -> Option<&mut HashMap<String, PropertyValue>> {
        self.id_to_idx.get(&id).map(|&idx| &mut self.vertex_props[idx])
    }

    // ── 边操作 ─────────────────────────

    /// 添加边（from → to，带权重）
    ///
    /// 算法：
    /// 1. 找到 from 顶点最后一个正向 block
    /// 2. 如果 block 不满，直接追加（O(1)）
    /// 3. 否则创建新 block（O(1) 摊销）
    /// 4. 对 to 顶点的反向 block 做同样操作
    pub fn add_edge(&mut self, from: u64, to: u64, weight: f32) {
        let from_idx = self.get_or_create_idx(from);
        let to_idx = self.get_or_create_idx(to);
        let to_idx_u32 = to_idx as u32;

        // ── 正向 Block（出边）──
        self.append_to_block(
            from_idx,
            to_idx_u32,
            &mut false, // forward
        );

        // ── 反向 Block（入边）──
        self.append_to_block(
            to_idx,
            from_idx as u32,
            &mut true, // reverse
        );

        self.total_edges += 1;
    }

    /// 向顶点 v_idx 的最后一个 block 追加一条边
    fn append_to_block(&mut self, v_idx: usize, neighbor: u32, is_reverse: &mut bool) {
        let (vertices, block_counts, blocks) = if *is_reverse {
            (&mut self.reverse_vertices, &mut self.reverse_block_counts, &mut self.reverse_blocks)
        } else {
            (&mut self.vertices, &mut self.block_counts, &mut self.blocks)
        };

        let count = block_counts[v_idx];

        if count == 0 {
            // 第一个 block
            let new_block_idx = (blocks.len() / Self::BLOCK_SIZE_U32) as u32;
            vertices[v_idx] = new_block_idx;

            // 写入 block header + 第一条边
            blocks.push(v_idx as u32);        // ownerVertex
            blocks.push(1);                    // edgeCount
            blocks.push(neighbor);             // 第一条边
            // 填充剩余 31 个槽位
            for _ in 1..Self::BLOCK_CAPACITY {
                blocks.push(0);
            }

            block_counts[v_idx] = 1;
            if *is_reverse {
                self.reverse_blocks_dirty = true;
            } else {
                self.blocks_dirty = true;
            }
        } else {
            // 找到最后一个 block
            let first_block = vertices[v_idx] as usize;
            let last_block = first_block + count as usize - 1;
            let block_start = last_block * Self::BLOCK_SIZE_U32;
            let edge_count = blocks[block_start + 1] as usize;

            if edge_count < Self::BLOCK_CAPACITY {
                // Block 不满，追加
                blocks[block_start + 2 + edge_count] = neighbor;
                blocks[block_start + 1] = (edge_count + 1) as u32;
                if *is_reverse {
                    self.reverse_blocks_dirty = true;
                } else {
                    self.blocks_dirty = true;
                }
            } else {
                // Block 已满，创建新 block
                blocks.push(v_idx as u32);        // ownerVertex
                blocks.push(1);                    // edgeCount
                blocks.push(neighbor);             // 第一条边
                for _ in 1..Self::BLOCK_CAPACITY {
                    blocks.push(0);
                }
                block_counts[v_idx] += 1;
                self.structure_dirty = true;
                if *is_reverse {
                    self.reverse_blocks_dirty = true;
                } else {
                    self.blocks_dirty = true;
                }
            }
        }
    }

    /// 获取顶点的所有出边邻居（外部 ID）
    pub fn out_neighbors(&self, id: u64) -> Vec<u64> {
        if let Some(&idx) = self.id_to_idx.get(&id) {
            self.out_neighbors_by_idx(idx)
        } else {
            Vec::new()
        }
    }

    /// 获取顶点的所有出边邻居（内部索引）
    pub fn out_neighbors_by_idx(&self, v_idx: usize) -> Vec<u64> {
        let mut result = Vec::new();
        if v_idx >= self.block_counts.len() || self.block_counts[v_idx] == 0 {
            return result;
        }

        let first_block = self.vertices[v_idx] as usize;
        let count = self.block_counts[v_idx] as usize;

        for b in 0..count {
            let block_start = (first_block + b) * Self::BLOCK_SIZE_U32;
            let edge_count = self.blocks[block_start + 1] as usize;
            for e in 0..edge_count {
                let target_idx = self.blocks[block_start + 2 + e] as usize;
                if target_idx < self.idx_to_id.len() {
                    result.push(self.idx_to_id[target_idx]);
                }
            }
        }
        result
    }

    /// 获取顶点的所有入边邻居（外部 ID）
    pub fn in_neighbors(&self, id: u64) -> Vec<u64> {
        if let Some(&idx) = self.id_to_idx.get(&id) {
            self.in_neighbors_by_idx(idx)
        } else {
            Vec::new()
        }
    }

    /// 获取顶点的所有入边邻居（内部索引）
    pub fn in_neighbors_by_idx(&self, v_idx: usize) -> Vec<u64> {
        let mut result = Vec::new();
        if v_idx >= self.reverse_block_counts.len() || self.reverse_block_counts[v_idx] == 0 {
            return result;
        }

        let first_block = self.reverse_vertices[v_idx] as usize;
        let count = self.reverse_block_counts[v_idx] as usize;

        for b in 0..count {
            let block_start = (first_block + b) * Self::BLOCK_SIZE_U32;
            let edge_count = self.reverse_blocks[block_start + 1] as usize;
            for e in 0..edge_count {
                let src_idx = self.reverse_blocks[block_start + 2 + e] as usize;
                if src_idx < self.idx_to_id.len() {
                    result.push(self.idx_to_id[src_idx]);
                }
            }
        }
        result
    }

    /// 计算出度
    pub fn out_degree(&self, id: u64) -> usize {
        if let Some(&idx) = self.id_to_idx.get(&id) {
            self.out_degree_by_idx(idx)
        } else {
            0
        }
    }

    /// 计算出度（内部索引）
    pub fn out_degree_by_idx(&self, v_idx: usize) -> usize {
        if v_idx >= self.block_counts.len() || self.block_counts[v_idx] == 0 {
            return 0;
        }
        let first_block = self.vertices[v_idx] as usize;
        let count = self.block_counts[v_idx] as usize;
        let mut total = 0;
        for b in 0..count {
            let block_start = (first_block + b) * Self::BLOCK_SIZE_U32;
            total += self.blocks[block_start + 1] as usize;
        }
        total
    }

    /// 获取所有出度（用于 GPU 增量 PageRank）
    pub fn compute_out_degrees(&self) -> Vec<u32> {
        let mut out_degrees = vec![0u32; self.vertex_count as usize];
        for v in 0..self.vertex_count as usize {
            out_degrees[v] = self.out_degree_by_idx(v) as u32;
        }
        out_degrees
    }

    // ── 从 CSR 构建（兼容旧接口/从文件加载）──────

    /// 从 CSR 构建 EdgeBlock（用于从 binary 文件加载后转换）
    pub fn from_csr(
        csr_offsets: &[u32],
        csr_targets: &[u32],
        vertex_count: u32,
    ) -> Self {
        let vc = vertex_count as usize;

        // ===== 正向 EdgeBlock =====
        let mut vertices = vec![0u32; vc];
        let mut block_counts = vec![0u32; vc];
        let mut blocks = Vec::new();

        for v in 0..vc {
            let start = csr_offsets[v] as usize;
            let end = csr_offsets[v + 1] as usize;
            let degree = end - start;
            if degree == 0 { continue; }

            vertices[v] = (blocks.len() / Self::BLOCK_SIZE_U32) as u32;
            let mut offset = start;
            while offset < end {
                let count = usize::min(Self::BLOCK_CAPACITY, end - offset);
                blocks.push(v as u32); // ownerVertex
                blocks.push(count as u32); // edgeCount
                for j in 0..count {
                    blocks.push(csr_targets[offset + j]);
                }
                for _ in count..Self::BLOCK_CAPACITY {
                    blocks.push(0);
                }
                offset += count;
                block_counts[v] += 1;
            }
        }

        // ===== 反向 EdgeBlock =====
        let mut in_degrees = vec![0u32; vc];
        for v in 0..vc {
            for i in csr_offsets[v] as usize..csr_offsets[v + 1] as usize {
                in_degrees[csr_targets[i] as usize] += 1;
            }
        }
        let mut rev_offsets = vec![0u32; vc + 1];
        for v in 0..vc {
            rev_offsets[v + 1] = rev_offsets[v] + in_degrees[v];
        }
        let mut rev_targets = vec![0u32; rev_offsets[vc] as usize];
        let mut cur = vec![0u32; vc];
        for v in 0..vc {
            for i in csr_offsets[v] as usize..csr_offsets[v + 1] as usize {
                let t = csr_targets[i] as usize;
                let pos = (rev_offsets[t] + cur[t]) as usize;
                rev_targets[pos] = v as u32;
                cur[t] += 1;
            }
        }

        let mut reverse_vertices = vec![0u32; vc];
        let mut reverse_block_counts = vec![0u32; vc];
        let mut reverse_blocks = Vec::new();

        for v in 0..vc {
            let start = rev_offsets[v] as usize;
            let end = rev_offsets[v + 1] as usize;
            let degree = end - start;
            if degree == 0 { continue; }

            reverse_vertices[v] = (reverse_blocks.len() / Self::BLOCK_SIZE_U32) as u32;
            let mut offset = start;
            while offset < end {
                let count = usize::min(Self::BLOCK_CAPACITY, end - offset);
                reverse_blocks.push(v as u32);
                reverse_blocks.push(count as u32);
                for j in 0..count {
                    reverse_blocks.push(rev_targets[offset + j]);
                }
                for _ in count..Self::BLOCK_CAPACITY {
                    reverse_blocks.push(0);
                }
                offset += count;
                reverse_block_counts[v] += 1;
            }
        }

        let total_edges = (0..vc).map(|v| (csr_offsets[v + 1] - csr_offsets[v]) as u64).sum();

        // ID 映射（从文件加载时用连续 ID）
        let idx_to_id: Vec<u64> = (0..vc as u64).collect();
        let id_to_idx: HashMap<u64, usize> = (0..vc).map(|i| (i as u64, i)).collect();

        GPUEdgeBlockGraph {
            vertices,
            block_counts,
            blocks,
            reverse_vertices,
            reverse_block_counts,
            reverse_blocks,
            vertex_count: vc as u32,
            block_capacity: Self::BLOCK_CAPACITY,
            idx_to_id,
            id_to_idx,
            vertex_props: vec![HashMap::new(); vc],
            total_edges,
            blocks_dirty: true,
            reverse_blocks_dirty: true,
            structure_dirty: true,
        }
    }

    /// 统计信息
    pub fn stats(&self) -> (usize, usize, usize) {
        (
            self.vertex_count as usize,
            self.blocks.len() / Self::BLOCK_SIZE_U32 + self.reverse_blocks.len() / Self::BLOCK_SIZE_U32,
            self.total_edges as usize,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_add_vertex_and_edge() {
        let mut g = GPUEdgeBlockGraph::new();

        let a = g.add_vertex(100, HashMap::new());
        let b = g.add_vertex(200, HashMap::new());
        g.add_edge(100, 200, 1.0);

        assert_eq!(g.vertex_count, 2);
        assert_eq!(g.total_edges, 1);
        assert_eq!(g.out_neighbors(100), vec![200]);
        assert_eq!(g.in_neighbors(200), vec![100]);
    }

    #[test]
    fn test_many_edges_one_vertex() {
        let mut g = GPUEdgeBlockGraph::new();
        let center = g.add_vertex(1, HashMap::new());

        // 添加 64 条边（会跨越 2 个 block）
        for i in 0..64u64 {
            let id = 100 + i;
            g.add_vertex(id, HashMap::new());
            g.add_edge(1, id, 1.0);
        }

        let neighbors = g.out_neighbors(1);
        assert_eq!(neighbors.len(), 64);
        assert_eq!(g.total_edges, 64);

        // 每个反向邻居应该只有 1 条入边
        for i in 0..64u64 {
            let rev = g.in_neighbors(100 + i);
            assert_eq!(rev.len(), 1, "vertex {} should have 1 in-neighbor", 100 + i);
        }
    }

    #[test]
    fn test_out_degrees() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(1, HashMap::new());
        g.add_vertex(2, HashMap::new());
        g.add_vertex(3, HashMap::new());

        g.add_edge(1, 2, 1.0);
        g.add_edge(1, 3, 1.0);
        g.add_edge(2, 3, 1.0);

        let degs = g.compute_out_degrees();
        assert_eq!(degs[g.id_to_idx[&1]], 2);
        assert_eq!(degs[g.id_to_idx[&2]], 1);
        assert_eq!(degs[g.id_to_idx[&3]], 0);
    }
}
