# Axolotl

> **📦 Swift 原型 — 历史参考实现**
> 
> 该 Swift 实现为项目早期原型，现已作为参考保留。当前活跃开发在 Rust 版本（`prototype-rust/`）中，v0.1.0-beta 已于 2026-07-07 发布。请以 [根目录 README](../README.md) 为权威文档。

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Swift](https://img.shields.io/badge/Swift-6.0-orange.svg)](https://swift.org)
[![Platform](https://img.shields.io/badge/Platform-macOS%2014+-lightgrey.svg)]()
[![Metal](https://img.shields.io/badge/Metal-3.2-green.svg)](https://developer.apple.com/metal/)

**面向 Apple Silicon 统一内存架构的图算法优化研究原型**

Axolotl 是一个探索统一内存架构（如 Apple M4）下图算法优化的研究原型。主要特性：

- 🧱 **EdgeBlock**: 一种为 GPU 合并内存访问优化的新型图数据结构
- 🚀 **CPU+GPU 协同**: 首个在统一内存上成功实现的异构图算法
- ⚡ **增量更新**: 增量 PageRank 比完整重算快 244 倍

📖 **[English Documentation](README.md)**

---

## 目录

- [为什么叫 "Axolotl"？](#为什么叫-axolotl)
- [核心创新](#核心创新)
- [性能结果](#性能结果)
- [安装](#安装)
- [快速开始](#快速开始)
- [技术细节](#技术细节)
- [项目结构](#项目结构)
- [未来工作](#未来工作)
- [技术报告](#技术报告)
- [许可证](#许可证)
- [贡献](#贡献)
- [引用](#引用)

---

## 为什么叫 "Axolotl"？

Axolotl（墨西哥钝口螈）是一种两栖动物，可以在两种环境中生存 —— 在水中（像鱼）和在陆地上（像蝾螈）。**这正是统一内存架构所实现的：CPU 和 GPU 无缝协作，就像两栖动物在两个环境中一样。**

但这个隐喻还有更多含义：

| Axolotl 的特征 | 统一内存类比 |
|---------------|-------------|
| **两栖性**（水 + 陆地） | **CPU + GPU** 协同工作（统一内存） |
| **再生能力**（再生四肢） | **增量更新**（只更新受影响的部分） |
| **适应性**（两种环境） | **动态负载均衡**（CPU/GPU 适应任务类型） |
| **高效性**（最小能量浪费） | **零拷贝内存**（无数据传输开销） |

### 更深层的联系

传统计算就像离开水的鱼 —— CPU 和 GPU 在独立的环境中工作：
- **鱼（GPU）**：在自己的环境中很棒（并行计算），但在陆地上挣扎（无法高效处理不规则任务）
- **陆地动物（CPU）**：在陆地上很棒（串行任务、复杂逻辑），但在水中挣扎（并行计算慢）

**统一内存是两栖解决方案**：
- CPU 和 GPU 共享相同的环境（统一内存）
- 它们都可以高效工作，各自做自己最擅长的事
- 它们之间没有"环境障碍"（数据传输成本）

### 为什么不是其他动物？

- **猎豹？** 快，但只有一种模式（纯 GPU）
- **大象？** 强，但只有一种模式（纯 CPU）
- **Axolotl？** 完美 —— 在两种环境中都表现出色，适应条件

正如 axolotl 代表了双环境生活的生物创新，这个项目探索计算上的"两栖计算" —— CPU 和 GPU 在统一内存架构中无缝协作。

---

## 核心创新

### 1. EdgeBlock 数据结构

**问题**：传统的 CSR（Compressed Sparse Row）格式按顶点存储连续的边数组，但 GPU 访问模式存在非合并内存访问问题。当一个 GPU warp 处理多个顶点时，它们的边数组可能分散在内存中，导致：

- 内存事务利用率低
- 内存带宽效率降低
- 缓存局部性差

**解决方案**：**EdgeBlock** 将边分组为固定大小的块（每块 32 条边），实现：

```
传统 CSR:
  顶点 0 的边: [5, 10, 15]      ← 地址 A
  顶点 1 的边: [3, 7, 9, 12]   ← 地址 A+3
  ...
  GPU warp 访问: 非合并（不同地址）

EdgeBlock:
  Block 0: {owner=0, edges=[5, 10, 15, 填充, ...]}  ← 地址 B
  Block 1: {owner=1, edges=[3, 7, 9, 12, ...]}      ← 地址 B+34
  ...
  GPU warp 访问: 合并（连续地址）
```

**结果**：
- ✅ 幂律图上的 BFS 加速 1.20x - 1.42x
- ✅ PageRank 加速 1.27x
- ✅ 优势随图规模增大而增加（在 100K 顶点时达到峰值）

**为什么在幂律图上优势更明显？** 真实世界的网络（社交网络、网页图）遵循幂律度分布。EdgeBlock 的优势在这些图上最为明显，因为：
- 高度数顶点在 CSR 中有很长的边数组 → 严重的非合并访问
- EdgeBlock 的固定大小块通过确保顺序访问来缓解这一点

### 2. CPU+GPU 协同算法

**之前的尝试失败了**：我们尝试按顶点 degree 分工：
- CPU 处理高度数顶点（假设"不规则"）
- GPU 处理低度数顶点（假设"规则"）

**结果**：比纯 GPU 慢 **3.5 倍**。

**为什么失败？**：
1. "高度数" ≠ "不规则" —— GPU 的并行处理能力可以高效处理高度数顶点
2. CPU 缓存优势对于大型邻接表来说微不足道
3. 每一层 BFS 的 GPU 调用开销很大
4. 静态分区不能适应动态 frontier 大小

**我们的洞察**：按**工作类型**分工，而不是按数据特征：

```
✅ 正确的方法:
   CPU: 调度、记账、收敛性检查（统筹性工作）
   GPU: 并行计算（执行层面的苦力活）

❌ 错误的方法:
   CPU: 高度数顶点
   GPU: 低度数顶点
```

**第一个成功案例：增量 PageRank**

当图发生变化（添加/删除边）时，我们不会从头重新计算 PageRank。而是：

1. **CPU**：检测受影响的顶点（那些分数变化的顶点）
2. **GPU**：并行更新受影响顶点的 PageRank 分数
3. **CPU**：检查收敛性，找出新受影响的顶点（传播）
4. 重复直到收敛

**结果**：
- ✅ 比完整重算快 **244 倍**
- ✅ 高精度（最大误差 < 2.53e-07）
- ✅ 对于小的图变化，1-2 次迭代即可收敛

**成功的关键**：
- 统一内存实现零拷贝数据共享
- CPU 和 GPU 并发工作（不是顺序执行）
- 动态负载均衡（每轮受影响的顶点数动态变化）

### 3. 统一内存的优势

Apple M4 的统一内存架构（CPU/GPU 共享物理地址空间）提供了新的可能性：

| 传统（独立 GPU） | 统一内存（Apple M4） |
|----------------|-------------------|
| 数据必须在 CPU ↔ GPU 之间拷贝 | 零拷贝共享内存 |
| CPU/GPU 异步执行 | 可以并发执行 |
| 内存空间分离 | 单一地址空间 |
| 数据传输成本高 | 无传输成本 |

**对图算法的影响**：
1. **细粒度协作**：CPU 可以检查/修改 GPU 正在处理的数据
2. **增量更新**：CPU 更新图，GPU 立即看到变化
3. **无批次大小约束**：可以高效处理单个顶点

---

## 性能结果

### 实验环境

- **硬件**：Apple M4（10 核 GPU，16 核神经网络引擎）
- **软件**：macOS 14.0, Xcode 16.0, Swift 6.0, Metal 3.2
- **图类型**：幂律图（γ=2.5，模拟真实世界网络）

### EdgeBlock vs CSR（GPU BFS）

| 顶点数 | 边数 | EdgeBlock 时间 | CSR 时间 | 加速比 |
|--------|------|----------------|----------|--------|
| 10K | 50K | 0.0156s | 0.0188s | **1.20x** |
| 50K | 250K | 0.0390s | 0.0523s | **1.34x** |
| 100K | 500K | 0.0677s | 0.0958s | **1.42x** |
| 200K | 1M | 0.1189s | 0.1559s | **1.31x** |

**观察**：加速比在 100K 顶点时达到峰值。对于更大的图，GPU 内存容量成为瓶颈。

### EdgeBlock vs CSR（GPU PageRank）

| 顶点数 | 边数 | EdgeBlock 时间 | CSR 时间 | 加速比 |
|--------|------|----------------|----------|--------|
| 10K | 50K | 207.38 ms | 262.97 ms | **1.27x** |

### 增量 PageRank（CPU+GPU 协同）

**场景**：10K 顶点、50K 边的图。添加 10 条边。更新 PageRank。

| 方法 | 时间 (ms) | 迭代次数 | 加速比 | 最大误差 |
|------|----------|---------|--------|---------|
| 完整 PageRank（10 次迭代） | 644.24 | 10 | 1.00x | - |
| 增量 PageRank | 2.66 | 1 | **244x** | 2.53e-07 |

**收敛准则**：PageRank 分数最大变化 < 1e-6

### 增量 BFS（CPU+GPU 协同）

**场景**：10 万顶点、50 万边的图。添加 1 千条新边。更新 BFS 距离。

| 方法 | 时间 (ms) | 迭代次数 | 加速比 | 结果匹配 |
|------|----------|---------|--------|---------|
| 完整 BFS | 151.47 | 1 | 1.00x | - |
| 增量 BFS | 1.89 | 1 | **80x** | ✅ 100% |

### 增量 SSSP（CPU+GPU 协同）

**场景**：10 万顶点、50 万边的图（带权）。添加 1 千条新边。更新最短路径。

| 方法 | 时间 (ms) | 迭代次数 | 加速比 | 结果匹配 |
|------|----------|---------|--------|---------|
| 完整 SSSP（16 次迭代） | 180.16 | 16 | 1.00x | - |
| 增量 SSSP | 2.15 | 1 | **84x** | ✅ 100% |

### 增量 Connected Components（Union-Find）

**场景**：10 万顶点、50 万边的图。添加 5 千条新边。更新连通分量。

| 方法 | 时间 (ms) | 处理的边数 | 加速比 | 结果匹配 |
|------|----------|------------|--------|---------|
| 完整构建（所有边） | 51.89 | 499,971 | 1.00x | - |
| 增量更新（仅新边） | 0.70 | 5,000 | **74x** | ✅ 100% |

**为什么选择 Union-Find？** Union-Find 操作（find/union）极快：O(α(V)) ≈ O(1) 摊还。路径压缩 + 按秩合并 = 近似常数时间。

### 增量 Triangle Counting（邻接集合）

**场景**：1 万顶点、5 万边的图。添加 100 条新边。计数新三角形。

| 方法 | 时间 (ms) | 处理的边数 | 加速比 | 结果匹配 |
|------|----------|------------|--------|---------|
| 完整计数（所有边） | 12.86 | 50,100 | 1.00x | - |
| 增量更新（仅新边） | 0.0265 | 100 | **486x** | ✅ 100% |

**为什么 Triangle Counting 天然适合增量更新？**

1. **三角形是边中心性的** - 每个三角形通过它的一条边被计数一次
2. **添加边只创建涉及该边的三角形** - 不需要重新计数所有三角形
3. **邻接集合加速共同邻居查询** - Set 交集操作是 O(min(|S1|, |S2|))

**应用**：
- **聚类系数**：衡量顶点邻居之间的连接紧密程度
- **社交网络分析**："朋友的朋友"关系
- **链接预测**：共享很多共同邻居（三角形）的顶点对，很可能形成新边
- **图聚类**：三角形表示社区结构

---

## 安装

### 前提条件

- **macOS**: 14.0+
- **Xcode**: 16.0+（需要 Metal 3.2 的原子操作支持）
- **Swift**: 6.0+
- **硬件**：Apple Silicon（M1/M2/M3/M4），用于统一内存架构

### 克隆仓库

```bash
git clone https://github.com/LittleLollipop/axolotl.git
cd axolotl
```

### 验证环境

```bash
# 检查 Swift 版本
swift --version

# 检查 Metal 支持
system_profiler SPDisplaysDataType | grep Metal
```

---

## 快速开始

### 运行增量 PageRank（CPU+GPU 协同）

```bash
cd Experiments
swift incremental_pagerank.swift
```

**预期输出**：
```
=== 增量 PageRank 测试 ===

生成图: 10000 顶点, 50000 边
Metal 设备: Apple M4

1. 计算原始图的完整 PageRank (10 次迭代)...
    迭代 0: PR 和 = 1.2296436
    迭代 5: PR 和 = 1.210745
   完整 PageRank 时间: 644.24 ms

2. 模拟图变化 (添加 10 条边)...
   受影响顶点: 20

3. 计算增量 PageRank (容差 = 1e-6)...
    增量 PageRank: 1 次迭代, 2.66 ms

4. 从头计算完整 PageRank 用于比较...
    迭代 0: PR 和 = 1.2296436

5. 比较结果...
   最大误差: 2.526358e-07
   平均误差: 1.9120926e-10

✅ 增量 PageRank 结果与完整 PageRank 匹配！
```

### 运行 PageRank 对比（EdgeBlock vs CSR）

```bash
cd Experiments
swift pagerank.swift
```

### 运行 BFS 对比（EdgeBlock vs CSR）

```bash
cd Experiments
swift edgeblock_vs_csr.swift
```

---

## 技术细节

### EdgeBlock 数据结构

**内存布局**：

```metal
struct EdgeBlock {
    uint ownerVertex;    // 拥有这些边的顶点
    uint edgeCount;      // 实际边数（≤ 32）
    uint edges[32];     // 邻接顶点（用 UInt32.max 填充）
};
```

**块大小原理**：32 = 一个 GPU warp 的大小。这确保：
- 一个 warp 可以合并访问一个块
- 没有浪费的线程（所有 32 个通道都有用，即使 edgeCount < 32）

**填充策略**：使用 `UInt32.max`（不是 0）进行填充，因为 0 是有效的顶点 ID。

### Metal 内核：使用原子操作的 PageRank

```metal
inline void atomic_add_float(device float* address, float val) {
    device atomic_uint* atomicAddr = (device atomic_uint*)address;
    uint oldBits = atomic_load_explicit(atomicAddr, memory_order_relaxed);
    uint newBits;
    do {
        float oldVal = as_type<float>(oldBits);
        float newVal = oldVal + val;
        newBits = as_type<uint>(newVal);
    } while (!atomic_compare_exchange_weak_explicit(
        atomicAddr, &oldBits, newBits,
        memory_order_relaxed, memory_order_relaxed));
}
```

**为什么使用 CAS 循环？** Metal 不支持浮点数的 `atomic_fetch_add`。我们用比较并交换来实现。

### 增量 PageRank：CPU+GPU 协作

**算法**：

```
输入: 图 G, 初始 PageRank 分数 PR, 变化的顶点 V_changed
输出: 更新后的 PageRank 分数 PR'

1. affectedSet = V_changed
2. while affectedSet 非空 且 iteration < maxIter:
3.     // GPU: 更新受影响顶点的 PageRank
4.     for each v in affectedSet (在 GPU 上并行):
5.         PR'[v] = computePageRank(v, PR)
6.     
7.     // CPU: 检查收敛并传播
8.     newAffected = {}
9.     for each v in 所有顶点:
10.        if |PR'[v] - PR[v]| > tolerance:
11.            for each u in reverseAdjacency[v]:
12.                newAffected.add(u)
13.    
14.    PR = PR'
15.    affectedSet = newAffected
16.    iteration++
17. 
18. return PR
```

**关键洞察**：第 11-12 行传播变化。如果顶点 v 的分数变化显著，所有指向 v 的顶点可能需要重新计算。

---

## 项目结构

```
axolotl/
├── Experiments/                      # 实验代码和基准测试
│   ├── incremental_pagerank.swift   # ✅ CPU+GPU 协同 (244x 加速)
│   ├── pagerank.swift               # PageRank (EdgeBlock vs CSR)
│   ├── edgeblock_vs_csr.swift       # BFS 对比
│   ├── heterogeneous_bfs.swift      # ❌ 失败的尝试（供参考）
│   ├── heterogeneous_bfs_final.swift # ❌ 另一个失败的尝试
│   └── prototype.c                  # CPU 原型 (EdgeBlock vs CSR)
├── Docs/                            # 文档
│   ├── technical_report.md          # 综合技术报告
│   ├── DESIGN.md                    # EdgeBlock 设计文档
│   └── unified_memory_design.md     # 统一内存算法设计
├── README.md                        # 英文文档
├── README_zh.md                     # 本文件（中文）
├── LICENSE                          # MIT 许可证
└── .gitignore                       # Git 忽略规则
```

---

## 未来工作

### 短期（1-2 个月）

- [ ] **增量 BFS**：将 CPU+GPU 协作应用到 BFS
- [ ] **增量 SSSP**：单源最短路径的增量更新
- [ ] **更多算法**：连通分量、三角形计数
- [ ] **参数调优**：测试不同的 EdgeBlock 大小（16, 64, 128）

### 中期（3-6 个月）

- [ ] **简单查询接口**：基本的图查询（邻居、路径、排名）
- [ ] **更大的图**：在 1M+ 顶点图上测试
- [ ] **内存优化**：减少大图的内存占用
- [ ] **多 GPU 支持**：利用多个 GPU 核心（M4 有 10 个 GPU 核心）

### 长期（6-12 个月）

- [ ] **移植到其他架构**：Intel Arc、NVIDIA Grace（统一内存）
- [ ] **学术论文**：提交到会议（SIGMOD, VLDB, SC）
- [ ] **完整的图数据库**：如果研究结果证明值得
- [ ] **开源社区**：吸引贡献者，构建生态系统

---

## 技术报告

详细的设计决策、基准测试结果和分析，请参阅：

- **技术报告**：[`Docs/technical_report.md`](Docs/technical_report.md)
  - EdgeBlock 设计和实现
  - 性能基准测试（BFS、PageRank、异构 BFS）
  - 异构 BFS 失败原因分析
  - 增量 PageRank 设计和结果
  
- **EdgeBlock 设计**：[`Docs/DESIGN.md`](Docs/DESIGN.md)
  - 数据结构规格
  - CSR 到 EdgeBlock 的转换算法
  - 内存布局细节

- **统一内存算法设计**：[`Docs/unified_memory_design.md`](Docs/unified_memory_design.md)
  - 动机和背景
  - CPU+GPU 协作设计原则
  - 未来的算法候选

---

## 贡献

欢迎贡献！这是一个研究原型，探索统一内存架构下图算法的新思想。

### 如何贡献

1. Fork 仓库
2. 创建功能分支 (`git checkout -b feature/amazing-idea`)
3. 提交更改 (`git commit -m 'Add amazing idea'`)
4. 推送到分支 (`git push origin feature/amazing-idea`)
5. 打开 Pull Request

### 研究合作

如果你对研究合作感兴趣（统一内存图算法、异构计算），请联系我们！

**我们希望帮助的领域**：
- 更多 CPU+GPU 协同算法
- 性能优化（Metal 内核调优）
- 移植到其他统一内存架构
- 理论分析（为什么 EdgeBlock 有效？）

---

## 引用

如果在研究中使用了 Axolotl，请引用：

```bibtex
@software{axolotl2026,
  author = {LittleLollipop},
  title = {Axolotl: Unified Memory Graph Algorithms for Apple Silicon},
  year = {2026},
  url = {https://github.com/LittleLollipop/axolotl}
}
```

---

## 致谢

- 灵感来自 [Gunrock](https://github.com/gunrock/gunrock)、[CuGraph](https://github.com/rapidsai/cugraph) 和 [GraphBLAST](https://github.com/gunrock/graphblast)
- 构建在 Apple 的 [Metal](https://developer.apple.com/metal/) 框架上
- 在 Apple M4（统一内存架构）上测试

---

## 许可证

本项目采用 MIT 许可证 - 请参阅 [`LICENSE`](LICENSE) 了解详情。

---

## 联系方式

- **GitHub Issues**: [报告错误或请求功能](https://github.com/LittleLollipop/axolotl/issues)
- **Discussions**: [加入讨论](https://github.com/LittleLollipop/axolotl/discussions)

---

**⚠️ 注意**：这是一个研究原型。代码质量是实验性的，API 可能会更改。使用风险自负。

**🎉 有趣的事实**：Axolotl 也被称为"墨西哥行走鱼"（虽然它们不是鱼，而是两栖动物）。它们在野外永远不会经历变态，永远保持幼虫形态 —— 就像这个项目将永远保持"原型"形式一样（希望不会）！
