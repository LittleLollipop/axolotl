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

    // ── 原生二进制持久化 ─────────────────────────

    const MAGIC: [u8; 4] = *b"AXEB";  // Axolotl EdgeBlock
    const FILE_VERSION: u16 = 1;

    /// 保存为原生 EdgeBlock 二进制格式
    ///
    /// 文件格式：
    /// ```
    /// Header (24 bytes):
    ///   magic: [u8; 4] = "AXEB"
    ///   version: u16 BE
    ///   block_capacity: u16 BE (always 32)
    ///   vertex_count: u32 BE
    ///   total_edges: u64 BE
    ///   reserved: [u8; 4]
    ///
    /// Sections (each: u32 BE len + bytes):
    ///   idx_to_id:     u64[]
    ///   vertices:      u32[]
    ///   block_counts:  u32[]
    ///   blocks:        u32[]
    ///   rev_vertices:  u32[]
    ///   rev_blk_cnts:  u32[]
    ///   rev_blocks:    u32[]
    ///   vertex_props:  encoded properties
    /// ```
    pub fn save(&self, file_path: &str) -> std::io::Result<()> {
        use std::io::Write;
        let mut buf: Vec<u8> = Vec::new();

        // Header
        buf.extend_from_slice(&Self::MAGIC);
        buf.extend_from_slice(&Self::FILE_VERSION.to_be_bytes());
        buf.extend_from_slice(&(Self::BLOCK_CAPACITY as u16).to_be_bytes());
        buf.extend_from_slice(&(self.vertex_count).to_be_bytes());
        buf.extend_from_slice(&self.total_edges.to_be_bytes());
        buf.extend_from_slice(&[0u8; 4]);

        // Helper: write u32 array
        fn write_u32_slice(buf: &mut Vec<u8>, data: &[u32]) {
            buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
            for &v in data {
                buf.extend_from_slice(&v.to_be_bytes());
            }
        }
        // Helper: write u64 array
        fn write_u64_slice(buf: &mut Vec<u8>, data: &[u64]) {
            buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
            for &v in data {
                buf.extend_from_slice(&v.to_be_bytes());
            }
        }

        write_u64_slice(&mut buf, &self.idx_to_id);
        write_u32_slice(&mut buf, &self.vertices);
        write_u32_slice(&mut buf, &self.block_counts);
        write_u32_slice(&mut buf, &self.blocks);
        write_u32_slice(&mut buf, &self.reverse_vertices);
        write_u32_slice(&mut buf, &self.reverse_block_counts);
        write_u32_slice(&mut buf, &self.reverse_blocks);

        // Vertex properties: count + per-vertex entries
        let prop_count = self.vertex_props.len() as u32;
        buf.extend_from_slice(&prop_count.to_be_bytes());
        for props in &self.vertex_props {
            let n = props.len() as u16;
            buf.extend_from_slice(&n.to_be_bytes());
            for (key, value) in props {
                let kb = key.as_bytes();
                buf.extend_from_slice(&(kb.len() as u16).to_be_bytes());
                buf.extend_from_slice(kb);
                Self::write_property_value(&mut buf, value);
            }
        }

        // 原子写入
        let tmp = format!("{}.tmp", file_path);
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&buf)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, file_path)?;
        Ok(())
    }

    /// 从原生 EdgeBlock 二进制文件加载
    pub fn open(file_path: &str) -> std::io::Result<Self> {
        use std::io::Read;
        let mut f = std::fs::File::open(file_path)?;

        // Header
        let mut magic = [0u8; 4];
        f.read_exact(&mut magic)?;
        if magic != Self::MAGIC {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                "Invalid AXEB magic bytes"));
        }

        let mut hdr = [0u8; 20];
        f.read_exact(&mut hdr)?;
        let _version = u16::from_be_bytes([hdr[0], hdr[1]]);
        let _cap = u16::from_be_bytes([hdr[2], hdr[3]]);
        let vertex_count = u32::from_be_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);
        let total_edges = u64::from_be_bytes([
            hdr[8], hdr[9], hdr[10], hdr[11],
            hdr[12], hdr[13], hdr[14], hdr[15],
        ]);

        // Helper: read u32 array
        fn read_u32_vec(f: &mut std::fs::File) -> std::io::Result<Vec<u32>> {
            let mut lb = [0u8; 4];
            f.read_exact(&mut lb)?;
            let len = u32::from_be_bytes(lb) as usize;
            let mut out = vec![0u32; len];
            for i in 0..len {
                let mut b = [0u8; 4];
                f.read_exact(&mut b)?;
                out[i] = u32::from_be_bytes(b);
            }
            Ok(out)
        }
        // Helper: read u64 array
        fn read_u64_vec(f: &mut std::fs::File) -> std::io::Result<Vec<u64>> {
            let mut lb = [0u8; 4];
            f.read_exact(&mut lb)?;
            let len = u32::from_be_bytes(lb) as usize;
            let mut out = vec![0u64; len];
            for i in 0..len {
                let mut b = [0u8; 8];
                f.read_exact(&mut b)?;
                out[i] = u64::from_be_bytes(b);
            }
            Ok(out)
        }

        let idx_to_id = read_u64_vec(&mut f)?;
        let vertices = read_u32_vec(&mut f)?;
        let block_counts = read_u32_vec(&mut f)?;
        let blocks = read_u32_vec(&mut f)?;
        let reverse_vertices = read_u32_vec(&mut f)?;
        let reverse_block_counts = read_u32_vec(&mut f)?;
        let reverse_blocks = read_u32_vec(&mut f)?;

        // ID mapping
        let id_to_idx: std::collections::HashMap<u64, usize> =
            idx_to_id.iter().enumerate().map(|(i, &id)| (id, i)).collect();

        // Properties
        let mut lb = [0u8; 4];
        f.read_exact(&mut lb)?;
        let prop_count = u32::from_be_bytes(lb) as usize;
        let mut vertex_props = Vec::with_capacity(prop_count);
        for _ in 0..prop_count {
            let mut kb = [0u8; 2];
            f.read_exact(&mut kb)?;
            let n = u16::from_be_bytes(kb) as usize;
            let mut props = std::collections::HashMap::new();
            for _ in 0..n {
                let mut lb2 = [0u8; 2];
                f.read_exact(&mut lb2)?;
                let klen = u16::from_be_bytes(lb2) as usize;
                let mut key = vec![0u8; klen];
                f.read_exact(&mut key)?;
                let key_str = String::from_utf8(key)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                let value = Self::read_property_value(&mut f)?;
                props.insert(key_str, value);
            }
            vertex_props.push(props);
        }

        Ok(GPUEdgeBlockGraph {
            vertices,
            block_counts,
            blocks,
            reverse_vertices,
            reverse_block_counts,
            reverse_blocks,
            vertex_count,
            block_capacity: Self::BLOCK_CAPACITY,
            idx_to_id,
            id_to_idx,
            vertex_props,
            total_edges,
            blocks_dirty: true,
            reverse_blocks_dirty: true,
            structure_dirty: true,
        })
    }

    // ── Property value encoding ──

    fn write_property_value(w: &mut Vec<u8>, pv: &crate::PropertyValue) {
        match pv {
            crate::PropertyValue::String(s) => {
                w.push(0); // type tag
                let b = s.as_bytes();
                w.extend_from_slice(&(b.len() as u32).to_be_bytes());
                w.extend_from_slice(b);
            }
            crate::PropertyValue::Int(i) => {
                w.push(1);
                w.extend_from_slice(&(*i as i64).to_be_bytes());
            }
            crate::PropertyValue::Double(d) => {
                w.push(2);
                w.extend_from_slice(&d.to_be_bytes());
            }
            crate::PropertyValue::Bool(b) => {
                w.push(3);
                w.push(*b as u8);
            }
            crate::PropertyValue::Null => {
                w.push(4);
            }
        }
    }

    fn read_property_value(f: &mut std::fs::File) -> std::io::Result<crate::PropertyValue> {
        use std::io::Read;
        let mut tag = [0u8; 1];
        f.read_exact(&mut tag)?;
        match tag[0] {
            0 => { // String
                let mut lb = [0u8; 4];
                f.read_exact(&mut lb)?;
                let len = u32::from_be_bytes(lb) as usize;
                let mut buf = vec![0u8; len];
                f.read_exact(&mut buf)?;
                let s = String::from_utf8(buf)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                Ok(crate::PropertyValue::String(s))
            }
            1 => { // Int
                let mut b = [0u8; 8];
                f.read_exact(&mut b)?;
                Ok(crate::PropertyValue::Int(i64::from_be_bytes(b)))
            }
            2 => { // Double
                let mut b = [0u8; 8];
                f.read_exact(&mut b)?;
                Ok(crate::PropertyValue::Double(f64::from_be_bytes(b)))
            }
            3 => { // Bool
                let mut b = [0u8; 1];
                f.read_exact(&mut b)?;
                Ok(crate::PropertyValue::Bool(b[0] != 0))
            }
            _ => Ok(crate::PropertyValue::Null),
        }
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

    #[test]
    fn test_save_load_roundtrip() {
        let path = "/tmp/test_edgeblock_native.bin";
        let _ = std::fs::remove_file(path);

        // 创建并保存
        let mut g = GPUEdgeBlockGraph::new();
        let mut a_props = HashMap::new();
        a_props.insert("name".to_string(), crate::PropertyValue::String("A".to_string()));
        a_props.insert("age".to_string(), crate::PropertyValue::Int(42));
        g.add_vertex(10, a_props);
        g.add_vertex(20, HashMap::new());
        g.add_edge(10, 20, 1.0);

        g.save(path).expect("save failed");

        // 加载
        let g2 = GPUEdgeBlockGraph::open(path).expect("open failed");

        assert_eq!(g2.vertex_count, 2);
        assert_eq!(g2.total_edges, 1);
        assert_eq!(g2.out_neighbors(10), vec![20]);

        let props = g2.get_vertex(10).unwrap();
        assert_eq!(props["name"], crate::PropertyValue::String("A".to_string()));
        assert_eq!(props["age"], crate::PropertyValue::Int(42));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_save_load_large() {
        let path = "/tmp/test_edgeblock_large.bin";
        let _ = std::fs::remove_file(path);

        let mut g = GPUEdgeBlockGraph::new();
        for i in 0..1000u64 {
            g.add_vertex(i, HashMap::new());
        }
        for i in 0..999 {
            g.add_edge(i, i + 1, 1.0);
        }

        g.save(path).expect("save failed");
        let g2 = GPUEdgeBlockGraph::open(path).expect("open failed");

        assert_eq!(g2.vertex_count, 1000);
        assert_eq!(g2.total_edges, 999);
        assert_eq!(g2.out_neighbors(0), vec![1]);
        assert_eq!(g2.out_neighbors(500), vec![501]);

        let _ = std::fs::remove_file(path);
    }
}
