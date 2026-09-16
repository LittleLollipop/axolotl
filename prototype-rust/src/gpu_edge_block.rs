// src/gpu_edge_block.rs
// GPU EdgeBlock 统一内存图存储
//
// EdgeBlock 的核心设计：
// ────────────────────
// 固定大小的 Block（34 个 u32/block = 136 bytes）直铺在内存中，
// CPU 和 GPU 共享同一份数据，Metal buffer 分配一次、复用到底。
//
// 正向 Block（出边）：
//   blocks[i]   = nextBlock    (header) —— 同一顶点的下一个 block 索引，
//                                          链尾 = u32::MAX
//   blocks[i+1] = edgeCount     (header)
//   blocks[i+2..i+34] = 目标顶点（32 个槽位，不足填 0）
//
// 反向 Block（入边）：同样结构，用于 PageRank 等需要入边的算法。
//
// 新增边时追加到该顶点 block **链尾**：链尾还有空位就就地追加，满了就新开一个
// block 接到链尾（摊销 O(1)，代价是先沿链走到尾）。
//
// ⚠️ **同一顶点的多个 block 在 blocks 数组里不保证相邻** —— 别的顶点的 block
//    会插在中间。所有遍历必须走 `walk_blocks`，**不许**再用
//    「first_block .. first_block + count」这个连续区间。那是一个真实缺陷的
//    来源：读取时会一路读到别人的 block，而**条数看起来还是对的**（实测真实库
//    Σout_neighbors 虚高 59,802 条，1670 个顶点度数不对且全部是「多」）。
//
// 为什么槽 0 用 nextBlock 而不是「在 header 后面再插一个字段」：
// 那个字段原来是 ownerVertex，而它**从来没有被读过**（全库只有 3 处 push）。
// 沿用空槽 ⇒ BLOCK_SIZE_U32 保持 34 ⇒ `scc` / `louvain` / `betweenness` /
// `wave_core` / `server` / `py_bindings`（硬编码 34）与 Metal 内核全都不用动。

use std::collections::HashMap;
use crate::PropertyValue;

/// 边属性数据（独立于拓扑结构存储）
#[derive(Debug, Clone)]
pub struct EdgeData {
    pub weight: f32,
    pub properties: HashMap<String, PropertyValue>,
}

impl EdgeData {
    pub fn new(weight: f32) -> Self {
        EdgeData { weight, properties: HashMap::new() }
    }
    pub fn with_props(weight: f32, properties: HashMap<String, PropertyValue>) -> Self {
        EdgeData { weight, properties }
    }
}

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

    // ── 边计数 + 属性 ──
    pub total_edges: u64,
    /// 边属性存储（外部 ID → **该对之间的全部平行边**），独立于拓扑结构。
    ///
    /// ★ 值是 `Vec` 而不是单个 `EdgeData`，这是被一个真实缺陷逼出来的：
    ///   拓扑层（`blocks`）本来就是**多重**的 —— `out_neighbors` 不去重，
    ///   同一对出现几次就是几条平行边（实测 `out_neighbors(1) = [2,2,2]`）。
    ///   而这里原来是 `(from,to) -> EdgeData`，后写覆盖前写 ⇒
    ///   `get_edge` 只能拿到**最后一条**，前面的边属性**静默丢失**。
    ///   （真实场景：一个词可以同时以多种关系指向同一个类。）
    ///
    /// 顺序保证：与 `out_neighbors` 的返回顺序一致 —— 两者都是**追加序**。
    pub edge_data: HashMap<(u64, u64), Vec<EdgeData>>,

    // ── Dirty 标记（GPU 同步用）──
    pub blocks_dirty: bool,
    pub reverse_blocks_dirty: bool,
    pub structure_dirty: bool, // vertices/block_counts 发生变化
}

impl GPUEdgeBlockGraph {
    pub const BLOCK_CAPACITY: usize = 32;
    /// 一个 block 占的 u32 数：`[nextBlock, edgeCount, edge0..edge31]` = 34。
    pub const BLOCK_SIZE_U32: usize = 2 + Self::BLOCK_CAPACITY; // = 34

    // ── block header 的两个槽 ──────────────────────────────────
    /// 槽 0：**同一下一个 block 的索引**，链尾为 `NO_BLOCK`。
    ///
    /// ★ 这个槽原来存 `ownerVertex`，而它**从来没有被读过**（全库只有 3 处
    ///   `push`，零处读取）。本次把它改作链指针是**刻意**的选择：
    ///   如果改成「在 header 后面再插一个字段」，`BLOCK_SIZE_U32` 会从 34
    ///   变成 35 —— 而它被 `scc.rs` / `louvain.rs` / `betweenness.rs` /
    ///   `wave_core.rs` / `server.rs` / `py_bindings.rs`（那里**硬编码 34**）
    ///   以及 `gpu/bfs_edgeblock.metal` 各自**独立解析**。
    ///   借用空槽则改动收敛在本文件内，GPU 内核一个字都不用动。
    pub const OFF_NEXT: usize = 0;
    /// 槽 1：这个 block 里实际装了几条边。
    pub const OFF_COUNT: usize = 1;
    /// 槽 2 起：邻居（内部索引）。
    pub const OFF_EDGES: usize = 2;
    /// 链尾哨兵（`u32::MAX`，与 `idx_to_id` 的 tombstone 同值但语义无关）。
    pub const NO_BLOCK: u32 = u32::MAX;

    /// 遍历某顶点的 block 链，对每个 block 的**起始偏移**调用 `f`。
    ///
    /// ★ **这是全库唯一允许知道「block 之间怎么走」的地方。**
    ///
    /// 这个知识原来以「连续区间 `first..first+count`」的形式**散落在 8 处**
    /// （append / out_neighbors / in_neighbors / remove_from_blocks /
    /// collect_outgoing_targets / collect_incoming_sources / clear_blocks /
    /// out_degree），而那正是一个真实缺陷的温床：
    ///
    /// > `append_to_block` 把新 block `push` 到全局 `blocks` 数组**末尾**，
    /// > 而读取侧按**连续区间**遍历。「push 到末尾」≠「紧接该顶点的最后一个
    /// > block 之后」—— 两者只在「该顶点是最后一个分配 block 的顶点」时相等。
    /// > 真实库实测：Σ`out_neighbors` 比真实边数虚高 59,802，1670/44,875
    /// > 个顶点度数不符，且**全部是「多」**（混进了别人的 block）。
    ///
    /// `guard` 是防环闸门：损坏的链（或手工构造的畸形数据）不许让它死循环 ——
    /// 死循环会把「数据坏了」变成「进程卡住」，后者更难查。
    fn walk_blocks<F: FnMut(usize)>(blocks: &[u32], head: u32, mut f: F) {
        let total = blocks.len() / Self::BLOCK_SIZE_U32;
        let mut blk = head;
        let mut guard = 0usize;
        while blk != Self::NO_BLOCK && (blk as usize) < total {
            let off = blk as usize * Self::BLOCK_SIZE_U32;
            f(off);
            blk = blocks[off + Self::OFF_NEXT];
            guard += 1;
            if guard > total {
                break;
            }
        }
    }

    /// 在 `blocks` 末尾新开一个 block，返回它的索引。链尾已是 `NO_BLOCK`。
    fn alloc_block(blocks: &mut Vec<u32>, neighbor: u32) -> u32 {
        let idx = (blocks.len() / Self::BLOCK_SIZE_U32) as u32;
        blocks.push(Self::NO_BLOCK);            // 槽 0：next
        blocks.push(1);                         // 槽 1：edgeCount
        blocks.push(neighbor);                  // 槽 2：第一条边
        for _ in 1..Self::BLOCK_CAPACITY {
            blocks.push(0);                     // 其余槽位填 0
        }
        idx
    }

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
            edge_data: HashMap::new(),

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
        self.vertices.push(Self::NO_BLOCK);
        self.block_counts.push(0);
        self.reverse_vertices.push(Self::NO_BLOCK);
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

