// src/mmap_graph.rs
// 真正的 mmap 实现（零拷贝，支持大于内存的图）
//
// 设计原则：
// - 使用 memmap2::Mmap 将整个文件映射到虚拟内存
// - 索引区域直接通过指针访问（unsafe，但安全因为格式可控）
// - 数据区域通过 &[u8] 切片访问（安全）
// - 支持大于内存的图（OS 自动处理 paging）
// - Arc<Mmap> 支持跨线程共享
//
// 文件格式（小端字节序，便携式）：
//
// ┌─────────────────────────────────────────┐
// │ Header (64 bytes)                     │
// │  magic: [u8; 4] = b"AXOM"        │
// │  version: u16 = 1                   │
// │  vertex_count: u64                   │
// │  edge_count: u64                     │
// │  vertex_index_offset: u64             │
// │  vertex_data_offset: u64              │
// │  edge_index_offset: u64               │
// │  edge_data_offset: u64                │
// │  reserved: [u8; 16]                │
// ├─────────────────────────────────────────┤
// │ Vertex Index (24 bytes/entry, 排序) │
// │  [VertexIdx; vertex_count]           │
// │    id: u64                           │
// │    data_offset: u64                   │
// │    data_len: u64                     │
// ├─────────────────────────────────────────┤
// │ Vertex Data (变长)                    │
// │  [u8]  properties 编码字节          │
// ├─────────────────────────────────────────┤
// │ Edge Index (40 bytes/entry, 排序)   │
// │  [EdgeIdx; edge_count]               │
// │    from: u64                         │
// │    to: u64                           │
// │    weight: f64                       │
// │    data_offset: u64                   │
// │    data_len: u64                     │
// ├─────────────────────────────────────────┤
// │ Edge Data (变长)                      │
// │  [u8]  properties 编码字节          │
// └─────────────────────────────────────────┘

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;

use crate::PropertyValue;

// ── 文件格式常量 ─────────────────────────

const MAGIC: &[u8; 4] = b"AXOM";
const VERSION: u16 = 1;

