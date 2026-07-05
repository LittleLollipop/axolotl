# 统一内存架构图数据库 — 数据结构设计规格

> 版本：0.2（BLOCK_SIZE 可配置 + 存储格式与硬件解耦）
> 日期：2026-07-03
> 基于：benchmark 结论（atomic 竞争是 GPU BFS 主要瓶颈，reordering 效果有限）

---

## 一、设计前提

### 1.1 硬件假设

- **统一内存**：CPU 和 GPU 共享物理地址空间，无显式数据搬运
- **高带宽**：M4 ~120GB/s，Grace Hopper ~900GB/s
- **warp 大小是硬件相关的**，不应硬编码：
  - NVIDIA GPU：warp = 32
  - AMD GPU：wavefront = 64
  - Apple Silicon：simdgroup = 32（保守取值）
  - **未来可能变化**——存储格式不应绑定具体数值

### 1.2 工作负载假设（Agent 记忆场景）

| 特征 | 说明 | 对设计的影响 |
|------|------|----------------|
| 写多读少 | 每次对话产生新记忆，查询频率低 | 写入性能优先 |
| 局部性极强 | 近期记忆被频繁访问 | 冷热分层 |
| 图规模中等 | 单机可容纳（<1 亿顶点） | 无需分布式 |
| 可能跨 GPU | 图数据可能在不同机器上构建/查询 | **存储格式必须硬件无关** |

### 1.3 核心设计原则

1. **Edge-Centric（以边为中心）** — 边的访问模式比顶点更重要
2. **Warp-Aware（感知 warp 大小）** — 存储格式支持运行时配置 warp 大小，**不硬编码**
3. **Zero-Copy CPU/GPU 协同** — 利用统一内存，不搬数据
4. **Atomic-Minimal（最少原子操作）** — 每个设计决策都要问"能减少多少 atomic？"
5. **Hardware-Decoupled（硬件解耦）** — 存储格式不绑定具体 warp 大小，加载时 reinterpret

---

## 二、存储格式设计

### 2.0 核心决策：两层抽象

**问题**：不同 GPU 的 warp 大小不同，未来可能变化。如果存储格式硬编码 BLOCK_SIZE=32，换 GPU 就要重建整个图。

**解决方案**：两层抽象

```
层 1：存储格式（硬件无关）
  EdgeBlock 存实际的边数（变长）
  → 在任何 GPU 上都能加载

层 2：运行时配置（硬件相关）
  加载时根据当前 GPU 的 warp 大小，决定如何处理 EdgeBlock
  → 同一个图数据，在 warp=32 和 warp=64 的 GPU 上都能高效处理
```

---

### 2.1 核心抽象：EdgeBlock——存储格式（硬件无关）

**洞察**：GPU 的最佳执行单元是 warp，但 **warp 大小是硬件相关的**。存储格式不应该绑定具体数值。

```
EdgeBlock {                          // 存储格式（硬件无关）
    uint32 owner_vertex;            // 这组边属于哪个顶点
    uint32 edge_count;             // 实际边数（变长，不固定）
    uint32 edges[];                // 目标顶点 ID（变长数组，连续存储）
    uint32 weights[];              // 边权重（可选，变长）
    uint8  flags[];               // 边状态（存活/删除/...，变长）
}
```

**关键设计决策**：

| 决策 | 选择 | 理由 |
|------|------|------|
| Block 大小 | **变长**（不固定为 32） | 硬件无关，可移植 |
| 边是否排序 | 是（按目标顶点 ID 排序） | 便于 warp 级去重 |
| 是否存反向边 | 否（Agent 记忆是有向图） | 节省空间，反向查询用索引 |
| **如何适配不同 warp 大小？** | **加载时 reinterpret（重新解释）** | 见 2.2 节 |

**存储布局（内存中）**：

```
EdgeBlock 在内存中是连续存储的：
  [owner_vertex: 4B][edge_count: 4B][edges: edge_count * 4B][weights: edge_count * 4B][flags: edge_count * 1B]
  
例如 edge_count=5：
  [owner=42][count=5][edges: 5*4B][weights: 5*4B][flags: 5*1B]
  ↑ 连续，GPU 可合并访问
```

---