        self.vertices.push(Self::NO_BLOCK);
        self.block_counts.push(0);
        self.reverse_vertices.push(Self::NO_BLOCK);
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
        self.add_edge_with_props(from, to, weight, HashMap::new())
    }

    /// 添加边（带属性）
    pub fn add_edge_with_props(
        &mut self, from: u64, to: u64, weight: f32,
        properties: HashMap<String, PropertyValue>,
    ) {
        let from_idx = self.get_or_create_idx(from);
        let to_idx = self.get_or_create_idx(to);
        let to_idx_u32 = to_idx as u32;

        // ── 正向 Block（出边）──
        self.append_to_block(from_idx, to_idx_u32, false);

        // ── 反向 Block（入边）──
        self.append_to_block(to_idx, from_idx as u32, true);

        // ── 存储边属性 ──
        // 追加而不是覆盖：拓扑层也是追加，两边的顺序因此保持一致
        self.edge_data
            .entry((from, to))
            .or_default()
            .push(EdgeData { weight, properties });

        self.total_edges += 1;
    }

    /// 获取边属性（**最后一条**；要全部平行边用 `get_edges`，要一次拿全用 `out_edges`）
    pub fn get_edge(&self, from: u64, to: u64) -> Option<&EdgeData> {
        self.edge_data.get(&(from, to)).and_then(|v| v.last())
    }

    /// 获取一对顶点之间的**全部平行边**（按添加顺序）
    pub fn get_edges(&self, from: u64, to: u64) -> &[EdgeData] {
        match self.edge_data.get(&(from, to)) {
            Some(v) => v.as_slice(),
            None => &[],
        }
    }

    /// ★ 一次拿到一个顶点的**全部出边及其数据** → `[(dst, weight, props)]`。
    ///
    /// 为什么需要它：`out_neighbors` 只给目标 id，要拿权重/属性得对每个邻居
    /// **再调一次** `get_edge`（N 次跨语言往返）；而且同一目标有多条平行边时
    /// `get_edge` **只能给最后一条**。这里一次返回全部。
    ///
    /// `weight` 是 `Option`：拓扑层有这条边、属性层却查不到时给 `None` ——
    /// **不拿默认值糊过去**。"这条边没有数据"和"这条边权重是 1.0"
    /// 在类型上就该分开，否则调用方永远看不出中间少了一层。
    pub fn out_edges(&self, id: u64)
        -> Vec<(u64, Option<f32>, HashMap<String, PropertyValue>)> {
        self.edges_of(id, false)
    }

    /// 入边的对称版本 → `[(src, weight, props)]`。
    pub fn in_edges(&self, id: u64)
        -> Vec<(u64, Option<f32>, HashMap<String, PropertyValue>)> {
        self.edges_of(id, true)
    }

    /// `out_edges` / `in_edges` 的共同实现。
    ///
    /// 对齐方式：`out_neighbors`(或 `in_neighbors`) 与 `edge_data` 都是追加序，
    /// 所以第 i 次出现的对端对应 `edge_data[(a,b)][i]`。
    fn edges_of(&self, id: u64, reverse: bool)
        -> Vec<(u64, Option<f32>, HashMap<String, PropertyValue>)> {
        let mut result = Vec::new();
        let mut seen: HashMap<u64, usize> = HashMap::new();
        let ends = if reverse { self.in_neighbors(id) } else { self.out_neighbors(id) };
        for other in ends {
            let slot = {
                let c = seen.entry(other).or_insert(0);
                let s = *c;
                *c += 1;
                s
            };
            let key = if reverse { (other, id) } else { (id, other) };
            match self.edge_data.get(&key).and_then(|v| v.get(slot)) {
                Some(d) => result.push((other, Some(d.weight), d.properties.clone())),
                None => result.push((other, None, HashMap::new())),
            }
        }
        result
    }

    /// ★ 紧凑型 `out_edges`：只回 `(邻居, 指定字符串属性)` 的**并列数组**。
    ///
    /// 为什么需要它 —— 实测逼出来的：`out_edges` 给每条边构造一个
    /// `HashMap<String, PropertyValue>`，`py_bindings` 再把它投影成 Python dict。
    /// 这层构造成本把底层 40 倍的优势**全部吃掉**：
    ///
    /// | 场景 | 纯邻居 `out_neighbors` | `out_edges` |
    /// |---|---|---|
    /// | 单跳 | 0.32ms | 8.95ms |
    /// | 长链 8 步 | 0.18ms | 4.30ms（**比 SQLite 还慢 16 倍**） |
    ///
    /// 而调用方（Banyan 的 `graph.py`）真正要的只是 `(relation, dst)` 对 ——
    /// weight 和其余属性从来不读。返回形态是这里唯一的性能杠杆。
    ///
    /// 对齐方式与 `edges_of` 相同：同一 `(id, dst)` 的第 i 次出现对应
    /// `edge_data[(id, dst)][i]` —— 两边都是追加序。
    /// 属性缺失、或值不是字符串时给**空串**：既 panic 也不拿假值糊。
    pub fn out_edges_cols(&self, id: u64, key: &str, distinct: bool)
        -> (Vec<u64>, Vec<String>)
    {
        let mut dsts: Vec<u64> = Vec::new();
        let mut vals: Vec<String> = Vec::new();
        let idx = match self.id_to_idx.get(&id) {
            Some(&i) => i,
            None => return (dsts, vals),
        };
        if self.block_counts[idx] == 0 {
            return (dsts, vals);
        }
        let blocks = &self.blocks;
        let ids = &self.idx_to_id;
        let ed = &self.edge_data;
        let mut seen: HashMap<u64, usize> = HashMap::new();
        // `distinct`：按 `(dst, key 的值)` 去重。
        //
        // ★ 这个开关不是可有可无的优化 —— 它对应**两种不同的语义**，
        //   而调用方（Banyan 的 `graph.py`）要的是**去重**那一种：
        //   `edges` 表的主键是 `(src, relation, dst)`，遍历只关心
        //   「有没有这条边」；而 `edge_events` 才是含重复观测的历史。
        //   同一次测量里混用两种语义，倍数会变成两种口径的混合，读数不可解释。
        let mut emitted: std::collections::HashSet<(u64, String)> =
            std::collections::HashSet::new();
        Self::walk_blocks(blocks, self.vertices[idx], |off| {
            let ec = (blocks[off + Self::OFF_COUNT] as usize).min(Self::BLOCK_CAPACITY);
            for e in 0..ec {
                let t = blocks[off + Self::OFF_EDGES + e] as usize;
                if t >= ids.len() || ids[t] == u64::MAX {
                    continue;
                }
                let d = ids[t];
                // ⚠️ slot 必须**先算**、无论这条边最后输不输出。
                //    slot 是「(id,d) 的第几次出现」，是定位 edge_data 的唯一依据；
                //    一旦对跳过的边不计数，后面所有同对边的属性都会错位一格。
                let slot = {
                    let c = seen.entry(d).or_insert(0);
                    let s = *c;
                    *c += 1;
                    s
                };
                let v = ed
                    .get(&(id, d))
                    .and_then(|v| v.get(slot))
                    .and_then(|e| e.properties.get(key))
                    .and_then(|pv| match pv {
                        PropertyValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                if distinct && !emitted.insert((d, v.clone())) {
                    continue;
                }
                dsts.push(d);
                vals.push(v);
            }
        });
        (dsts, vals)
    }

    /// 向顶点 `v_idx` 的 block **链尾**追加一条边。
    ///
    /// `is_reverse` 从 `&mut bool` 改成 `bool`：调用方传的是 `&mut false` /
    /// `&mut true`（对临时值取可变引用），而函数里从未写过它 —— 一个签名上的
    /// 假象，会让读代码的人以为它会被修改。
    fn append_to_block(&mut self, v_idx: usize, neighbor: u32, is_reverse: bool) {
        let (vertices, block_counts, blocks) = if is_reverse {
            (&mut self.reverse_vertices, &mut self.reverse_block_counts, &mut self.reverse_blocks)
        } else {
            (&mut self.vertices, &mut self.block_counts, &mut self.blocks)
        };

        if block_counts[v_idx] == 0 {
            // 该顶点还没有任何 block
            let b = Self::alloc_block(blocks, neighbor);
            vertices[v_idx] = b;
            block_counts[v_idx] = 1;
            if is_reverse {
                self.reverse_blocks_dirty = true;
            } else {
                self.blocks_dirty = true;
            }
        } else {
            // ★ 沿链走到最后一个 block。
            //
            // 这里**绝不能**写成 `last = first + count - 1` —— 那是本文件
            // 原来的写法，也是那个真实缺陷的来源：一个顶点的多个 block 在
            // `blocks` 里**不一定相邻**（别的顶点的 block 会插在中间），
            // 于是读的时候会一路读到别人的数据上，而**条数看起来还是对的**。
            let mut last = vertices[v_idx];
            loop {
                let nxt = blocks[last as usize * Self::BLOCK_SIZE_U32 + Self::OFF_NEXT];
                if nxt == Self::NO_BLOCK {
                    break;
                }
                last = nxt;
            }
            let off = last as usize * Self::BLOCK_SIZE_U32;
            let edge_count = blocks[off + Self::OFF_COUNT] as usize;

            if edge_count < Self::BLOCK_CAPACITY {
                // 链尾 block 还有空位，就地追加
                blocks[off + Self::OFF_EDGES + edge_count] = neighbor;
                blocks[off + Self::OFF_COUNT] = (edge_count + 1) as u32;
                if is_reverse {
                    self.reverse_blocks_dirty = true;
                } else {
                    self.blocks_dirty = true;
                }
            } else {
                // 链尾已满：新开一个 block 并接到链尾
                let b = Self::alloc_block(blocks, neighbor);
                blocks[off + Self::OFF_NEXT] = b;
                block_counts[v_idx] += 1;
                self.structure_dirty = true;
                if is_reverse {
                    self.reverse_blocks_dirty = true;
                } else {
                    self.blocks_dirty = true;
                }
            }
        }
    }

    // ── 删除操作 ─────────────────────────

    /// 删除一条边 (from → to)
    /// 返回 true 如果边存在且被删除
    pub fn remove_edge(&mut self, from: u64, to: u64) -> bool {
        let from_idx = match self.id_to_idx.get(&from) { Some(&i) => i, None => return false };
        let to_idx = match self.id_to_idx.get(&to) { Some(&i) => i, None => return false };

        // 1. 从正向 blocks 删除
        let found_fwd = self.remove_from_blocks(from_idx, to_idx as u32, false);
        // 2. 从反向 blocks 删除
        let found_rev = self.remove_from_blocks(to_idx, from_idx as u32, true);

        if found_fwd || found_rev {
            self.total_edges = self.total_edges.saturating_sub(1);
            self.edge_data.remove(&(from, to));
            self.blocks_dirty = true;
            self.reverse_blocks_dirty = true;
            true
        } else {
            false
        }
    }

    /// 从指定顶点的 blocks 中删除一条边（内部方法）
    fn remove_from_blocks(&mut self, owner_idx: usize, neighbor: u32, is_reverse: bool) -> bool {
        let (vertices, block_counts, blocks) = if is_reverse {
            (&mut self.reverse_vertices, &mut self.reverse_block_counts, &mut self.reverse_blocks)
        } else {
            (&mut self.vertices, &mut self.block_counts, &mut self.blocks)
        };

        if block_counts[owner_idx] == 0 { return false; }

        let total = blocks.len() / Self::BLOCK_SIZE_U32;
        let mut blk = vertices[owner_idx];
        let mut guard = 0usize;
        while blk != Self::NO_BLOCK && (blk as usize) < total {
            let off = blk as usize * Self::BLOCK_SIZE_U32;
            let ec = (blocks[off + Self::OFF_COUNT] as usize).min(Self::BLOCK_CAPACITY);
            for e in 0..ec {
                if blocks[off + Self::OFF_EDGES + e] == neighbor {
                    // 找到了，shift 剩余边左移
                    for j in e..ec.saturating_sub(1) {
                        blocks[off + Self::OFF_EDGES + j] =
                            blocks[off + Self::OFF_EDGES + j + 1];
                    }
                    // 清零最后一个槽位
                    if ec > 0 {
                        blocks[off + Self::OFF_EDGES + ec - 1] = 0;
                    }
                    blocks[off + Self::OFF_COUNT] = (ec - 1) as u32;
                    return true;
                }
            }
            blk = blocks[off + Self::OFF_NEXT];
            guard += 1;
            if guard > total { break; }
        }
        false
    }

    /// 删除顶点及其所有关联边
    /// 返回被删除顶点的属性（如果存在）
    pub fn remove_vertex(&mut self, id: u64) -> Option<HashMap<String, crate::PropertyValue>> {
        let idx = self.id_to_idx.get(&id)?;
        let idx = *idx;

        // 1. 收集所有出边邻居（删除前获取）
        let outgoing: Vec<usize> = self.collect_outgoing_targets(idx);

        // 2. 收集所有入边源顶点（删除前获取）
        let incoming_sources: Vec<usize> = self.collect_incoming_sources(idx);

        // 3. 清空正向 blocks
        self.clear_blocks(idx, false);

        // 4. 从出边邻居的反向 blocks 中删除本顶点
        for target_idx in &outgoing {
            self.remove_from_blocks(*target_idx, idx as u32, true);
        }

        // 5. 从入边源顶点的正向 blocks 中删除指向本顶点的边
        for src_idx in &incoming_sources {
            self.remove_from_blocks(*src_idx, idx as u32, false);
        }

        // 6. 清空反向 blocks
        self.clear_blocks(idx, true);

        // 7. 清理边属性
        for target_idx in &outgoing {
            if *target_idx < self.idx_to_id.len() && self.idx_to_id[*target_idx] != u64::MAX {
                self.edge_data.remove(&(id, self.idx_to_id[*target_idx]));
            }
        }
        for src_idx in &incoming_sources {
            if *src_idx < self.idx_to_id.len() && self.idx_to_id[*src_idx] != u64::MAX {
                self.edge_data.remove(&(self.idx_to_id[*src_idx], id));
            }
        }

        // 8. 标记删除
        let props = std::mem::replace(&mut self.vertex_props[idx], HashMap::new());
        self.id_to_idx.remove(&id);
        self.idx_to_id[idx] = u64::MAX; // tombstone
        self.total_edges = self.total_edges.saturating_sub(
            (outgoing.len() + incoming_sources.len()) as u64);

        self.structure_dirty = true;
        Some(props)
    }

    /// 收集某顶点的所有出边目标
    fn collect_outgoing_targets(&self, v_idx: usize) -> Vec<usize> {
        let mut targets = Vec::new();
        if v_idx >= self.block_counts.len() || self.block_counts[v_idx] == 0 {
            return targets;
        }
        let blocks = &self.blocks;
        Self::walk_blocks(blocks, self.vertices[v_idx], |off| {
            let ec = (blocks[off + Self::OFF_COUNT] as usize).min(Self::BLOCK_CAPACITY);
            for e in 0..ec {
                targets.push(blocks[off + Self::OFF_EDGES + e] as usize);
            }
        });
        targets
    }

    /// 清除顶点的所有 blocks。
    ///
    /// 结尾把 `vertices[v_idx]` 设成 `NO_BLOCK`：让「这个顶点没有 block」
    /// 在数据上**明确**，而不是靠 `block_counts == 0` 隐式表达 ——
    /// 两个字段分别表示两件事时，一旦不同步就是静默的错。
    fn clear_blocks(&mut self, v_idx: usize, is_reverse: bool) {
        let (vertices, block_counts, blocks) = if is_reverse {
            (&mut self.reverse_vertices, &mut self.reverse_block_counts, &mut self.reverse_blocks)
        } else {
            (&mut self.vertices, &mut self.block_counts, &mut self.blocks)
        };
        if block_counts[v_idx] == 0 { return; }
        let total = blocks.len() / Self::BLOCK_SIZE_U32;
        let mut blk = vertices[v_idx];
        let mut guard = 0usize;
        while blk != Self::NO_BLOCK && (blk as usize) < total {
            let off = blk as usize * Self::BLOCK_SIZE_U32;
            let nxt = blocks[off + Self::OFF_NEXT];
            blocks[off + Self::OFF_COUNT] = 0; // edgeCount → 0
            blk = nxt;
            guard += 1;
            if guard > total { break; }
        }
        block_counts[v_idx] = 0;
        vertices[v_idx] = Self::NO_BLOCK;
    }

    /// 收集指向某顶点的所有源顶点
    fn collect_incoming_sources(&self, target_idx: usize) -> Vec<usize> {
        let mut sources = Vec::new();
        if target_idx >= self.reverse_block_counts.len()
            || self.reverse_block_counts[target_idx] == 0
        {
            return sources;
        }
        let blocks = &self.reverse_blocks;
        let ids = &self.idx_to_id;
        Self::walk_blocks(blocks, self.reverse_vertices[target_idx], |off| {
            let ec = (blocks[off + Self::OFF_COUNT] as usize).min(Self::BLOCK_CAPACITY);
            for e in 0..ec {
                let src = blocks[off + Self::OFF_EDGES + e] as usize;
                if src < ids.len() && ids[src] != u64::MAX {
                    sources.push(src);
                }
            }
        });
        sources
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

        let blocks = &self.blocks;
        let ids = &self.idx_to_id;
        Self::walk_blocks(blocks, self.vertices[v_idx], |off| {
            // 夹住损坏的 edgeCount：越界 panic 会把「数据坏了」变成「进程挂了」，
            // 后者更难查。
            let edge_count =
                (blocks[off + Self::OFF_COUNT] as usize).min(Self::BLOCK_CAPACITY);
            for e in 0..edge_count {
                let target_idx = blocks[off + Self::OFF_EDGES + e] as usize;
                if target_idx < ids.len() && ids[target_idx] != u64::MAX {
                    result.push(ids[target_idx]);
                }
            }
        });
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

        let blocks = &self.reverse_blocks;
        let ids = &self.idx_to_id;
        Self::walk_blocks(blocks, self.reverse_vertices[v_idx], |off| {
            let edge_count =
                (blocks[off + Self::OFF_COUNT] as usize).min(Self::BLOCK_CAPACITY);
            for e in 0..edge_count {
                let src_idx = blocks[off + Self::OFF_EDGES + e] as usize;
                if src_idx < ids.len() && ids[src_idx] != u64::MAX {
                    result.push(ids[src_idx]);
                }
            }
        });
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
        let blocks = &self.blocks;
        let mut total = 0;
        Self::walk_blocks(blocks, self.vertices[v_idx], |off| {
            total += (blocks[off + Self::OFF_COUNT] as usize).min(Self::BLOCK_CAPACITY);
        });
        total
    }

    // ── 增强遍历 ─────────────────────────

    /// 多跳遍历：从 start 出发，最深 depth 层，对每层的邻居调用 visitor
    /// 返回 visited 集合（所有访问过的顶点）
    pub fn walk<F>(&self, start: u64, max_depth: usize, mut visitor: F) -> Vec<u64>
    where
        F: FnMut(u64, usize, u64), // (当前顶点, 深度, 邻居)
    {
        use std::collections::{HashSet, VecDeque};
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        if self.id_to_idx.get(&start).is_none() {
            return result;
        }

        queue.push_back((start, 0usize));
        visited.insert(start);
        result.push(start);

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth { continue; }
            for nbr in self.out_neighbors(current) {
                visitor(current, depth, nbr);
                if !visited.contains(&nbr) {
                    visited.insert(nbr);
                    result.push(nbr);
                    queue.push_back((nbr, depth + 1));
                }
            }
        }
        result
    }

    /// 子图提取：从 seeds 出发，取出 depth 层内的所有顶点和边
    /// 返回 (顶点列表, 边列表)
    pub fn subgraph(&self, seeds: &[u64], max_depth: usize)
        -> (Vec<u64>, Vec<(u64, u64, Option<EdgeData>)>)
    {
        use std::collections::{HashSet, VecDeque};
        let mut vertices = HashSet::new();
        let mut edges = Vec::new();
        let mut queue = VecDeque::new();

        for &seed in seeds {
            if self.id_to_idx.contains_key(&seed) {
                queue.push_back((seed, 0usize));
                vertices.insert(seed);
            }
        }

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth { continue; }
            for nbr in self.out_neighbors(current) {
                // 取该对的最后一条（与 `get_edge` 同语义）；要全部平行边用 `get_edges`
                edges.push((current, nbr,
                            self.edge_data.get(&(current, nbr))
                                .and_then(|v| v.last()).cloned()));
                if !vertices.contains(&nbr) {
                    vertices.insert(nbr);
                    queue.push_back((nbr, depth + 1));
                }
            }
        }

        (vertices.into_iter().collect(), edges)
    }

    /// 路径模式匹配：找出所有满足条件的路径 A→B→C（长度 = path_length）
    /// vertex_filter: 筛选中间顶点
    /// edge_filter: 筛选边属性（可选）
    /// 返回 Vec<路径顶点序列>
    pub fn find_paths(
        &self,
        vertex_filter: Option<&dyn Fn(u64) -> bool>,
        edge_filter: Option<&dyn Fn(&EdgeData) -> bool>,
        path_length: usize,
    ) -> Vec<Vec<u64>> {
        let mut paths = Vec::new();

        for v_idx in 0..self.vertex_count as usize {
            let start_id = self.idx_to_id[v_idx];
            if start_id == u64::MAX { continue; }
            if let Some(ref f) = vertex_filter {
                if !f(start_id) { continue; }
            }

            let mut current_path = vec![start_id];
            self.find_paths_dfs(
                v_idx, start_id, &mut current_path, path_length,
                vertex_filter, edge_filter, &mut paths,
            );
        }
        paths
    }

    fn find_paths_dfs(
        &self,
        v_idx: usize,
        v_id: u64,
        path: &mut Vec<u64>,
        max_len: usize,
        vertex_filter: Option<&dyn Fn(u64) -> bool>,
        edge_filter: Option<&dyn Fn(&EdgeData) -> bool>,
        results: &mut Vec<Vec<u64>>,
    ) {
        if path.len() > max_len {
            results.push(path.clone());
            return;
        }

        for nbr_id in self.out_neighbors_by_idx(v_idx) {
            if let Some(ref ef) = edge_filter {
                // 一对之间可能有多条平行边：**任一条满足**就算通过
                // （原来是单值，取到哪条取决于覆盖顺序 —— 现在语义明确了）
                if let Some(v) = self.edge_data.get(&(v_id, nbr_id)) {
                    if !v.iter().any(ef) { continue; }
                }
            }

            if path.len() < max_len {
                if let Some(ref vf) = vertex_filter {
                    if !vf(nbr_id) { continue; }
                }
            }

            if let Some(&nbr_idx) = self.id_to_idx.get(&nbr_id) {
                path.push(nbr_id);
                self.find_paths_dfs(
                    nbr_idx, nbr_id, path, max_len,
                    vertex_filter, edge_filter, results,
                );
                path.pop();
            }
        }
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
        let mut vertices = vec![Self::NO_BLOCK; vc];
        let mut block_counts = vec![0u32; vc];
        let mut blocks = Vec::new();

        for v in 0..vc {
            let start = csr_offsets[v] as usize;
            let end = csr_offsets[v + 1] as usize;
            let degree = end - start;
            if degree == 0 { continue; }

            vertices[v] = (blocks.len() / Self::BLOCK_SIZE_U32) as u32;
            let mut prev_blk = Self::NO_BLOCK;
            let mut offset = start;
            while offset < end {
                let count = usize::min(Self::BLOCK_CAPACITY, end - offset);
                let blk = (blocks.len() / Self::BLOCK_SIZE_U32) as u32;
                // 把上一个 block 的 next 指到这里。CSR 是按顶点顺序构建的，
                // 所以同一顶点的 block 天然相邻 —— 但仍然**显式串链**，
                // 不让「相邻」成为任何读取路径的前提。
                if prev_blk != Self::NO_BLOCK {
                    blocks[prev_blk as usize * Self::BLOCK_SIZE_U32] = blk;
                }
                blocks.push(Self::NO_BLOCK); // next
                blocks.push(count as u32); // edgeCount
                for j in 0..count {
                    blocks.push(csr_targets[offset + j]);
                }
                for _ in count..Self::BLOCK_CAPACITY {
                    blocks.push(0);
                }
                prev_blk = blk;
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

        let mut reverse_vertices = vec![Self::NO_BLOCK; vc];
        let mut reverse_block_counts = vec![0u32; vc];
        let mut reverse_blocks = Vec::new();

        for v in 0..vc {
            let start = rev_offsets[v] as usize;
            let end = rev_offsets[v + 1] as usize;
            let degree = end - start;
            if degree == 0 { continue; }

            reverse_vertices[v] = (reverse_blocks.len() / Self::BLOCK_SIZE_U32) as u32;
            let mut prev_blk = Self::NO_BLOCK;
            let mut offset = start;
            while offset < end {
                let count = usize::min(Self::BLOCK_CAPACITY, end - offset);
                let blk = (reverse_blocks.len() / Self::BLOCK_SIZE_U32) as u32;
                if prev_blk != Self::NO_BLOCK {
                    reverse_blocks[prev_blk as usize * Self::BLOCK_SIZE_U32] = blk;
                }
                reverse_blocks.push(Self::NO_BLOCK); // next
                reverse_blocks.push(count as u32);
                for j in 0..count {
                    reverse_blocks.push(rev_targets[offset + j]);
                }
                for _ in count..Self::BLOCK_CAPACITY {
                    reverse_blocks.push(0);
                }
                prev_blk = blk;
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
            edge_data: HashMap::new(),
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
    /// 文件格式版本。
    ///
    /// **2（2026-09-16）**：block header 的槽 0 从 `ownerVertex`（从未被读取）
    /// 改成 `nextBlock` 链指针。**布局一个字节都没变**（仍是 34 个 u32/block），
    /// 但**语义变了** —— 用新代码读版本 1 的文件，会把 owner 值当链指针走，
    /// 结果是静默地读到错误的邻居。所以版本号必须递增，且 `open` 必须
    /// **拒绝**不认识的版本，而不是像原来那样把版本读进 `_version` 就不管了。
    const FILE_VERSION: u16 = 2;

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

        // Edge data: count + per-edge entries
        // ★ 写的是「边数据**条数**」而不是「键数」—— 有平行边时两者不同。
        //   旧格式没有平行边，所以两种读法等价（**向后兼容**，格式一个字没改）。
        let flat: Vec<((u64, u64), &EdgeData)> = self.edge_data.iter()
            .flat_map(|(k, v)| v.iter().map(move |e| (*k, e)))
            .collect();
        let n_edge_data = flat.len() as u32;
        buf.extend_from_slice(&n_edge_data.to_be_bytes());
        for ((from, to), ed) in flat {
            buf.extend_from_slice(&from.to_be_bytes());
            buf.extend_from_slice(&to.to_be_bytes());
            buf.extend_from_slice(&ed.weight.to_be_bytes());
            let n = ed.properties.len() as u16;
            buf.extend_from_slice(&n.to_be_bytes());
            for (key, value) in &ed.properties {
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
        let version = u16::from_be_bytes([hdr[0], hdr[1]]);
        let cap = u16::from_be_bytes([hdr[2], hdr[3]]);
        if cap as usize != Self::BLOCK_CAPACITY {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "AXEB 文件 BLOCK_CAPACITY={} 与本代码的 {} 不符",
                    cap,
                    Self::BLOCK_CAPACITY
                ),
            ));
        }
        // 版本 1 **可迁移** —— 既不是「可接受」，也不是「可拒绝」。
        //
        // 版本 1 的槽 0 是 `ownerVertex`（这个 block 属于谁），而读取侧按
        // **连续区间**走 —— 那正是版本 1 的缺陷：一个顶点的多个 block 在数组里
        // 不保证相邻。而 `ownerVertex` 恰好记着「谁拥有这个 block」，所以按它
        // 分组就能把每个顶点的 block 找齐并串成链（见 `migrate_v1_blocks`）。
        //
        // ⚠️ **不能拒绝**：现存图库（含 `lobster-memory` 的记忆）都是版本 1 的，
        //    拒绝等于把记忆弄丢。收到这个错误时先想「数据还能救吗」，
        //    再决定是报错还是迁移 —— 直接拒绝是最省事也最不可逆的那个选择。
        let need_migrate = match version {
            1 => true,
            v if v == Self::FILE_VERSION => false,
            other => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "AXEB 文件版本 {} 本代码不认识（支持 1（自动迁移）与 {}）。\
                         布局相同但语义可能不同，**静默误读比报错危险得多**。",
                        other,
                        Self::FILE_VERSION
                    ),
                ));
            }
        };
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
        let mut vertices = read_u32_vec(&mut f)?;
        let block_counts = read_u32_vec(&mut f)?;
        let mut blocks = read_u32_vec(&mut f)?;
        let mut reverse_vertices = read_u32_vec(&mut f)?;
        let reverse_block_counts = read_u32_vec(&mut f)?;
        let mut reverse_blocks = read_u32_vec(&mut f)?;

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

        // Read edge data
        let mut edge_data = HashMap::new();
        let mut lb_edge = [0u8; 4];
        f.read_exact(&mut lb_edge)?;
        let edge_count = u32::from_be_bytes(lb_edge) as usize;
        for _ in 0..edge_count {
            let mut b = [0u8; 8];
            f.read_exact(&mut b)?; let from = u64::from_be_bytes(b);
            f.read_exact(&mut b)?; let to = u64::from_be_bytes(b);
            let mut wb = [0u8; 4];
            f.read_exact(&mut wb)?;
            let weight = f32::from_be_bytes(wb);
            let mut kb = [0u8; 2];
            f.read_exact(&mut kb)?;
            let n = u16::from_be_bytes(kb) as usize;
            let mut props = HashMap::new();
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
            // 追加（同一个 (from,to) 可能有多条平行边，旧格式里每条各占一行）
            edge_data.entry((from, to)).or_insert_with(Vec::new)
                .push(EdgeData { weight, properties: props });
        }

        let mut g = GPUEdgeBlockGraph {
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
            edge_data,
            blocks_dirty: true,
            reverse_blocks_dirty: true,
            structure_dirty: true,
        };
        if need_migrate {
            // 版本 1 的一个顶点的 block 段**不保证连续**（那正是它的缺陷），
            // 所以不能按连续区间走 —— 用槽 0 的 ownerVertex 重新串链。
            Self::migrate_v1_blocks(&mut g.blocks, &mut g.vertices);
            Self::migrate_v1_blocks(&mut g.reverse_blocks, &mut g.reverse_vertices);
        }
        Ok(g)
    }

    /// 把**版本 1** 的 block 数组就地迁移到版本 2 的链式布局。
    ///
    /// 版本 1：槽 0 = `ownerVertex`（这个 block 属于谁），读取靠连续区间（有缺陷）
    /// 版本 2：槽 0 = `nextBlock`（链指针），读取靠链
    ///
    /// 迁移依据：`ownerVertex` 记的正是「谁拥有这个 block」，所以按它分组
    /// 就能把每个顶点的 block 找齐。组内按 block 索引升序 = **添加顺序**
    /// （新 block 一律 `push` 到数组末尾，所以索引大的更晚添加）。
    ///
    /// 为什么不需要重建图：**拓扑数据本身是完整的** —— 每次 `add_edge` 都
    /// 老老实实 `push` 了边，坏的只是「怎么从顶点走到它的 block」。
    /// 所以这是一次索引方式的重写，不是数据修复。
    fn migrate_v1_blocks(blocks: &mut [u32], vertices: &mut [u32]) {
        let nblk = blocks.len() / Self::BLOCK_SIZE_U32;
        let nv = vertices.len();
        let mut by_owner: Vec<Vec<u32>> = vec![Vec::new(); nv];
        for b in 0..nblk {
            let owner = blocks[b * Self::BLOCK_SIZE_U32] as usize;
            if owner < nv {
                by_owner[owner].push(b as u32);
            }
        }
        for v in 0..nv {
            let chain = &by_owner[v];
            if chain.is_empty() {
                vertices[v] = Self::NO_BLOCK;
                continue;
            }
            for w in chain.windows(2) {
                let prev = w[0] as usize * Self::BLOCK_SIZE_U32;
                blocks[prev] = w[1];
            }
            let last = *chain.last().unwrap() as usize * Self::BLOCK_SIZE_U32;
            blocks[last] = Self::NO_BLOCK;
            vertices[v] = chain[0];
        }
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

    // ── block 归属（2026-09-16）─────────────────────────────────
    //
    // 这一组补的是**一个真实缺陷**，比上面那组严重得多：原实现假设
    // 「一个顶点的多个 block 在 `blocks` 数组里连续」，而 `append_to_block`
    // 新开 block 时是 `push` 到**数组末尾**的。「push 到末尾」≠「紧接该顶点的
    // 最后一个 block 之后」—— 两者只在「该顶点是最后一个分配 block 的顶点」
    // 时相等。一旦别的顶点在中间插过 block，读的时候就会跨到别人的数据上，
    // 而**条数看起来还是对的**，所以单看长度断言抓不到。
    //
    // 现有测试为什么全绿：触发形状是固定的三步，少一步都碰不到 ——
    // 单顶点连续加边（`test_many_edges_one_vertex`）和 < 32 条的小图
    // 都永远走在「连续」的情形里。

    /// ★ 最小复现。三步，一步都不能少。
    #[test]
    fn test_block_ownership_interleaved() {
        let mut g = GPUEdgeBlockGraph::new();
        let cap = GPUEdgeBlockGraph::BLOCK_CAPACITY;

        // 1. A（顶点 0）加满一个 block，全部指向 1
        for _ in 0..cap {
            g.add_edge(0, 1, 1.0);
        }
        assert_eq!(g.out_neighbors(0).len(), cap);

        // 2. B（顶点 2）插一条，目标是 7 —— 它占了 A 后面的那个位置
        g.add_edge(2, 7, 1.0);
        assert_eq!(g.out_neighbors(2), vec![7]);

        // 3. A 再加一条（目标 5）—— 这一次必须新开 block
        g.add_edge(0, 5, 1.0);

        let ns = g.out_neighbors(0);
        assert_eq!(ns.len(), cap + 1, "条数");
        assert!(
            !ns.contains(&7),
            "A 的邻居里混进了 B 的目标 7 —— block 归属错位"
        );
        assert_eq!(
            ns.iter().filter(|&&x| x == 5).count(),
            1,
            "A 自己新加的那条必须读得到"
        );
        assert_eq!(ns.iter().filter(|&&x| x == 1).count(), cap);
        assert_eq!(g.out_degree(0), cap + 1);
        assert_eq!(g.out_neighbors(2), vec![7], "B 也必须不受影响");
        assert_eq!(g.total_edges, (cap + 2) as u64);
    }

    /// ★ 保守恒：**大量顶点互相交错**，每个都跨多个 block。
    ///
    /// 最小复现只覆盖「一个热点顶点」。真实库（44,875 顶点 / 109,750 边）是
    /// 全体交错，实测 Σ`out_neighbors` 比真实边数虚高 59,802。这条测那个形状。
    #[test]
    fn test_block_chain_total_conservation_interleaved() {
        let mut g = GPUEdgeBlockGraph::new();
        let cap = GPUEdgeBlockGraph::BLOCK_CAPACITY;
        let nv: u64 = 40;
        let per: u64 = cap as u64 * 2 + 7; // 每个顶点都跨 3 个 block
        for i in 0..nv {
            g.add_vertex(i, HashMap::new());
        }

        // 外层是「第几条」，内层是顶点 —— 保证同一顶点的边被别的顶点隔开
        let mut added: u64 = 0;
        for k in 0..per {
            for i in 0..nv {
                let dst = (i + 1 + k) % nv;
                g.add_edge(i, dst, 1.0);
                added += 1;
            }
        }

        assert_eq!(g.total_edges, added);
        let total_out: usize = (0..nv).map(|i| g.out_neighbors(i).len()).sum();
        assert_eq!(
            total_out as u64, added,
            "Σout_neighbors 必须等于边数（虚高 = 读到了别人的 block）"
        );
        let total_in: usize = (0..nv).map(|i| g.in_neighbors(i).len()).sum();
        assert_eq!(total_in as u64, added, "入边侧同样必须守恒");
        // 出度和走的是同一批 block，必须与邻居数一致
        let deg_sum: usize = (0..nv).map(|i| g.out_degree(i)).sum();
        assert_eq!(deg_sum as u64, added, "Σout_degree 也必须守恒");
    }

    /// ★ 链必须穿过 save/load。
    ///
    /// 布局没变（仍是 34 个 u32/block），但槽 0 的含义变了 —— 文件里存的
    /// 就是链指针本身，所以往返之后链必须还在。
    #[test]
    fn test_block_chain_survives_save_load() {
        let mut g = GPUEdgeBlockGraph::new();
        let cap = GPUEdgeBlockGraph::BLOCK_CAPACITY;
        for _ in 0..cap {
            g.add_edge(0, 1, 1.0);
        }
        g.add_edge(2, 7, 1.0); // 插在中间
        g.add_edge(0, 5, 1.0); // 触发新 block

        let p = std::env::temp_dir().join("axolotl_test_block_chain.axeb");
        let p = p.to_str().unwrap();
        g.save(p).unwrap();
        let g2 = GPUEdgeBlockGraph::open(p).unwrap();

        assert_eq!(g2.out_neighbors(0), g.out_neighbors(0));
        assert_eq!(g2.out_neighbors(2), g.out_neighbors(2));
        assert!(
            !g2.out_neighbors(0).contains(&7),
            "往返之后仍然不许串到别人的 block"
        );
        assert_eq!(g2.total_edges, g.total_edges);
    }

    /// ★ 版本不符必须**响亮失败**。
    ///
    /// 原实现把版本读进 `_version` 就不管了 —— 于是拿版本 1 的文件喂新代码，
    /// 会把 ownerVertex 当链指针走，**静默**读到错误的邻居。
    /// 「不知道」和「坏了」在界面上长得一样，是本项目最顽固的 bug 形状。
    #[test]
    fn test_open_rejects_wrong_version() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_edge(1, 2, 1.0);
        let p = std::env::temp_dir().join("axolotl_test_badver.axeb");
        let p = p.to_str().unwrap();
        g.save(p).unwrap();

        let mut bytes = std::fs::read(p).unwrap();
        assert_eq!(&bytes[0..4], b"AXEB");
        bytes[4] = 0;
        bytes[5] = 99; // 不认识的版本
        std::fs::write(p, &bytes).unwrap();

        let err = GPUEdgeBlockGraph::open(p).expect_err("版本 99 必须被拒绝");
        let msg = format!("{}", err);
        assert!(msg.contains("版本"), "错误信息必须说清是版本问题: {msg}");
        assert!(
            !msg.contains("magic"),
            "错误信息**不许**伪装成 magic 问题 —— 那会把排查带偏: {msg}"
        );
    }

    /// ★ 版本 1 必须**能读**（靠迁移），不是被拒绝。
    ///
    /// 版本 1 的一个顶点的 block 段不保证连续 —— 那正是它的缺陷。而
    /// `ownerVertex` 恰好记着「谁拥有这个 block」，所以能按它重新串链。
    /// 拒绝等于把现存图库（含 `lobster-memory` 的记忆）弄丢。
    #[test]
    fn test_migrate_v1_blocks_rewires_chain() {
        // ⚠️ 块的步长是 `BLOCK_SIZE_U32`（= 34），**不是** `BLOCK_CAPACITY`（= 32）。
        //    32 只是能装几条边，header 另占 2 个 u32。第一版测试把这两个搞混了，
        //    于是 96 个 u32 只解析出 2 个块，第三块压根没被扫到 ——
        //    断言失败在「不该失败的地方」，而真正的原因在测试自己身上。
        let stride = GPUEdgeBlockGraph::BLOCK_SIZE_U32;
        let mut blocks: Vec<u32> = Vec::new();

        // 顶点 0 的第 1 个 block（索引 0）：2 条边
        blocks.push(0); blocks.push(2); blocks.push(10); blocks.push(11);
        for _ in 4..stride { blocks.push(0); }
        // 顶点 1 的 block（索引 1）：1 条边 —— **插在中间**
        blocks.push(1); blocks.push(1); blocks.push(20);
        for _ in 3..stride { blocks.push(0); }
        // 顶点 0 的第 2 个 block（索引 2）：1 条边 —— 与它的第一个**不相邻**
        blocks.push(0); blocks.push(1); blocks.push(12);
        for _ in 3..stride { blocks.push(0); }

        // 版本 1 形状：槽 0 = ownerVertex；`vertices` = 第一个 block 索引
        let mut vertices = vec![0u32, 1];
        let bc = vec![2u32, 1]; // block 数（版本 1 里这个字段同样可信）

        GPUEdgeBlockGraph::migrate_v1_blocks(&mut blocks, &mut vertices);

        assert_eq!(vertices[0], 0, "顶点 0 的链从 block 0 开始");
        assert_eq!(blocks[0], 2, "block 0 的 next 应指向 block 2（不相邻也要串上）");
        assert_eq!(blocks[2 * stride], GPUEdgeBlockGraph::NO_BLOCK, "block 2 是链尾");
        assert_eq!(vertices[1], 1);
        assert_eq!(blocks[stride], GPUEdgeBlockGraph::NO_BLOCK, "顶点 1 只有一个 block");

        // 迁移后必须能**走链**读出正确的邻居 —— 这才是它存在的目的
        //
        // 建 16 个顶点而不是 2 个：上面那些边指向 10/11/12/20，而
        // `out_neighbors` 会**过滤掉不存在的顶点索引**（`target_idx <
        // idx_to_id.len()`）。只建 2 个顶点的话全部边都会被过滤掉，
        // 断言会以「0 条边」的形式失败 —— 而原因在测试的脚手架，不在被测代码。
        let mut g = GPUEdgeBlockGraph::new();
        for i in 0..21u64 {
            g.add_vertex(i, HashMap::new());
        }
        g.blocks = blocks;
        g.vertices = vertices;
        g.block_counts = bc;
        assert_eq!(g.out_neighbors(0).len(), 3, "顶点 0 应有 3 条边");
        assert_eq!(g.out_neighbors(1).len(), 1, "顶点 1 应有 1 条边");
        // 顶点 0 的第 3 条边在**第二个 block** 里（索引 2）—— 不串链就读不到
        assert!(g.out_neighbors(0).contains(&12), "跨 block 的那条边必须读得到");
    }

    // ── 平行边（一对顶点之间多条边）─────────────────────────────
    //
    // 这一组补的是一个真实缺陷：`edge_data` 原来是 `(from,to) -> EdgeData`，
    // 后写覆盖前写 ⇒ 同一对之间的**第 2 条起属性静默丢失**。
    // 而拓扑层（`blocks`）本来就是多重的（`out_neighbors` 不去重），
    // 两层不对称：拓扑知道有 3 条，属性只留得下 1 条。
    //
    // 真实场景：一个词可以同时以多种关系指向同一个类（实测 Banyan 的图里
    // 有 1,986 对节点带 2~10 种关系）。

    fn props(rel: &str) -> HashMap<String, PropertyValue> {
        let mut m = HashMap::new();
        m.insert("relation".to_string(),
                 PropertyValue::String(rel.to_string()));
        m
    }

    fn rel_of(d: &EdgeData) -> Option<PropertyValue> {
        d.properties.get("relation").cloned()
    }

    #[test]
    fn test_parallel_edges_are_all_kept() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(1, HashMap::new());
        g.add_vertex(2, HashMap::new());
        g.add_edge_with_props(1, 2, 0.9, props("属于"));
        g.add_edge_with_props(1, 2, 0.8, props("包含"));
        g.add_edge_with_props(1, 2, 0.7, props("属于"));

        assert_eq!(g.total_edges, 3);
        assert_eq!(g.out_neighbors(1), vec![2, 2, 2], "拓扑一直是多重的");
        assert_eq!(g.get_edges(1, 2).len(), 3, "属性层也必须留住 3 条");
        assert_eq!(g.get_edges(1, 2).iter().map(rel_of).collect::<Vec<_>>(),
                   vec![Some(PropertyValue::String("属于".into())),
                        Some(PropertyValue::String("包含".into())),
                        Some(PropertyValue::String("属于".into()))],
                   "顺序必须是添加顺序");
    }

    #[test]
    fn test_get_edge_still_returns_last() {
        // 向后兼容：单值语义保持「最后一条」
        let mut g = GPUEdgeBlockGraph::new();
        g.add_edge_with_props(1, 2, 0.9, props("A"));
        g.add_edge_with_props(1, 2, 0.7, props("B"));
        let ed = g.get_edge(1, 2).expect("应有边");
        assert_eq!(ed.weight, 0.7);
        assert_eq!(rel_of(ed), Some(PropertyValue::String("B".into())));
    }

    #[test]
    fn test_out_edges_aligns_topology_with_properties() {
        // ★ 关键：`out_edges` 里第 i 次出现的 dst 必须对到第 i 条属性。
        //   两边都是追加序 —— 这个对齐依赖它，所以写一条测试钉住。
        let mut g = GPUEdgeBlockGraph::new();
        g.add_edge_with_props(1, 2, 0.9, props("A"));
        g.add_edge_with_props(1, 3, 0.5, props("B"));
        g.add_edge_with_props(1, 2, 0.1, props("C"));

        let out = g.out_edges(1);
        assert_eq!(out.len(), 3);
        assert_eq!((out[0].0, out[0].1), (2, Some(0.9f32)));
        assert_eq!((out[1].0, out[1].1), (3, Some(0.5f32)));
        assert_eq!((out[2].0, out[2].1), (2, Some(0.1f32)));
        assert_eq!(out[2].2.get("relation"),
                   Some(&PropertyValue::String("C".into())),
                   "第三条必须是 C 而不是被覆盖后的值");
    }

    #[test]
    fn test_in_edges_symmetric() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_edge_with_props(1, 9, 0.9, props("A"));
        g.add_edge_with_props(2, 9, 0.8, props("B"));
        g.add_edge_with_props(1, 9, 0.1, props("C"));

        let inc = g.in_edges(9);
        assert_eq!(inc.iter().map(|x| x.0).collect::<Vec<_>>(), vec![1, 2, 1]);
        assert_eq!(inc[2].1, Some(0.1f32));
    }

    #[test]
    fn test_out_edges_empty_for_unknown_vertex() {
        let g = GPUEdgeBlockGraph::new();
        assert!(g.out_edges(12345).is_empty());
        assert!(g.get_edges(1, 2).is_empty());
        assert!(g.in_edges(12345).is_empty());
    }

    #[test]
    fn test_parallel_edges_survive_save_load() {
        // 持久化格式**一个字没改**（写的仍是"边数据条数"，只是现在它包含平行边），
        // 所以旧文件读得进来、新文件平行边不丢。
        let path = "/tmp/test_edgeblock_parallel.bin";
        let _ = std::fs::remove_file(path);

        let mut g = GPUEdgeBlockGraph::new();
        g.add_edge_with_props(1, 2, 0.9, props("属于"));
        g.add_edge_with_props(1, 2, 0.8, props("包含"));
        g.save(path).expect("save failed");
        let h = GPUEdgeBlockGraph::open(path).expect("open failed");

        assert_eq!(h.total_edges, 2);
        assert_eq!(h.out_neighbors(1), vec![2, 2]);
        assert_eq!(h.get_edges(1, 2).len(), 2, "平行边必须往返不丢");
        assert_eq!(h.out_edges(1)[1].2.get("relation"),
                   Some(&PropertyValue::String("包含".into())));
        let _ = std::fs::remove_file(path);
    }

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

    #[test]
    fn test_remove_edge() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(1, HashMap::new());
        g.add_vertex(2, HashMap::new());
        g.add_vertex(3, HashMap::new());
        g.add_edge(1, 2, 1.0);
        g.add_edge(1, 3, 1.0);
        g.add_edge(2, 3, 1.0);

        assert_eq!(g.total_edges, 3);

        // 删除 1→2
        let removed = g.remove_edge(1, 2);
        assert!(removed);
        assert_eq!(g.total_edges, 2);
        assert_eq!(g.out_neighbors(1), vec![3]);
        assert_eq!(g.in_neighbors(2), Vec::<u64>::new());

        // 删除不存在的边
        let removed = g.remove_edge(1, 2);
        assert!(!removed);
        assert_eq!(g.total_edges, 2);
    }

    #[test]
    fn test_remove_vertex() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(1, HashMap::new());
        g.add_vertex(2, HashMap::new());
        g.add_vertex(3, HashMap::new());
        g.add_edge(1, 2, 1.0);
        g.add_edge(1, 3, 1.0);
        g.add_edge(2, 3, 1.0);
        g.add_edge(3, 1, 1.0);

        assert_eq!(g.total_edges, 4);

        // 删除顶点 1
        let props = g.remove_vertex(1);
        assert!(props.is_some());
        assert_eq!(g.get_vertex(1), None); // 顶点被删除
        assert_eq!(g.out_neighbors(1), Vec::<u64>::new()); // 所有出边清除
        assert_eq!(g.in_neighbors(1), Vec::<u64>::new()); // 所有入边清除

        // 顶点 2 不再有 1→2 的入边
        assert_eq!(g.in_neighbors(2), Vec::<u64>::new());
        // 但 2→3 还在
        assert_eq!(g.out_neighbors(2), vec![3]);
    }

    #[test]
    fn test_remove_edge_across_blocks() {
        let mut g = GPUEdgeBlockGraph::new();
        let center = g.add_vertex(1, HashMap::new());

        // 添加 64 条边（跨 2 个 block）
        for i in 0..64u64 {
            let id = 100 + i;
            g.add_vertex(id, HashMap::new());
            g.add_edge(1, id, 1.0);
        }
        assert_eq!(g.total_edges, 64);

        // 删除中间一条边
        g.remove_edge(1, 130);
        assert_eq!(g.total_edges, 63);
        let neighbors = g.out_neighbors(1);
        assert!(!neighbors.contains(&130));
        assert!(neighbors.contains(&100));
        assert!(neighbors.contains(&163));
    }

    #[test]
    fn test_edge_properties() {
        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(1, HashMap::new());
        g.add_vertex(2, HashMap::new());

        let mut props = HashMap::new();
        props.insert("label".to_string(), PropertyValue::String("knows".to_string()));
        props.insert("since".to_string(), PropertyValue::Int(2024));
        g.add_edge_with_props(1, 2, 0.75, props);

        // 验证边属性
        let ed = g.get_edge(1, 2).expect("edge should exist");
        assert!((ed.weight - 0.75).abs() < 1e-6);
        assert_eq!(ed.properties["label"], PropertyValue::String("knows".to_string()));
        assert_eq!(ed.properties["since"], PropertyValue::Int(2024));

        // 删除后丢失
        g.remove_edge(1, 2);
        assert!(g.get_edge(1, 2).is_none());
    }

    #[test]
    fn test_edge_properties_roundtrip() {
        let path = "/tmp/test_edge_props.bin";
        let _ = std::fs::remove_file(path);

        let mut g = GPUEdgeBlockGraph::new();
        g.add_vertex(1, HashMap::new());
        g.add_vertex(2, HashMap::new());
        g.add_edge(1, 2, 0.5);

        let mut props = HashMap::new();
        props.insert("weight_tag".to_string(), PropertyValue::String("half".to_string()));
        g.add_edge_with_props(2, 1, 0.5, props);

        g.save(path).unwrap();
        let g2 = GPUEdgeBlockGraph::open(path).unwrap();

        let ed1 = g2.get_edge(1, 2).unwrap();
        assert!((ed1.weight - 0.5).abs() < 1e-6);
        assert!(ed1.properties.is_empty());

        let ed2 = g2.get_edge(2, 1).unwrap();
        assert!((ed2.weight - 0.5).abs() < 1e-6);
        assert_eq!(ed2.properties["weight_tag"], PropertyValue::String("half".to_string()));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_walk() {
        let mut g = GPUEdgeBlockGraph::new();
        for i in 0..5u64 { g.add_vertex(i, HashMap::new()); }
        g.add_edge(0, 1, 1.0); g.add_edge(0, 2, 1.0);
        g.add_edge(1, 3, 1.0); g.add_edge(2, 4, 1.0); g.add_edge(3, 4, 1.0);

        let mut visited_edges = Vec::new();
        let visited = g.walk(0, 2, |from, d, to| {
            visited_edges.push((from, d, to));
        });

        assert_eq!(visited.len(), 5); // 0,1,2,3,4
        assert!(!visited_edges.is_empty());
    }

    #[test]
    fn test_subgraph() {
        let mut g = GPUEdgeBlockGraph::new();
        for i in 0..5u64 { g.add_vertex(i, HashMap::new()); }
        g.add_edge(0, 1, 1.0); g.add_edge(0, 2, 1.0);
        g.add_edge(1, 3, 1.0); g.add_edge(3, 4, 1.0);

        let (vertices, edges) = g.subgraph(&[0], 2);
        assert_eq!(vertices.len(), 4); // 0,1,2,3
        assert_eq!(edges.len(), 3);    // 0→1, 0→2, 1→3
    }

    #[test]
    fn test_find_paths() {
        let mut g = GPUEdgeBlockGraph::new();
        for i in 0..5u64 { g.add_vertex(i, HashMap::new()); }
        g.add_edge(0, 1, 1.0); g.add_edge(1, 2, 1.0);
        g.add_edge(0, 2, 1.0); g.add_edge(2, 3, 1.0);

        // 找所有 2-hop 路径
        let paths = g.find_paths(None, None, 2);
        // 应该有 0→1→2 和 0→2→3 等
        assert!(!paths.is_empty());
    }
}