const HEADER_SIZE: usize = 64;
const VERTEX_IDX_SIZE: usize = 24; // u64 + u64 + u64
const EDGE_IDX_SIZE: usize   = 40; // u64 + u64 + f64 + u64 + u64

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct VertexIdx {
    id:          u64,
    data_offset:  u64,
    data_len:     u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct EdgeIdx {
    from:        u64,
    to:          u64,
    weight:      f64,
    data_offset:  u64,
    data_len:     u64,
}

// ── 读写辅助 ─────────────────────────

fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn read_u64(buf: &[u8], off: usize) -> u64 {
    let s = &buf[off..off + 8];
    u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
}
fn read_f64(buf: &[u8], off: usize) -> f64 {
    let s = &buf[off..off + 8];
    f64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
}

// ── 属性编解码（与 compact_storage.rs 格式兼容） ─────────────────────────

fn encode_properties(props: &HashMap<String, PropertyValue>) -> Vec<u8> {
    let mut buf = Vec::new();
    let len = props.len() as u32;
    buf.extend_from_slice(&len.to_le_bytes());
    let mut items: Vec<_> = props.iter().collect();
    items.sort_by_key(|(k, _)| *k);
    for (key, value) in items {
        buf.extend_from_slice(&(key.len() as u32).to_le_bytes());
        buf.extend_from_slice(key.as_bytes());
        match value {
            PropertyValue::String(s) => {
                buf.push(0);
                buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
                buf.extend_from_slice(s.as_bytes());
            }
            PropertyValue::Int(i) => {
                buf.push(1);
                buf.extend_from_slice(&(*i as u64).to_le_bytes());
            }
            PropertyValue::Double(d) => {
                buf.push(2);
                buf.extend_from_slice(&d.to_le_bytes());
            }
            PropertyValue::Bool(b) => {
                buf.push(3);
                buf.push(*b as u8);
            }
            PropertyValue::Null => {
                buf.push(4);
            }
        }
    }
    buf
}

fn decode_properties(buf: &[u8]) -> HashMap<String, PropertyValue> {
    if buf.len() < 4 { return HashMap::new(); }
    let mut cursor = 0;
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    cursor = 4;
    let mut props = HashMap::new();
    for _ in 0..len {
        if cursor + 4 > buf.len() { break; }
        let klen = u32::from_le_bytes([buf[cursor], buf[cursor+1], buf[cursor+2], buf[cursor+3]]) as usize;
        cursor += 4;
        if cursor + klen > buf.len() { break; }
        let key = String::from_utf8(buf[cursor..cursor+klen].to_vec()).unwrap_or_default();
        cursor += klen;
        if cursor >= buf.len() { break; }
        let type_tag = buf[cursor];
        cursor += 1;
        let value = match type_tag {
            0 => {
                if cursor + 4 > buf.len() { break; }
                let slen = u32::from_le_bytes([buf[cursor], buf[cursor+1], buf[cursor+2], buf[cursor+3]]) as usize;
                cursor += 4;
                if cursor + slen > buf.len() { break; }
                let s = String::from_utf8(buf[cursor..cursor+slen].to_vec()).unwrap_or_default();
                cursor += slen;
                PropertyValue::String(s)
            }
            1 => {
                if cursor + 8 > buf.len() { break; }
                let i = i64::from_le_bytes([buf[cursor], buf[cursor+1], buf[cursor+2], buf[cursor+3],
                                             buf[cursor+4], buf[cursor+5], buf[cursor+6], buf[cursor+7]]);
                cursor += 8;
                PropertyValue::Int(i)
            }
            2 => {
                if cursor + 8 > buf.len() { break; }
                let d = f64::from_le_bytes([buf[cursor], buf[cursor+1], buf[cursor+2], buf[cursor+3],
                                            buf[cursor+4], buf[cursor+5], buf[cursor+6], buf[cursor+7]]);
                cursor += 8;
                PropertyValue::Double(d)
            }
            3 => {
                if cursor >= buf.len() { break; }
                let b = buf[cursor] != 0;
                cursor += 1;
                PropertyValue::Bool(b)
            }
            4 => PropertyValue::Null,
            _ => { cursor += 1; continue; }
        };
        props.insert(key, value);
    }
    props
}

// ── MmapGraph（真正的 mmap 只读访问） ─────────────────────────

/// 真正的 mmap 图访问（只读，零拷贝）
///
/// 用途：
/// - 图大于内存时，用 mmap 访问（OS 自动 paging）
/// - 多线程并发读（Arc<Mmap> 天然支持）
/// - 直接访问文件内存，无额外拷贝
///
/// 限制：
/// - 只读（要写入请用 PersistentGraph，然后调用 convert_from 生成 mmap 文件）
/// - 小端字节序（Apple Silicon / x86 均支持）
pub struct MmapGraph {
    mmap: Arc<Mmap>,

    vertex_count: usize,
    edge_count:   usize,

    vertex_index_offset: usize,
    vertex_data_offset:  usize,
    edge_index_offset:   usize,
    edge_data_offset:    usize,
}

impl MmapGraph {
    /// 从文件打开 mmap
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path.as_ref())?;
        let mmap = unsafe { Mmap::map(&file)? };
        let mmap_arc = Arc::new(mmap);

        // 解析 Header（前 64 字节）
        if mmap_arc.len() < HEADER_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file too small for header"));
        }
        let h = &mmap_arc[..HEADER_SIZE];

        if &h[0..4] != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
        }
        let version = u16::from_le_bytes([h[4], h[5]]);
        if version != VERSION {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "unsupported version"));
        }

        let vertex_count       = read_u64(h, 8)  as usize;
        let edge_count         = read_u64(h, 16) as usize;
        let vertex_index_offset = read_u64(h, 24) as usize;
        let vertex_data_offset  = read_u64(h, 32) as usize;
        let edge_index_offset   = read_u64(h, 40) as usize;
        let edge_data_offset    = read_u64(h, 48) as usize;

        //  sanity：文件大小至少能容纳所有索引区域
        // （数据区域的大小无法直接从 header 得知，在访问时逐条检查）
        let min_file_size = edge_data_offset;  // 至少是边数据区域的起始位置
        if mmap_arc.len() < min_file_size {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file too small for edge data offset"));
        }

        Ok(MmapGraph {
            mmap: mmap_arc,
            vertex_count,
            edge_count,
            vertex_index_offset,
            vertex_data_offset,
            edge_index_offset,
            edge_data_offset,
        })
    }

    /// 顶点数量
    pub fn vertex_count(&self) -> usize { self.vertex_count }

    /// 边数量
    pub fn edge_count(&self) -> usize { self.edge_count }

    // ── 索引解析（从 mmap 区域直接读字节） ──

    fn vertex_idx(&self, i: usize) -> (u64, u64, u64) {
        let off = self.vertex_index_offset + i * VERTEX_IDX_SIZE;
        let b = &self.mmap[off..off + VERTEX_IDX_SIZE];
        (
            read_u64(b, 0),
            read_u64(b, 8),
            read_u64(b, 16),
        )
    }

    fn edge_idx(&self, i: usize) -> (u64, u64, f64, u64, u64) {
        let off = self.edge_index_offset + i * EDGE_IDX_SIZE;
        let b = &self.mmap[off..off + EDGE_IDX_SIZE];
        (
            read_u64(b, 0),
            read_u64(b, 8),
            read_f64(b, 16),
            read_u64(b, 24),
            read_u64(b, 32),
        )
    }

    // ── 查询 API ──

    /// 按键查询顶点（O(log n)，二分查找 mmap 索引）
    pub fn get_vertex(&self, id: u64) -> Option<HashMap<String, PropertyValue>> {
        // 在 vertex index 中二分查找
        let mut lo = 0usize;
        let mut hi = self.vertex_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let (mid_id, _, _) = self.vertex_idx(mid);
            match mid_id.cmp(&id) {
                std::cmp::Ordering::Less    => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal   => {
                    let (_, data_offset, data_len) = self.vertex_idx(mid);
                    let start = data_offset as usize;
                    let end   = start + data_len as usize;
                    let props_bytes = &self.mmap[start..end];
                    return Some(decode_properties(props_bytes));
                }
            }
        }
        None
    }

    /// 按 (from, to) 查询边（O(log e)，二分查找 mmap 索引）
    pub fn get_edge(&self, from: u64, to: u64) -> Option<(f64, HashMap<String, PropertyValue>)> {
        let mut lo = 0usize;
        let mut hi = self.edge_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let (mid_from, mid_to, weight, data_offset, data_len) = self.edge_idx(mid);
            match (mid_from.cmp(&from), mid_to.cmp(&to)) {
                (std::cmp::Ordering::Less, _) if mid_from < from => lo = mid + 1,
                (_, std::cmp::Ordering::Less) if mid_to < to  => lo = mid + 1,
                (std::cmp::Ordering::Greater, _) => hi = mid,
                (_, std::cmp::Ordering::Greater) => hi = mid,
                _ => {
                    let start = data_offset as usize;
                    let end   = start + data_len as usize;
                    let props_bytes = &self.mmap[start..end];
                    return Some((weight, decode_properties(props_bytes)));
                }
            }
        }
        None
    }

    /// 查顶点的出边邻居 ID 列表（需要扫描边索引）
    pub fn out_neighbors(&self, id: u64) -> Vec<u64> {
        // 边索引按 (from, to) 排序，所以同一 from 的边是连续的
        // 用二分找到 from == id 的区间
        let mut lo = 0usize;
        let mut hi = self.edge_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let (mid_from, _, _, _, _) = self.edge_idx(mid);
            if mid_from < id {
                lo = mid + 1;
            } else if mid_from > id {
                hi = mid;
            } else {
                // 找到第一个 from == id 的位置
                lo = mid;
                break;
            }
        }
        // 现在 lo 是第一个 from == id 的位置（可能）
        // 扫描所有 from == id 的边
        let mut result = Vec::new();
        for i in lo..self.edge_count {
            let (from, to, _, _, _) = self.edge_idx(i);
            if from == id {
                result.push(to);
            } else if from > id {
                break;
            }
        }
        result
    }

    /// 迭代所有顶点（O(n)，直接从 mmap 读）
    pub fn iter_vertices(&self) -> MmapVertexIter {
        MmapVertexIter {
            graph: self,
            idx: 0,
        }
    }

    /// 迭代所有边（O(e)，直接从 mmap 读）
    pub fn iter_edges(&self) -> MmapEdgeIter {
        MmapEdgeIter {
            graph: self,
            idx: 0,
        }
    }

    /// 将 mmap 图转换为 CSR（供 GPU 算法使用）
    /// 注意：此操作会将整个图的结构部分加载到内存，但属性数据仍可 mmap
    pub fn to_csr(&self) -> crate::CSRGraph {
        let mut csr = crate::CSRGraph::new();

        // 添加顶点（只记录 ID 和属性）
        for i in 0..self.vertex_count {
            let (id, data_offset, data_len) = self.vertex_idx(i);
            let start = data_offset as usize;
            let end   = start + data_len as usize;
            // 防御性检查
            if end > self.mmap.len() {
                continue;
            }
            let props_bytes = &self.mmap[start..end];
            let props = decode_properties(props_bytes);
            csr.add_vertex(id, props);
        }

        // 添加边
        let mut edges: Vec<(u64, u64)> = Vec::with_capacity(self.edge_count);
        for i in 0..self.edge_count {
            let (from, to, weight, _, _) = self.edge_idx(i);
            edges.push((from, to));
            // 记录权重（通过 csr.weights 设置，在 build_csr 之后）
        }

        csr.build_csr(&edges);

        // 设置权重
        let mut wi = 0;
        for i in 0..self.edge_count {
            let (_, _, weight, _, _) = self.edge_idx(i);
            if wi < csr.weights.len() {
                csr.weights[wi] = weight as f32;
                wi += 1;
            }
        }

        csr
    }
}