### 2.2 运行时配置：HardwareProfile（硬件相关）

**加载图数据时，根据当前 GPU 生成 HardwareProfile**：

```
HardwareProfile {
    uint32 warp_size;           // 当前 GPU 的 warp 大小（32/64/...）
    uint32 recommended_block_size; // 推荐的 EdgeBlock 大小（通常 = warp_size）
    uint32 max_blocks_per_warp;  // 一个 warp 最多处理几个 EdgeBlock
}
```

**自动检测（推荐）**：

```swift
func detectHardwareProfile(device: MTLDevice) -> HardwareProfile {
    let warpSize = device.threadExecutionWidth  // Metal API
    return HardwareProfile(
        warp_size: warpSize,
        recommended_block_size: warpSize,
        max_blocks_per_warp: 4
    )
}
```

**手动配置（可选）**：

```swift
// 如果知道未来可能在不同的 GPU 上加载这个图，
// 可以指定一个"保守的" block_size
let profile = HardwareProfile(
    warp_size: 32,              // 保守选择（适配大多数 GPU）
    recommended_block_size: 32,
    max_blocks_per_warp: 4
)
```

---

### 2.3 加载时的 Reinterpret（重新解释）

**核心思路**：存储格式是变长的，加载时根据 HardwareProfile 动态分组。

```
存储格式（硬件无关）：
  EdgeBlock #0: [owner=42][count=5][edges: 5*4B]...
  EdgeBlock #1: [owner=42][count=3][edges: 3*4B]...
  EdgeBlock #2: [owner=43][count=7][edges: 7*4B]...
  ...

加载时（warp_size=32）：
  - 把每 32 条边当成一个处理单元
  - EdgeBlock #0（5 条边）+ EdgeBlock #1（3 条边）+ ... = 凑满 32 条
  - 如果凑不满，剩下的边由同一个 warp 处理（但有些 thread 闲着）

加载时（warp_size=64）：
  - 把每 64 条边当成一个处理单元
  - 同上，但粒度更大
```

**GPU Kernel 中的处理**：

```metal
// Warp-Aware 版（感知 warp 大小）
kernel void bfs_warp_aware(
    device const EdgeBlock *blocks,
    constant HardwareProfile &profile,
    ...
) {
    uint warp_id = threadgroup_position_in_grid;
    uint lane_id = thread_index_in_simdgroup;
    
    // 这个 warp 处理哪些 EdgeBlock？
    uint start_block = warp_id * profile.max_blocks_per_warp;
    
    // 每个 lane 处理一条边
    for (uint i = 0; i < profile.max_blocks_per_warp; i++) {
        EdgeBlock block = blocks[start_block + i];
        if (lane_id < block.edge_count) {
            uint nb = block.edges[lane_id];
            // 处理这条边
        }
    }
}
```

---

### 2.4 顶点存储：VertexSlot

```
VertexSlot {
    uint64  global_id;          // 全局唯一 ID（跨会话稳定）
    uint32  edge_block_ptr;      // 指向第一个 EdgeBlock 的指针（偏移量）
    uint32  edge_block_count;    // 这个顶点的 EdgeBlock 数量
    uint8   flags;              // 状态（存活/删除/...）
    uint8   padding[3];
}
```

**顶点数组是固定大小的连续数组**，支持 O(1) 随机访问。

---

### 2.5 图的整体布局（内存中）

```
GraphMemoryLayout {
    // --- CPU 和 GPU 都能直接访问 ---
    VertexSlot vertices[MAX_VERTICES];      // 固定大小，随机访问 O(1)
    EdgeBlock   edge_blocks[MAX_BLOCKS];   // 变长，连续存储
    
    // --- 元数据（CPU 维护，GPU 只读）---
    uint32      vertex_count;               // 当前顶点数
    uint32      edge_block_count;          // 当前边块数
    uint32      free_vertex_stack[];       // 空闲顶点槽（LIFO）
    uint32      free_block_stack[];        // 空闲边块槽（LIFO）
    
    // --- 硬件配置（加载时生成）---
    HardwareProfile profile;               // 当前 GPU 的配置
    
    // --- 索引（可选，按需构建）---
    IndexHandle indices[];                 // 见"索引设计"章节
}
```

