# Axolotl-RS 项目核心原则

## 1. 项目目标

**构建"同一内存下 CPU/GPU 协同工作的高性能图数据库"**

- ✅ **不是**纯 CPU 图数据库
- ✅ **不是**纯 GPU 图数据库
- ✅ **是** CPU 和 GPU 协同工作（利用 Apple M4 的统一内存架构）

---

## 2. 技术栈

| 组件 | 技术 | 说明 |
|------|------|------|
| **语言** | Rust | 内存安全，性能媲美 C++ |
| **GPU 加速** | Metal (macOS) | 使用 `metal-rs` 库 |
| **图结构** | EdgeBlock (自定义) | 优化 GPU 访问（coalesced） |
| **对比基准** | CSR 格式 | 用于性能对比 |

---

## 3. 核心算法实现原则

### 3.1 增量 PageRank（目标：244x 加速比）

**CPU 负责**：
1. 管理"受影响顶点队列"
2. 构建反向邻接表（用于影响传播）
3. 检查收敛，更新受影响集合

**GPU 负责**：
1. 计算受影响顶点的 PR 值
2. 使用 Metal 计算着色器

**协同工作流程**：
```
1. CPU：找出初始受影响顶点（添加边后）
2. GPU：计算这些顶点的新 PR 值
3. CPU：检查收敛，找出新的受影响顶点
4. 重复 2-3，直到收敛
```

**参考实现**：`/tmp/axolotl_tmp/Experiments/incremental_pagerank.swift`

---

### 3.2 增量 BFS（目标：80x 加速比）

**CPU 负责**：
1. 管理队列
2. 检查收敛

**GPU 负责**：
1. 并行扩展邻居

**参考实现**：`/tmp/axolotl_tmp/Experiments/incremental_bfs.swift`

---

### 3.3 增量 SSSP（目标：84x 加速比）

**CPU 负责**：
1. 管理优先队列
2. 检查收敛

**GPU 负责**：
1. 并行松弛边

**参考实现**：`/tmp/axolotl_tmp/Experiments/incremental_sssp.swift`

---

### 3.4 增量 Connected Components（目标：74x 加速比）

**CPU 负责**：
1. Union-Find 数据结构
2. 合并连通分量

**GPU 负责**：
1. 并行查找根节点

**参考实现**：`/tmp/axolotl_tmp/Experiments/incremental_connected_components.swift`

---

### 3.5 增量 Triangle Counting（目标：486x 加速比）

**CPU 负责**：
1. 管理受影响顶点集合
2. 更新三角形计数

**GPU 负责**：
1. 并行计算三角形

**参考实现**：`/tmp/axolotl_tmp/Experiments/incremental_triangle_counting.swift`

---

## 4. 数据结构

### 4.1 EdgeBlock（GPU 友好）

**设计要点**：
- Block 大小 = 32（一个 cache line 的大小）
- 顶点按度数分成多个 block
- 所有 block 连续存储（提高缓存命中率）
- GPU 访问时 coalesced（合并内存访问）

**参考实现**：`/tmp/axolotl_tmp/Experiments/incremental_pagerank.swift`（EdgeBlock 结构体）

---

### 4.2 CSR 格式（对比基准）

**设计要点**：
- 标准 CSR 格式
- 用于性能对比

**参考实现**：`/tmp/axolotl_tmp/Experiments/incremental_pagerank.swift`（CSRGraph 结构体）

---

## 5. 实现检查清单

在**每次实现算法前**，检查：

- [ ] 是否使用了 Metal GPU 加速？
- [ ] CPU 是否只管理队列/收敛检查？
- [ ] GPU 是否计算了受影响顶点？
- [ ] 是否严格按照 Swift 版本实现？
- [ ] 是否测试了性能加速比？

---

## 6. 常见错误

### ❌ 错误 1：写"纯 CPU"的增量算法

**错误代码**：
```rust
// ❌ 错误：纯 CPU 增量算法
fn update(&mut self, graph: &GraphDB, added_edges: &[(u64, u64)]) {
    // 在 CPU 上计算所有顶点的 PR 值
    for &vertex_id in graph.vertices.keys() {
        // ...
    }
}
```

**正确做法**：
```rust
// ✅ 正确：CPU/GPU 协同
fn update(&mut self, graph: &GraphDB, added_edges: &[(u64, u64)]) {
    // 1. CPU：找出受影响顶点
    let affected = find_affected_vertices(graph, added_edges);
    
    // 2. GPU：计算这些顶点的 PR 值
    let gpu_result = self.gpu.compute_incremental(&graph, &affected);
    
    // 3. CPU：检查收敛
    let new_affected = check_convergence(&self.pr, &gpu_result);
}
```

---

### ❌ 错误 2：忘记处理 dead ends

**问题**：PageRank 中，出度为 0 的顶点（dead ends）的贡献应该分给所有顶点。

**正确做法**：
1. 计算所有 dead end 的总 PR 值
2. 将贡献平均分给所有顶点

---

### ❌ 错误 3：PR 值之和不等于 1.0

**问题**：PageRank 算法中，所有顶点的 PR 值之和应该接近 1.0。

**检查方法**：
```rust
let sum: f64 = pr.values().sum();
assert!((sum - 1.0).abs() < 0.01, "PR 值之和应该接近 1.0");
```

---

## 7. 性能目标

| 算法 | 目标加速比 | 当前状态 |
|------|-----------|---------|
| 增量 PageRank | **244x** | ❌ 未实现 |
| 增量 BFS | **80x** | ❌ 未实现 |
| 增量 SSSP | **84x** | ❌ 未实现 |
| 增量 Connected Components | **74x** | ❌ 未实现 |
| 增量 Triangle Counting | **486x** | ❌ 未实现 |

---

## 8. 下一步

1. ✅ 创建本项目核心原则文档
2. ⏳ 实现正确的增量 PageRank（CPU/GPU 协同）
3. ⏳ 实现其他增量算法
4. ⏳ 性能测试（对比 igraph、NetworkX）
5. ⏳ 存储层实现
6. ⏳ Python 绑定

---

**最后提醒**：
- **每次实现前先读这个文档！**
- **不要写"纯 CPU"的代码！**
- **严格按照 Swift 版本实现！**