// ── 迭代器 ─────────────────────────

pub struct MmapVertexIter<'a> {
    graph: &'a MmapGraph,
    idx:   usize,
}

impl<'a> Iterator for MmapVertexIter<'a> {
    type Item = (u64, HashMap<String, PropertyValue>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.graph.vertex_count {
            return None;
        }
        let (id, data_offset, data_len) = self.graph.vertex_idx(self.idx);
        let start = data_offset as usize;
        let end   = start + data_len as usize;
        let props = decode_properties(&self.graph.mmap[start..end]);
        self.idx += 1;
        Some((id, props))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.graph.vertex_count - self.idx;
        (remaining, Some(remaining))
    }
}

pub struct MmapEdgeIter<'a> {
    graph: &'a MmapGraph,
    idx:   usize,
}

impl<'a> Iterator for MmapEdgeIter<'a> {
    type Item = (u64, u64, f64, HashMap<String, PropertyValue>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.graph.edge_count {
            return None;
        }
        let (from, to, weight, data_offset, data_len) = self.graph.edge_idx(self.idx);
        let start = data_offset as usize;
        let end   = start + data_len as usize;
        let props = decode_properties(&self.graph.mmap[start..end]);
        self.idx += 1;
        Some((from, to, weight, props))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.graph.edge_count - self.idx;
        (remaining, Some(remaining))
    }
}