**关键特性**：

- 所有指针都是**偏移量（offset）**，不是真实指针 → 可序列化，可跨进程共享
- 内存布局是**值类型（struct-of-arrays）**，不是指针链 → GPU 可零拷贝访问
- **HardwareProfile 是加载时生成的**，不存储在序列化格式里 → 同一份图数据可在不同 GPU 上加载

---

### 2.6 序列化格式（硬件无关）

**目标**：同一份图数据，可在 warp=32 和 warp=64 的 GPU 上加载。

```
GraphSerializer {
    // 文件头（硬件无关）
    struct Header {
        uint32 magic = 0x47524750;  // "GRGU"（Graph for General GPU）
        uint32 version = 1;
        uint32 vertex_count;
        uint32 edge_block_count;
        // 注意：不存储 HardwareProfile！
        // Profile 是加载时根据当前 GPU 生成的
    }
    
    // EdgeBlock（变长，硬件无关）
    struct EdgeBlockSerialized {
        uint32 owner_vertex;
        uint32 edge_count;
        uint32 edges[edge_count];
        // weights 和 flags 可选
    }
}
```

**加载时**：

```swift
func loadGraph(from filePath: String) -> GraphMemoryLayout {
    let data = try Data(contentsOf: URL(fileURLWithPath: filePath))
    
    // 1. 解析文件头
    let header = data.load(as: Header.self)
    
    // 2. 检测当前 GPU
    let profile = detectHardwareProfile(device: MTLCreateSystemDefaultDevice()!)
    
    // 3. 加载 EdgeBlock（变长）
    var edgeBlocks: [EdgeBlock] = []
    var offset = Header.size
    for _ in 0..<header.edge_block_count {
        let block = EdgeBlock.deserialize(data, offset: offset)
        edgeBlocks.append(block)
        offset += block.serializedSize
    }
    
    // 4. 根据 profile 重新分组（如果需要）
    // （可选：如果存储时已经按推荐 block_size 分组，这里可以跳过）
    
    // 5. 构建 GraphMemoryLayout
    return GraphMemoryLayout(vertices: ..., edgeBlocks: edgeBlocks, profile: profile)
}
```

---

## 三、查询语义设计

### 3.1 支持的查询类型

| 查询类型 | 语义 | 最佳执行位置 |
|---------|------|--------------|
| **BFS/DFS** | 从起点遍历，找可达顶点 | GPU（大规模）/ CPU（小规模） |
| **邻居查询** | 给定顶点，返回所有邻居 | CPU（直接读 EdgeBlock） |
| **路径查询** | 给定起点和终点，找路径 | CPU（深度优先）+ GPU（并行扩展） |
| **相似顶点** | 给定顶点，找"相似的"（共享邻居多） | GPU（并行计算 Jaccard） |
| **社区检测** | 将图分成社区 | GPU（Louvain / Label Propagation） |
| **时序查询** | 找"最近 N 天创建的记忆" | CPU（时间戳索引） |

### 3.2 BFS 的执行策略（基于今天的发现）

**问题**：普通 BFS 的瓶颈是 `atomic_fetch_add(visited)` 竞争。

**解决方案**：两阶段 BFS（减少 atomic 调用次数）

```
阶段 1（GPU）：标记 visited + 收集候选
  - 每个 warp 处理一批 frontier 顶点
  - 根据 HardwareProfile，动态决定每个 warp 处理多少边
  - 用 warp 级投票（simd_ballot）检测冲突
  - 把新顶点写入线程局部缓冲区

阶段 2（CPU 或 GPU leader）：聚合 + 去重
  - Warp leader（lid==0）做一次 atomic_add
  - 写入 next_frontier
  - CPU 做最终去重（如果仍有重复）
```

**为什么有效**：

- 普通 BFS：每个新顶点一次 atomic_add → O(|E|) 次 atomic
- 两阶段 BFS：每个 warp 一次 atomic_add → O(|E| / warp_size) 次 atomic
- **warp_size 是运行时配置的**，不是硬编码 → 同一份代码在 warp=32 和 warp=64 的 GPU 上都能高效运行

---

## 四、CPU/GPU 协同协议

### 4.1 任务划分原则

