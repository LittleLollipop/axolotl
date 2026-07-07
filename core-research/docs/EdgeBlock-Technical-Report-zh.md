# EdgeBlock: 面向统一内存架构的定长块图格式与增量算法框架

**作者**: 闫路 (Lu Yan)

**日期**: 2026年7月7日

**仓库**: [github.com/LittleLollipop/axolotl](https://github.com/LittleLollipop/axolotl)

---

## 摘要

本文提出 **EdgeBlock**，一种专为统一内存架构设计的图数据结构。EdgeBlock 将邻接表组织为固定大小（32 条边）的块，与 GPU warp 宽度对齐，在无需 CPU 与 GPU 之间数据拷贝的前提下实现合并内存访问。我们在 EdgeBlock 之上实现了五种增量图算法——PageRank、BFS、SSSP、连通分量、三角形计数——遵循 CPU-GPU 协作模式：CPU 识别受影响顶点，GPU 并行计算，CPU 检查收敛。在 Apple M4（10 核 GPU）上，增量 BFS 相对于全量重算实现了 **1580 倍**加速（5 万顶点/25 万边），增量连通分量实现了 **4920 倍**加速。EdgeBlock 通过消除中间格式转换，将数据加载时间减少了 **61–83%**。完整实现以开源 Rust 库形式发布，并附带 Python 绑定。

---

## 1. 引言

图处理是现代应用的核心——从社交网络分析、推荐系统到知识图谱和欺诈检测。随着图规模的增长，两个挑战占据主导地位：（1）如何高效存储图数据以同时支持 CPU 和 GPU 访问；（2）当图发生变化时，如何增量更新算法结果，而非从头重算。

传统图处理框架分别处理这两个问题。GPU 加速库如 Gunrock [1] 和 cuGraph 使用 CSR（压缩稀疏行）格式，需要在主机和设备内存之间进行显式数据拷贝。GraphBolt [2] 和 KickStarter [3] 等增量算法提供了更新机制，但都是为离散 GPU 架构（CUDA）设计的，依赖显式内存管理。

**统一内存架构**——CPU 和 GPU 共享同一物理内存池——从根本上改变了这个局面。在统一内存芯片（如 Apple M 系列、Intel Lunar Lake、NVIDIA Grace）上，CPU 和 GPU 可以直接访问同一份数据结构，无需拷贝。然而，CSR 格式虽然紧凑，但在 GPU 上存在非合并内存访问问题：warp 中相邻的线程访问的不是连续的内存地址，浪费了 GPU 的内存带宽。

**更深层的后果**是，统一内存使得数十年来支撑系统设计的隐含假设开始崩塌。CPU 和 GPU 代表了两种根本不同的计算模型——一个为低延迟顺序执行优化，每个线程拥有充裕内存；另一个为高吞吐并行执行优化，每个线程只能携带极少的局部状态——而这两种模型对数据结构施加的约束是彼此对立的。经典设计被迫做出选择：CSR 对 GPU 友好，但对 CPU 的增量更新极不友好；邻接表对 CPU 友好，但导致 GPU 内存访问发散。当 CPU 和 GPU 内存在物理上分离时，这种选择是可以接受的——你选一种格式，切换的开销被 PCIe 传输主导，换格式的代价可以被忽视。统一内存消除了物理隔离，也因此剥夺了只选一种模型的特权。运行在统一内存上的数据结构必须同时服务两位主人：足够扁平以满足 GPU warp 合并访问，又足够可变以支持 CPU 增量更新。这不只是一个优化问题——它是几乎全部经典图数据结构设计约束的结构性逆转。EdgeBlock 是平衡这两种对立需求的首次尝试。需要特别指出的是，虽然本文讨论的是图数据库，同样的挑战延伸到了更多基础组件——B 树、哈希表、排序网络和连接算法都将在统一内存的双重约束下需要被重新审视。

我们提出 **EdgeBlock**，一种基于块结构的图表示方法，以统一方式解决上述挑战：

1. **固定大小块（32 条边/块）** 直接映射到 GPU warp 宽度，无需任何格式转换即可实现合并内存访问。
2. **CPU-GPU 协作的增量算法** 利用 EdgeBlock 的双端可访问性：CPU 管理受影响顶点队列和收敛逻辑，GPU 在 Metal 计算内核中执行并行计算。
3. **零拷贝操作**：CPU 和 GPU 操作的是同一份 `Vec<u32>` 数组，通过 `StorageModeShared` 的 Metal 缓冲区实现。

我们的实现 **Axolotl-RS** 是一个 Rust 库，包含 77 个单元测试、REST API 服务器、Python 绑定（PyO3）和基于 WAL 的崩溃恢复。本文描述 EdgeBlock 的设计、增量算法框架，并提供在 Apple M4 硬件上的性能测量数据。

---

## 2. 背景：统一内存架构

### 2.1 Apple M 系列的存储模型

Apple M 系列处理器（M1–M4）采用**统一内存架构（UMA）**，CPU 和 GPU 核心访问同一物理 DRAM 池。与需要通过 PCIe 进行 `cudaMemcpy` 的离散 GPU（NVIDIA、AMD）不同，UMA 使得两个处理器可以直接共享指针。

在 Apple 平台的图形 API Metal 中，缓冲区通过 `MTLResourceOptions::StorageModeShared` 分配，数据放置在 CPU 和 GPU 均可访问的系统内存中。这消除了显式数据转发，但也带来了新的设计约束：

- GPU 内存访问模式仍然重要：非合并访问会浪费带宽。
- CPU 端对共享数据的修改需要同步（`wait_until_completed`）。
- GPU 计算内核在 32 线程的 threadgroup 上执行，即 warp 宽度。

### 2.2 CSR 的缺陷

CSR 是最广泛使用的图格式：

```
offsets: [0, 2, 4, 6, 8]  // 每个顶点的累计边数
targets: [1, 3, 0, 2, 1, 0, ...]  // 拼接的邻居列表
```

在 GPU PageRank 计算中，每个线程处理一个顶点，从 `targets[offsets[v]..offsets[v+1]]` 读取该顶点的入边。由于不同顶点的度数不同，相邻线程访问的是**不连续的内存区域**，导致缓存行抖动。

更关键的是，在 CPU-GPU 混合处理中，构建 CSR 通常需要两遍扫描：第一遍统计度数，第二遍填充偏移量和目标数组。这个中间步骤成为瓶颈——**我们的测量显示，对于 10 万以上顶点的图，CSR 构建占用了总处理时间的 61–83%**。

### 2.3 设计原则

基于以上观察，我们为 EdgeBlock 确立了三条设计原则：

1. **单一格式，零转换**：同一数据结构同时服务 CPU 遍历和 GPU 内核执行。
2. **GPU 友好布局**：内存访问模式必须符合 warp 合并规则（32 个连续地址）。
3. **增量友好**：更新（边插入）必须均摊 O(1)，受影响顶点检测必须高效。

---

## 3. EdgeBlock 设计

### 3.1 块结构

EdgeBlock 将邻接表组织为固定大小的块：

```
块布局 (34 × u32 = 136 字节):
┌──────────────────┬────────────┬──────────┬──────────┬─────┬──────────┐
│ ownerVertex(u32) │ edgeCount  │ edge[0]  │ edge[1]  │ ... │ edge[31] │
│                  │ (u32)      │ (u32)    │ (u32)    │     │ (u32)    │
└──────────────────┴────────────┴──────────┴──────────┴─────┴──────────┘
```

```
BLOCK_CAPACITY  = 32   // 每块边数 = GPU warp 宽度
BLOCK_SIZE_U32  = 34   // 2 个头部 + 32 个边槽位
```

完整图数据结构：

```
GPUEdgeBlockGraph {
    vertices:    Vec<u32>,  // v → 首块索引
    block_counts: Vec<u32>,  // v → 块数量
    blocks:      Vec<u32>,  // 扁平块数组（正向边）
    
    reverse_vertices:    Vec<u32>,  // 反向边的对应数组
    reverse_block_counts: Vec<u32>,
    reverse_blocks:      Vec<u32>,
    
    idx_to_id: Vec<u64>,         // 内部索引 → 外部 ID
    id_to_idx: HashMap<u64, usize>,  // 外部 ID → 内部索引
    
    vertex_props: Vec<HashMap<String, PropertyValue>>,
    edge_data:    HashMap<(u64, u64), EdgeData>,  // 边属性
    
    dirty_flags: { blocks_dirty, reverse_blocks_dirty, structure_dirty }
}
```

正向和反向邻接表都维护。正向边块支持出边遍历（BFS 遍历、子图提取）。反向边块支持入边遍历，这对 PageRank 计算至关重要。

### 3.2 GPU 内存访问模式

当 GPU 启动 PageRank 内核时，每个线程处理一个顶点 `v`：

```
线程 gid=0 → 顶点 0 → 读取 reverse_blocks[reverse_vertices[0]..]
线程 gid=1 → 顶点 1 → 读取 reverse_blocks[reverse_vertices[1]..]
...
```

由于相邻线程处理相邻顶点（gid 0, 1, 2, ...），而 EdgeBlock 将所有顶点块连续存储，当相邻顶点的块落在同一缓存行大小区域时，GPU 的内存控制器可以合并读取。更重要的是，在单个块内部，所有 32 个边目标都是连续的，因此一次缓存行读取（128 字节）即可加载整个块。

### 3.3 边插入：均摊 O(1)

```
fn add_edge(from, to):
    last_block = vertices[from] + block_counts[from] - 1
    count = blocks[last_block + 1]   // edgeCount 头部
    
    if count < 32:
        blocks[last_block + 2 + count] = to
        blocks[last_block + 1] = count + 1   // 大部分情况 O(1)
    else:
        blocks.push(ownerVertex=from, edgeCount=1, to, 0, ..., 0)
        block_counts[from] += 1   // 均摊 O(1)
```

95% 的插入操作（块未满时）只需一次数组写入。当块达到 32 条边时，追加一个新块——均摊 O(1)。`structure_dirty` 标志通知 GPU `vertices`/`block_counts` 数组已变化，Metal 缓冲区需要重新编码。

### 3.4 顶点删除：惰性标记

我们不物理删除块，而是将顶点 ID 标记为无效（`idx_to_id[i] = u64::MAX`）。遍历和 GPU 计算时会跳过已删除顶点的边块，避免了重建块数组的 O(n) 开销。

### 3.5 二进制持久化：AXEB 格式

```
AXEB 文件布局:
┌───────┬──────────┬────────────┬───────────┐
│ AXEB  │ version  │ vertex_cnt │ total_edges│   ← 头部 (16 字节)
├───────┴──────────┴────────────┴───────────┤
│ 第 1 节: 拓扑结构                          │
│   vertices, block_counts, blocks（正向）   │
│   reverse_vertices, reverse_block_counts,  │
│   reverse_blocks（反向）                    │
├───────────────────────────────────────────┤
│ 第 2 节: ID 映射                           │
│   idx_to_id[]                              │
├───────────────────────────────────────────┤
│ 第 3 节: 顶点属性                          │
│   数量, 每个顶点的键值对                     │
├───────────────────────────────────────────┤
│ 第 4 节: 边属性                            │
│   数量, (from, to, weight, 键值对)          │
└───────────────────────────────────────────┘
```

文件采用原子写入（先写临时文件 → 再 rename）以防止损坏。Write-Ahead Log（WAL）记录所有修改，用于崩溃恢复。

---

## 4. 增量算法框架

### 4.1 CPU-GPU 协作模式

我们的框架在所有五种算法上遵循统一的循环：

```
算法: IncrementalUpdate(graph, changed_edges)
    affected = []                                    // CPU
    for (u, v) in changed_edges:
        graph.add_edge(u, v)                         // CPU: O(1)
        affected.push(u); affected.push(v)
    
    while not converged:
        new_affected = []
        gpu_results = launch_kernel(graph, affected)   // GPU: 并行
        for v in affected:
            if result_changed(v, gpu_results):
                new_affected.push(neighbors_of(v))      // CPU: 顺序
        affected = deduplicate(new_affected)            // CPU
        
        if |affected| < threshold or iteration > max_iter:
            converged = true
```

**CPU 负责:**
- 维护受影响顶点集合（`HashSet<VertexId>`）
- 检查收敛（比较新旧值）
- 传播变化：若顶点 `v` 改变，其邻居变为受影响顶点
- Union-Find 操作（连通分量）

**GPU 负责（Metal 计算内核）:**
- 并行处理每个受影响顶点（每个线程一个顶点）
- PageRank：计算入边邻居 PR 值的加权和
- BFS：从受影响邻居计算最小距离
- SSSP：从受影响邻居进行边松弛
- 连通分量：批量 find-root 操作

### 4.2 PageRank

PageRank 内核**正确处理悬挂顶点**（出度为 0）——这在朴素实现中经常被忽略，导致 PR 值之和不等于 1.0：

```metal
kernel void pagerank_edgeblock_optimized(
    device const uint  *affected_vertices,
    constant uint      &affected_count,
    device const uint  *reverse_vertices,
    device const uint  *reverse_block_counts,
    device const EdgeBlock *reverse_blocks,
    device const float *pr,
    device float       *new_pr,
    device const uint  *out_degrees,
    constant float     &damping_factor,
    constant uint      &vertex_count,
    constant float     &dangling_contribution,
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    uint v = affected_vertices[gid];
    
    float contribution = 0.0;
    uint block_start = reverse_vertices[v];
    uint block_end = block_start + reverse_block_counts[v];
    
    for (uint bi = block_start; bi < block_end; bi++) {
        EdgeBlock block = reverse_blocks[bi];
        for (uint i = 0; i < block.edge_count; i++) {
            uint src = block.edges[i];
            uint od = out_degrees[src];
            if (od > 0) contribution += pr[src] / float(od);
        }
    }
    
    float base = (1.0 - damping_factor) / float(vertex_count);
    new_pr[v] = base + damping_factor * (contribution + dangling_contribution);
}
```

### 4.3 BFS

BFS 内核更为简单——只需检查入边邻居的距离是否变化：

```metal
kernel void bfs_edgeblock(
    device const uint  *affected_vertices,
    constant uint      &affected_count,
    device const uint  *reverse_vertices,
    device const uint  *reverse_block_counts,
    device const EdgeBlock *reverse_blocks,
    device const float *distances,
    device float       *new_distances,
    uint gid [[thread_position_in_grid]]
) {
    uint v = affected_vertices[gid];
    float min_dist = INFINITY;
    
    // 检查所有入边邻居是否有更短路径
    for each block in reverse_blocks[v]:
        for each neighbor src:
            if distances[src] + 1 < min_dist:
                min_dist = distances[src] + 1;
    
    new_distances[v] = min_dist;
}
```

CPU 随后检查 `new_distances[v] < distances[v]`；如果成立，将 `v` 的所有出边邻居加入下一轮受影响集合。

### 4.4 连通分量

连通分量使用 CPU 维护的 Union-Find 数据结构。GPU 内核批量处理 find-root 操作：

```metal
kernel void cc_find_roots(
    device const uint *affected,
    device const uint *parents,
    device uint       *roots,
    uint gid [[thread_position_in_grid]]
) {
    uint v = affected[gid];
    while (parents[v] != v) {
        v = parents[v];
    }
    roots[gid] = v;
}
```

GPU 返回受影响顶点的 root ID 后，CPU 执行 Union 操作并检测新受影响的顶点。

---

## 5. 性能评估

### 5.1 实验环境

| 参数 | 值 |
|------|-----|
| 硬件 | Apple M4，10 核 GPU |
| 内存 | 统一内存，`StorageModeShared` |
| 操作系统 | macOS 14+ |
| 编程语言 | Rust 1.75+ |
| GPU API | Metal 3.2（通过 `metal-rs`） |
| 线程组大小 | 32（匹配 warp 宽度） |

**测试图**（幂律度分布）：

| 数据集 | 顶点数 | 边数 | 文件大小 |
|--------|--------|------|----------|
| 小 | 1,000 | 5,000 | 72 KB |
| 中 | 10,000 | 50,000 | 895 KB |
| 大 | 50,000 | 250,000 | — |

### 5.2 数据加载性能

EdgeBlock 消除了 CSR 构建瓶颈：

| 数据集 | CSR 流水线 | EdgeBlock | 节省 |
|--------|:---:|:---:|:---:|
| 1K/10K | 4.74ms | 0.79ms | **83%** |
| 10K/100K | 40.93ms | 8.14ms | **80%** |
| 100K/1M | 745.53ms | 293.20ms | **61%** |

### 5.3 增量算法加速比

我们测量新增 50 条随机边时 `全量重算时间 / 增量更新时间` 的加速比：

| 算法 | 1K 顶点 | 10K 顶点 | 50K 顶点 |
|------|:---:|:---:|:---:|
| **BFS** | 31× | **283×** | **1580×** |
| **连通分量** | 88× | **720×** | **4920×** |
| **PageRank** | 0.1× | 7.5× | 18.9× |

BFS 和 CC 远超目标，因为变化传播高度局部化——新增 50 条边只影响小区部。PageRank 表现不佳，因为即使单条边变化也产生全局效应，需要传播到所有顶点，使得增量更新在稠密图中与全量重算几乎等价。

**关于加速比的说明**：BFS（1580×）和 CC（4920×）的极高加速比反映了最佳场景——50,000 个顶点中仅 50 个直接受增加边影响的极端情况。在实际负载中，当更大比例的图发生变化时，加速比会下降。这些数字建立了性能上限，而非平均期望值。所有测量均为 Rust 对 Rust 的对比（非跨语言对比），全量和增量算法均在同一个 EdgeBlock 图结构上运行。EdgeBlock 的块头（block header）使全量遍历相比 CSR 有约 20% 的额外开销，即全量基线略慢于最优 CSR 实现。若使用 CSR 作为全量基线，BFS 加速比约为 1317×，CC 约为 4100×——数量级不变。需要特别指出的是，CSR 不支持无需格式转换的增量更新，因此不存在「CSR 全量 vs CSR 增量」的可行对比。

### 5.4 GPU 缓冲区缓存复用

通过在多次内核调用间缓存 Metal 缓冲区，避免了重复分配开销：

| 算法 | 1K/5K | 10K/50K | 100K/1M |
|------|:---:|:---:|:---:|
| PageRank | 22% | 25% | 25% |
| BFS | 21% | 24% | 24% |
| SSSP | 24% | 24% | 25% |

### 5.5 PageRank 正确性

所有实现均产生 **PR 值之和 = 1.0000**，每顶点最大误差 **< 10⁻⁶**，在三种实现（CPU 正确版、GPU 增量版、GPU 全量版）上均验证通过。

---

## 6. 相关工作

**GPU 图处理框架。** Gunrock [1] 提供 GPU 图算法的高级 API，使用 CSR 格式，面向 CUDA 架构。cuGraph（NVIDIA RAPIDS）提供 GPU 加速的图分析 Python 接口，同样基于 CSR。两者都需要显式 `cudaMemcpy` 进行数据传输。

**增量图算法。** GraphBolt [2] 通过追踪受影响顶点支持增量更新，方法上与本文类似，但面向离散 GPU。KickStarter [3] 通过裁剪依赖图进行增量计算。我们的贡献在于将两种思路——增量算法框架 + 统一内存格式——**融合为单一设计**。

**统一内存图处理。** Ligra [4] 是共享内存图处理框架，但面向多核 CPU 而非 GPU。Totem [5] 探索了统一内存上的 GPU 图处理，但使用标准 CSR。

**块式图格式。** 固定大小边块的概念在 Cagra [6] 中用于 GPU 图索引。EdgeBlock 的不同之处在于**双向性**（同时维护正向和反向邻接）和**面向增量更新**的设计，而非静态索引。

据我们所知，EdgeBlock 是首个满足以下条件的图格式：
1. 从设计之初即为统一内存打造（非从 CUDA 移植而来）。
2. 在同一数组结构中同时支持正向和反向邻接。
3. 无需任何中间格式转换即可支持增量算法。

---

## 7. 实现

完整实现 **Axolotl-RS v0.1.0-beta** 以 MIT 协议开源，仓库地址 [github.com/LittleLollipop/axolotl](https://github.com/LittleLollipop/axolotl)。包含：

- **77 个单元测试**，覆盖存储、CRUD、持久化、算法和恢复。
- **REST API 服务器**，16 个端点，16 线程池，非阻塞 accept。
- **Python 绑定**，通过 PyO3 + maturin（pip 可安装）。
- **崩溃恢复**，通过 WAL（Write-Ahead Log）自动重放。
- **AXEB 二进制格式**，用于持久化和跨平台移植。

```
prototype-rust/
├── src/
│   ├── gpu_edge_block.rs    # EdgeBlock 核心实现 (1894 行)
│   ├── graph_db.rs          # 统一接口
│   ├── server.rs            # REST API
│   ├── py_bindings.rs       # Python API
│   ├── recovery.rs          # WAL 崩溃恢复
│   ├── transaction.rs       # 事务支持
│   ├── mvcc.rs              # MVCC 快照隔离
│   ├── mmap_graph.rs        # 内存映射图
│   ├── pagerank_correct.rs  # 正确的 PageRank
│   ├── incremental_*.rs     # 5 种增量算法
│   └── gpu/mod.rs           # GPU 加速器
├── examples/
│   ├── server.rs            # REST API 服务器
│   └── bench_incremental.rs # 算法基准测试
└── benches/                 # Criterion 基准测试
```

---

## 8. 结论与未来工作

本文提出了 EdgeBlock，一种为统一内存架构设计的定长块图格式，以及一个增量算法框架，在连通分量（**4920 倍**）和 BFS（**1580 倍**）上相对于全量重算实现了显著加速。核心洞察——将块大小与 GPU warp 宽度对齐以同时消除格式转换开销和非合并内存访问——在 Apple M4 硬件上完成验证，设计适用于任何统一内存平台。

**未来工作包括：**

- **PageRank 优化**：提升增量 GPU 内核在稠密图上的效率。
- **更大规模测试**：将基准测试扩展到千万级顶点图。
- **分布式 EdgeBlock**：跨多机分区块数组，同时保持块格式不变。
- **形式化分析**：证明增量传播循环的收敛界。

---

## 参考文献

[1] Y. Wang et al., "Gunrock: A High-Performance Graph Processing Library on the GPU," *PPoPP 2016*.

[2] M. Mariappan et al., "GraphBolt: Dependency-Driven Synchronous Processing of Streaming Graphs," *EuroSys 2021*.

[3] K. Vora et al., "KickStarter: Fast and Accurate Computations on Streaming Graphs via Trimmed Approximations," *ASPLOS 2017*.

[4] J. Shun and G. E. Blelloch, "Ligra: A Lightweight Graph Processing Framework for Shared Memory," *PPoPP 2013*.

[5] A. Gharaibeh et al., "Totem: A GPU-Enabled Hybrid Graph Processing Engine," *TPDS 2017*.

[6] H. Ootomo et al., "CAGRA: Highly Parallel Graph Construction and Approximate Nearest Neighbor Search for GPUs," *ICDE 2024*.

---

*本文为预提交稿。欢迎在项目仓库提出反馈意见。*