// ── 转换：PersistentGraph → mmap 文件 ─────────────────────────

impl MmapGraph {
    /// 将 PersistentGraph 转换为 mmap 友好格式文件
    ///
    /// 用途：
    /// 1. 用 PersistentGraph 正常读写
    /// 2. 数据准备好后，调用此函数生成 mmap 文件
    /// 3. 用 MmapGraph::open() 以 mmap 方式只读访问（支持大于内存）
    pub fn convert_from(
        src:       &crate::PersistentGraph,
        mmap_path:  &str,
    ) -> io::Result<()> {
        let mut file = File::create(mmap_path)?;

        let vertex_count = src.vertices.len();
        let edge_count   = src.edges.len();

        // ── 第 1 步：收集并排序顶点/边（保证索引有序） ──

        let mut sorted_vertices: Vec<u64> = src.vertices.keys().copied().collect();
        sorted_vertices.sort_unstable();

        let mut sorted_edges: Vec<(u64, u64)> = src.edges.keys().copied().collect();
        sorted_edges.sort_unstable();

        // ── 第 2 步：编码所有顶点的属性（计算偏移量） ──

        let mut vertex_data_bytes: Vec<u8> = Vec::new();
        let mut vertex_index_entries: Vec<(u64, u64, u64)> = Vec::with_capacity(vertex_count);

        for id in &sorted_vertices {
            let vr = &src.vertices[id];
            let data_offset = (VERTEX_IDX_SIZE as u64) * (vertex_count as u64)
                           + (HEADER_SIZE as u64)
                           + (vertex_data_bytes.len() as u64);
            let encoded = encode_properties(&vr.properties);
            let data_len = encoded.len() as u64;
            vertex_index_entries.push((*id, data_offset, data_len));
            vertex_data_bytes.extend_from_slice(&encoded);
        }

        // ── 第 3 步：编码所有边的属性和权重 ──

        let mut edge_data_bytes: Vec<u8> = Vec::new();
        let mut edge_index_entries: Vec<(u64, u64, f64, u64, u64)> = Vec::with_capacity(edge_count);

        for (from, to) in &sorted_edges {
            let er = &src.edges[&(*from, *to)];
            // edge_data 区域的起始偏移量
            let edge_data_base = HEADER_SIZE
                              + VERTEX_IDX_SIZE * vertex_count
                              + vertex_data_bytes.len()
                              + EDGE_IDX_SIZE * edge_count;
            let data_offset = (edge_data_base as u64) + (edge_data_bytes.len() as u64);
            let encoded = encode_properties(&er.properties);
            let data_len = encoded.len() as u64;
            edge_index_entries.push((*from, *to, er.weight, data_offset, data_len));
            edge_data_bytes.extend_from_slice(&encoded);
        }

        // ── 第 4 步：计算各区域偏移量 ──

        let vertex_index_offset = HEADER_SIZE;
        let vertex_data_offset  = vertex_index_offset + VERTEX_IDX_SIZE * vertex_count;
        let edge_index_offset   = vertex_data_offset  + vertex_data_bytes.len();
        let edge_data_offset    = edge_index_offset   + EDGE_IDX_SIZE   * edge_count;

        // ── 第 5 步：写入文件 ──

        // Header（固定 64 字节，小端字节序）
        let mut header_buf = [0u8; HEADER_SIZE];
        header_buf[0..4].copy_from_slice(MAGIC);
        header_buf[4..6].copy_from_slice(&VERSION.to_le_bytes());
        // bytes 6..8 保留（对齐到 u64）
        header_buf[8..16].copy_from_slice(&(vertex_count as u64).to_le_bytes());
        header_buf[16..24].copy_from_slice(&(edge_count   as u64).to_le_bytes());
        header_buf[24..32].copy_from_slice(&(vertex_index_offset as u64).to_le_bytes());
        header_buf[32..40].copy_from_slice(&(vertex_data_offset  as u64).to_le_bytes());
        header_buf[40..48].copy_from_slice(&(edge_index_offset   as u64).to_le_bytes());
        header_buf[48..56].copy_from_slice(&(edge_data_offset    as u64).to_le_bytes());
        // bytes 56..64 = reserved（全 0）
        file.write_all(&header_buf)?;

        // Vertex Index
        for (id, data_offset, data_len) in &vertex_index_entries {
            write_u64(&mut Vec::new(), *id);  // placeholder，直接用 file.write_all
            let mut buf = [0u8; VERTEX_IDX_SIZE];
            buf[0..8].copy_from_slice(&id.to_le_bytes());
            buf[8..16].copy_from_slice(&data_offset.to_le_bytes());
            buf[16..24].copy_from_slice(&data_len.to_le_bytes());
            file.write_all(&buf)?;
        }

        // Vertex Data
        file.write_all(&vertex_data_bytes)?;

        // Edge Index
        for (from, to, weight, data_offset, data_len) in &edge_index_entries {
            let mut buf = [0u8; EDGE_IDX_SIZE];
            buf[0..8].copy_from_slice(&from.to_le_bytes());
            buf[8..16].copy_from_slice(&to.to_le_bytes());
            buf[16..24].copy_from_slice(&weight.to_le_bytes());
            buf[24..32].copy_from_slice(&data_offset.to_le_bytes());
            buf[32..40].copy_from_slice(&data_len.to_le_bytes());
            file.write_all(&buf)?;
        }

        // Edge Data
        file.write_all(&edge_data_bytes)?;

        file.sync_all()?;
        Ok(())
    }
}