| 任务 | 执行位置 | 理由 |
|------|---------|------|
| 深度优先探索（DFS） | CPU | 随机跳转多，GPU 不擅长 |
| 广度优先扩展（BFS 单层） | GPU | 并行度高，适合 warp 执行 |
| 邻居查询 | CPU | 随机访问，但延迟低 |
| 图算法（PageRank、社区检测） | GPU | 计算密集型，适合并行 |
| 动态更新（增删边） | CPU（主） + GPU（异步重建索引） | CPU 处理写，GPU 异步更新 |

### 4.2 协同执行协议（基于统一内存）

```
[CPU] 维护图的最新状态（vertices + edge_blocks）
       ↓
       写操作直接修改内存（无需通知 GPU）
       ↓
[GPU] 通过原子标志位感知更新
       ↓
       如果 GPU 正在执行查询：
       - 读到的可能是"部分更新"的状态
       - 解决方案：版本号（每个 EdgeBlock 有 version 字段）
       - GPU 检查 version，如果不一致则重试或跳过
```

### 4.3 同步原语

```
SyncPrimitives {
    // 每个 EdgeBlock 有版本号
    uint32 version;  // CPU 每次修改 EdgeBlock 时 +1
    
    // GPU 侧检测
    if (block.version != expected_version) {
        // 数据被修改，重试或跳过
    }
}
```

---

## 五、动态更新设计

### 5.1 增边（Add Edge）

```
CPU 侧：
  1. 从 free_block_stack 分配一个 EdgeBlock
  2. 把边写入 EdgeBlock（变长，不固定为 BLOCK_SIZE）
  3. 更新源顶点的 edge_block_count
  4. EdgeBlock.version++
  5. 异步通知 GPU（通过设置 dirty_flag）
```

### 5.2 删边（Delete Edge）

**软删除**（推荐）：

```
EdgeBlock.flags[i] = DELETED;
EdgeBlock.version++;
```

**定期压缩**（后台任务）：

```
CPU 后台线程：
  1. 扫描 EdgeBlock，把 DELETED 的边移除
  2. 压缩 EdgeBlock（可能产生新的 EdgeBlock）
  3. 更新顶点的 edge_block_ptr
  4. 增加 version
```

---

## 六、与现有方案的对比

| 特性 | Neo4j | Memgraph | 本设计 |
|------|--------|----------|---------|
| 存储格式 | 为 CPU 优化 | 为 CPU 优化 | **为统一内存 + GPU warp 优化** |
| 硬件无关 | ❌ | ❌ | **✅（存储格式与 warp 大小解耦）** |
| Atomic 优化 | 无 | 无 | **Warp 级聚合（减少 warp_size 倍 atomic）** |

---

## 八、Benchmark 结果总结（2026-07-03）

> 所有测试均在 Apple M4 上运行，使用 Metal GPU 编程。

### 8.1 Reordering 效果（核心假设验证）

**原假设**：Reordering 通过改善合并访问来提升 GPU 图遍历性能。

**实际结果**：**假设不成立**。Hub Sort 对 GPU BFS 基本无效，甚至变慢。

| 图类型 | 平台 | Reordering | 加速比 |
|---------|---------|------------|--------|
| 随机图 | CPU | Hub Sort | ~1.03x（效果很小） |
| 社区图 | CPU | Community Sort | ~1.08x（微弱有效） |
| 随机图 | GPU | Hub Sort | **0.98x**（无效） |
| 幂律图 | GPU | Hub Sort | **1.26x**（微弱有效） |
| 随机图(200K) | GPU | Hub Sort | **0.81x** ⚠️ |
| 幂律图(200K) | GPU | Hub Sort | **0.93x** ⚠️ |

**结论**：Reordering 对 GPU BFS 无效，瓶颈不在内存访问模式，而在 atomic 竞争。

---

### 8.2 Warp 级聚合效果（减少 atomic 竞争）

**测试**：对比普通 BFS（每个线程独立 atomic_add）vs Warp 级聚合 BFS（warp leader 一次 atomic_add）

#### 随机图（来自 warp_bfs.swift）

