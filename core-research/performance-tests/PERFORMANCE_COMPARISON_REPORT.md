# Axolotl 性能对比报告（最新版）

> **📋 历史文档 — 最后更新 2026-07-05**
> 
> 数据仍有效。v0.1.0-beta 已发布，新增 Python 绑定和 REST API。

## 测试环境

- **CPU**: Apple M 系列
- **GPU**: Apple GPU (Metal)
- **Rust**: 1.75+
- **Python**: 3.13
- **NetworkX**: 3.6.1

## 测试数据集

| 数据集 | 顶点数 | 边数 | 文件大小 |
|--------|--------|------|----------|
| dataset_1k_10k.edgelist | 1,000 | 10,000 | 72K |
| dataset_10k_100k.edgelist | 10,000 | 100,000 | 895K |
| dataset_100k_1m.edgelist | 100,000 | 1,000,000 | 11M |

## 测试结果

### PageRank

#### 10K 顶点, 100K 边

| 实现 | 平均时间 (ms/iter) | PR 值之和 | 说明 | 加速比 (vs NetworkX) |
|------|-------------------|----------|------|---------------------|
| **Axolotl (CPU, f64)** | **0.070** | 1.0 | ✓ 精确 | **857x** 快 ⚡⚡⚡ |
| **Axolotl (GPU 全量, f32+Kahan)** | **0.51** | 0.999999983 | 误差 1.7e-8 | **119x** 快 ⚡⚡ |
| **Axolotl (GPU 增量, f32+Kahan)** | **0.68** | 0.999999983 | 误差 1.7e-8 | **89x** 快 ⚡⚡ |
| **NetworkX (f64)** | **60.8** (100 iter) | 1.0 | ✓ 精确 | 1x (基准) |

**关键发现:**
- ✅ CPU (f64) PR 值之和 = 1.0（完全正确）
- ✅ GPU (f32 + Kahan 求和 + 归一化) 误差约 **1.7e-8**，对排名几乎无影响
- ✅ Axolotl CPU 比 NetworkX 快 **857x**

#### 100K 顶点, 1M 边

| 实现 | 平均时间 (ms/iter) | PR 值之和 | 说明 | 加速比 (vs CPU) |
|------|-------------------|----------|------|----------------|
| **Axolotl (CPU, f64)** | **3.40** | 1.0 | ✓ 精确 | 1x (基准) |
| **Axolotl (GPU 全量, f32+Kahan)** | **1.52** | 1.000000012 | 误差 1.2e-8 | **2.24x** 快 ✅ |
| **Axolotl (GPU 增量, f32+Kahan)** | **3.67** | 1.000000024 | 误差 2.4e-8 | **0.93x** (略慢) |

**关键发现:**
- ✅ 在大型图上（100K+ 顶点），**GPU 全量版本比 CPU 快 2.24x**
- ✅ 规模化效应明显：图越大 GPU 优势越显著
- ⚠️ GPU 增量版本在 100K 顶点时仍略慢于 CPU（需要优化任务分配策略）

### BFS

#### 10K 顶点, 100K 边

| 实现 | 平均时间 (ms/iter) | 说明 | 加速比 (vs NetworkX) |
|------|-------------------|------|---------------------|
| **Axolotl (CPU)** | **0.24** | 精确 | **20x** 快 ⚡ |
| **Axolotl (GPU)** | **4.7** | 单次 BFS | **1.0x** (与 NetworkX 相当) |
| **NetworkX** | **4.76** (10 iter) | 精确 | 1x (基准) |

**关键发现:**
- ✅ CPU BFS 比 NetworkX 快 **20x**
- ⚠️ GPU BFS 比 CPU 慢 **19.6x**（BFS 不适合 GPU 高并发，任务太简单）

## 问题分析

### 1. PageRank: GPU 精度问题 — 已部分解决 ✅

**方案：f32 + Kahan 求和 + 后归一化**
- Kahan 求和提升了累加精度
- 每轮结束后用 f64 重新计算总和并归一化
- 误差从 ~1e-4 降到 ~1e-8
- **对排名结果几乎没有影响**（PR 值相对顺序不变）

**结论：** 不需要 f64，当前方案精度已足够。

### 2. BFS: GPU 版本性能问题 — 未解决 ⚠️

**原因：**
- BFS 是任务并行（逐个 level 扩展），不适合 GPU 大规模并发
- GPU 启动开销在小型图上占主导

**解决方案（待实现）：**
- 实现 GPU BFS 优化（Frontier 压缩 + Top-Down/Bottom-Up 混合）
- 或接受 BFS 只用 CPU，GPU 留给适合高并发的算法

### 3. GPU 增量 PageRank 性能问题 — 未解决 ⚠️

**现象：** 100K 顶点时 GPU 增量比 GPU 全量慢

**原因：** 增量版本需要 CPU 先计算受影响的顶点列表，再传给 GPU，通信开销大

**解决方案（待研究）：** 让 GPU 自己判断哪些顶点需要更新（类似异步 PageRank）

## 与业界对比

### Axolotl vs NetworkX

| 算法 | 数据集 | Axolotl (CPU, f64) | NetworkX | 加速比 |
|------|--------|-------------------|----------|--------|
| **PageRank** | 10K 顶点 | 0.07ms | 60.8ms (100 iter) | **857x** 快 ⚡⚡⚡ |
| **BFS** | 10K 顶点 | 0.24ms | 4.76ms | **20x** 快 ⚡ |

**结论：** Axolotl 性能远超 NetworkX。

## 结论

**Axolotl 的优势:**
- ✅ PageRank CPU (f64) 性能远超 NetworkX（**857x** 快）
- ✅ PageRank GPU (f32+Kahan) 在 100K+ 顶点时比 CPU 快 **2.24x**
- ✅ BFS CPU 性能远超 NetworkX（**20x** 快）
- ✅ CPU 版本精度完全正确（f64）
- ✅ GPU 版本精度误差极小（~1e-8），对实际应用无影响

**Axolotl 的劣势:**
- ⚠️ BFS GPU 版本性能差（算法不适合 GPU）
- ⚠️ GPU 增量 PageRank 性能不如 GPU 全量（需要优化）

**建议:**
- PageRank：小型图用 CPU (f64)，大型图用 GPU (f32+Kahan) ✅ 已可用
- BFS：只用 CPU ✅ 已可用
- 下一步：实现更适合 GPU 的算法（Betweenness、Connected Components）

---

**测试时间:** 2026-07-05
**测试者:** Lu Yan
**代码位置:** `prototype-rust/examples/performance_comparison.rs`