// ── 测试 ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PersistentGraph, PropertyValue};
    use std::collections::HashMap;

    fn setup_test_graph() -> PersistentGraph {
        let mut g = PersistentGraph::open("/tmp/test_mmap_convert.bin").unwrap();
        // 清空
        g.vertices.clear();
        g.edges.clear();

        let mut alice = HashMap::new();
        alice.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        alice.insert("age".to_string(),  PropertyValue::Int(28));
        g.add_vertex(1, alice);

        let mut bob = HashMap::new();
        bob.insert("name".to_string(), PropertyValue::String("Bob".to_string()));
        g.add_vertex(2, bob);

        g.add_edge(1, 2, 1.0, HashMap::new());
        g
    }

    #[test]
    fn test_convert_and_open() {
        let g = setup_test_graph();
        let mmap_path = "/tmp/test_mmap_graph.mmap";

        // 转换
        MmapGraph::convert_from(&g, mmap_path).expect("convert failed");

        // 用 mmap 打开
        let mmap_g = MmapGraph::open(mmap_path).expect("open failed");

        assert_eq!(mmap_g.vertex_count(), 2);
        assert_eq!(mmap_g.edge_count(),   1);

        // 验证顶点属性可读
        let alice = mmap_g.get_vertex(1).expect("Alice not found");
        assert_eq!(alice["name"], PropertyValue::String("Alice".to_string()));

        // 验证边可读
        let (weight, _) = mmap_g.get_edge(1, 2).expect("edge not found");
        assert_eq!(weight, 1.0);

        // 清理
        let _ = std::fs::remove_file(mmap_path);
    }

    #[test]
    fn test_out_neighbors() {
        let g = setup_test_graph();
        let mmap_path = "/tmp/test_mmap_out_neighbors.mmap";
        MmapGraph::convert_from(&g, mmap_path).unwrap();

        let mmap_g = MmapGraph::open(mmap_path).unwrap();
        let neighbors = mmap_g.out_neighbors(1);
        assert_eq!(neighbors, vec![2]);

        // 不存在的顶点，返回空
        let empty = mmap_g.out_neighbors(999);
        assert!(empty.is_empty());

        let _ = std::fs::remove_file(mmap_path);
    }

    #[test]
    fn test_iter_vertices() {
        let g = setup_test_graph();
        let mmap_path = "/tmp/test_mmap_iter_vertices.mmap";
        MmapGraph::convert_from(&g, mmap_path).unwrap();

        let mmap_g = MmapGraph::open(mmap_path).unwrap();
        let vertices: Vec<_> = mmap_g.iter_vertices().collect();
        assert_eq!(vertices.len(), 2);

        // 验证属性正确解码
        for (id, props) in &vertices {
            if *id == 1 {
                assert_eq!(props["name"], PropertyValue::String("Alice".to_string()));
            }
        }

        let _ = std::fs::remove_file(mmap_path);
    }

    #[test]
    fn test_iter_edges() {
        let g = setup_test_graph();
        let mmap_path = "/tmp/test_mmap_iter_edges.mmap";
        MmapGraph::convert_from(&g, mmap_path).unwrap();

        let mmap_g = MmapGraph::open(mmap_path).unwrap();
        let edges: Vec<_> = mmap_g.iter_edges().collect();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0], (1, 2, 1.0, HashMap::new()));

        let _ = std::fs::remove_file(mmap_path);
    }

    #[test]
    fn test_open_invalid_file() {
        // 不存在的文件
        let result = MmapGraph::open("/tmp/nonexistent.mmap");
        assert!(result.is_err());

        // magic 错误的文件
        let mut f = File::create("/tmp/test_bad_magic.mmap").unwrap();
        f.write_all(b"XXXX").unwrap();  // 不是 AXOM
        f.sync_all().unwrap();
        let result = MmapGraph::open("/tmp/test_bad_magic.mmap");
        assert!(result.is_err());
        let _ = std::fs::remove_file("/tmp/test_bad_magic.mmap");
    }
}