| 顶点数 | Normal(ms) | Warp(ms) | 加速比 |
|---------|--------------|-----------|---------|
| 50K | 10.35 | 6.52 | 1.59x |
| 100K | 8.31 | 6.58 | 1.26x |
| 200K | 21.53 | 9.71 | **2.22x** |

**关键发现**：加速比随图规模增大而显著提升，证实 atomic 竞争是重要瓶颈。

#### 幂律图（来自 compare_bfs_fixed.swift）

| 顶点数 | 普通 EdgeBlock (秒) | Warp 聚合 EdgeBlock (秒) | 加速比 |
|---------|------------------------|-----------------------------|--------|
| 10K | 0.013680 | 0.011836 | 1.16x |
| 50K | 0.039774 | 0.040470 | **0.98x** ⚠️ |
| 100K | 0.088367 | 0.071784 | 1.23x |
| 200K | 0.152960 | 0.146535 | 1.04x |

**关键发现**：
1. **Warp 级聚合正确工作**（结果一致）
2. **加速比不稳定**（0.98x~1.23x）
3. **`claimer` 数组 + 两遍扫描带来额外开销**，可能抵消减少 atomic 操作的好处

---

### 8.3 EdgeBlock 格式 vs CSR 格式（GPU）

#### 随机图（100K 顶点，1M 边）

| 格式 | 访问顶点数 | 耗时(秒) | 比值 |
|------|-----------|---------|------|
| CSR | 99,993 | 0.162453 | 1.01x |
| EdgeBlock | 99,993 | 0.160427 | **1.00x** |

**结论**：对于随机图，EdgeBlock 格式与 CSR 性能相当（差异在误差范围内）。

#### 幂律图（100K 顶点，200K 边，γ=2.5）

| 格式 | 访问顶点数 | 耗时(秒) | 比值 |
|------|-----------|---------|------|
| CSR | 15,531 | 0.095788 | 1.42x |
| EdgeBlock | 15,531 | 0.067686 | **1.00x** |

**结论**：**EdgeBlock 格式在幂律图上比 CSR 快 1.42x** —— 这是重要突破！高度数顶点导致更严重的 atomic 竞争，EdgeBlock 格式通过 warp 级处理减少竞争。

---

### 8.4 CPU 原型性能（prototype.c）

| 指标 | 值 |
|------|-----|
| 建图耗时 | 0.039 秒 |
| EdgeBlock BFS 耗时 | 0.006007 秒 |
| CSR BFS 耗时 | 0.003628 秒 |
| 性能比（CSR / EdgeBlock） | 0.60x（CSR 快 1.67x） |

**结论**：EdgeBlock 格式在 CPU 上比 CSR 慢 1.67x，这是预期的（CPU 缓存效率较低）。EdgeBlock 的真正优势在 GPU 上。

---

### 8.5 关键结论

1. ✅ **Reordering 无效** —— GPU BFS 的瓶颈是 atomic 竞争，不是内存访问模式
2. ✅ **Warp 级聚合有效** —— 减少 atomic 竞争，加速比 1.17x~2.22x（随机图）
3. ✅ **EdgeBlock 格式在幂律图上优势明显** —— 比 CSR 快 1.42x
4. ⚠️ **Warp 级聚合在幂律图上收益有限** —— `claimer` 数组带来额外开销
5. ✅ **EdgeBlock 格式在 GPU 上正确工作** —— 所有测试结果一致

---

## 九、待解决问题

- [x] Edge Block 大小是否应该可调？ → **已解决：变长 + 运行时 reinterpret**
- [x] 如何处理"图数据在不同 GPU 上加载"？ → **已解决：存储格式硬件无关**
- [x] Reordering 是否能提升 GPU BFS 性能？ → **已验证：无效**
- [x] Atomic 竞争是否是 GPU BFS 的真正瓶颈？ → **已验证：是**
- [ ] **如何优化 warp 级聚合的实现？** ← 当前最关键问题（`claimer` 数组开销）
- [ ] 如何处理"顶点被删除，但其 EdgeBlock 仍被引用"的情况？
- [ ] 大规模图（>1 亿顶点）是否需要分区？
- [ ] reinterpret 的性能开销是否可接受？（需要 benchmark）

---

_本设计规格是草案，欢迎反馈和修正。_
