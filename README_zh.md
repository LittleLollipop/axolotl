# Axolotl 图数据库项目

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Swift](https://img.shields.io/badge/Swift-6.0-orange.svg)](https://swift.org)
[![Platform](https://img.shields.io/badge/Platform-macOS%2014+-lightgrey.svg)]()
[![Metal](https://img.shields.io/badge/Metal-3.2-green.svg)](https://developer.apple.com/metal/)

**面向统一内存架构的高性能图数据库，支持增量算法和 GPU 加速**

🧱 **EdgeBlock**：统一内存时代的新型数据结构——兼顾 GPU warp 合并访问与 CPU 可变性  
🚀 **CPU+GPU 协同**：CPU 调度，GPU 计算——同一份数据，零拷贝  
⚡ **增量算法**：BFS **1580×**，连通分量 **4920×**（CPU+GPU 协同，相对于全量重算）  
🔬 **正确性优先**：PageRank PR 和 = 1.0，77 个单元测试验证  

📖 **[English Documentation](README.md)** | 📄 **[技术报告 (arXiv 预备稿)](docs/EdgeBlock-Technical-Report.md)**

---

## 目录

- [为什么叫 "Axolotl"？](#为什么叫-axolotl)
- [项目结构](#项目结构)
- [核心创新](#核心创新)
- [性能结果](#性能结果)
- [快速开始](#快速开始)
- [算法细节](#算法细节)
- [技术文档](#技术文档)
- [未来工作](#未来工作)
- [贡献](#贡献)
- [引用](#引用)
- [许可证](#许可证)

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

## 项目结构

本仓库包含多个实现和研究材料：

```
axolotl/
├── core-research/                  # 研究报告和实验
│   ├── experiment-reports/         # 实验记录和结果
│   ├── algorithm-research/         # 算法设计和分析
│   ├── performance-tests/          # 性能基准测试
│   └── docs/                      # 技术文档
├── prototype-swift/               # Swift 原型（参考实现）
│   ├── Experiments/               # 实验代码（Swift）
│   ├── Docs/                     # 设计文档
│   └── Sources/                  # Swift 源代码
├── prototype-rust/                # Rust 实现（当前开发）
│   ├── src/                      # Rust 源代码
│   ├── examples/                 # 示例程序和测试
│   └── benches/                  # 基准测试
└── README.md                     # 本文件
```

### 分支组织

- `dev`: 当前开发分支（默认）
- `swift`: Swift 原型（从 `main` 分支重命名）
- `rust`: Rust 实现（从 `master` 分支重命名）

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

**结果**（Swift 原型）：
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

### 3. 正确的 PageRank 实现

**关键修复**：处理悬挂顶点（出度为 0 的顶点）

PageRank 公式：
```
PR(v) = (1-d)/N + d × Σ PR(u) / out_degree(u)
```

**问题**：悬挂顶点（出度为 0）导致 PR 值"丢失"，PR 值之和 ≠ 1.0

**解决方案**：将悬挂顶点的 PR 值贡献均匀分布到所有顶点

**状态**（Rust 实现）：
- ✅ CPU 版本：PR 值之和 = 1.0
- ✅ GPU 增量版本：PR 值之和 = 1.0
- ✅ GPU 全量版本：PR 值之和 = 1.0

### 4. 统一内存的优势

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

### PageRank 正确性（Rust 实现）

| 实现方式 | PR 值之和 | 最大误差 | 状态 |
|---------|----------|---------|------|
| CPU（正确版本） | 1.0000 | < 1e-6 | ✅ |
| GPU 增量版本 | 1.0000 | < 1e-6 | ✅ |
| GPU 全量版本 | 1.0000 | < 1e-6 | ✅ |

### 增量算法加速比（Rust，5万顶点，+50条边）

| 算法 | 全量 (ms) | 增量 (ms) | 加速比 |
|------|-----------|-----------|--------|
| BFS | 1.91 | 0.001 | **1580x** |
| 连通分量 | 7.38 | 0.002 | **4920x** |
| PageRank | 59.85 | 3.17 | **18.9x** |

> **参考数据（纯 CPU）**：SSSP 使用差分 BFS，加速比 197x（4.18ms → 0.021ms）。三角形计数加速比 2.0x（48.95ms → 24.45ms）。两者均为 CPU 实现，不涉及 GPU 加速，仅作为参考数据。
>
> 加速比反映最佳场景（50/50,000 顶点受影响），为性能上限。三角形计数使用 10 条新增边。详见[技术报告](core-research/docs/EdgeBlock-Technical-Report.md)。

### 标准图数据集（100K 顶点，+50 随机边）

| 数据集 | V | E | BFS | CC | PageRank | SSSP |
|---------|:---:|:---:|:---:|:---:|:--------:|:----:|
| soc-Epinions1 (SNAP) | 76K | 509K | 2x | **7182x** | 8.8x | **427x** |
| com-DBLP (SNAP) | 100K | 196K | 2x | **2249x** | 8.7x | 67x |
| web-Google (GAP) | 100K | 56K | 1x | 937x | 10.3x | 95x |
| RMAT scale 20 | 100K | 170K | 1x | **2074x** | 10.3x | **233x** |

> **说明**：CC 在真实图上达到 937x-7182x——高于合成基准测试。真实社交/信任网络的强社团结构使增量更新几乎零成本。详细数据：[标准数据集基准测试](core-research/performance-tests/BENCHMARK_STANDARD_DATASETS.md)。

### GPU Buffer 缓存复用（Rust）

| 规模 | PageRank | BFS | SSSP |
|------|:---:|:---:|:---:|
| 1K/5K | 22% | 21% | 24% |
| 100K/1M | 25% | 24% | 25% |

### 增量 vs 全量重算（Swift 原型，参考）

| 算法 | 全量时间 (ms) | 增量时间 (ms) | 加速比 |
|------|--------------|--------------|--------|
| PageRank | 644.24 | 2.66 | **244x** |
| BFS | 151.47 | 1.89 | **80x** |
| SSSP | 180.16 | 2.15 | **84x** |
| 连通分量 | 51.89 | 0.70 | **74x** |
| 三角形计数 | 12.86 | 0.0265 | **486x** |

**实验环境**：Apple M4（10 核 GPU），幂律分布图

---

## 快速开始

### Rust 版本（推荐）

```bash
git clone https://github.com/LittleLollipop/axolotl.git
cd axolotl/prototype-rust

# 运行全部 77 个测试
cargo test --lib

# 启动 REST API 服务器（持久化）
cargo run --example server -- --data data/graph.axeb

# 另开终端：
curl http://localhost:8080/health
# → {"status":"ok","name":"Axolotl GraphDB"}

# 添加顶点
curl -X POST http://localhost:8080/vertices \
  -H "Content-Type: application/json" \
  -d '{"id":42,"properties":{"name":"测试"}}'

# 运行 PageRank
curl -X POST http://localhost:8080/algorithms/pagerank \
  -H "Content-Type: application/json" \
  -d '{"iterations":50}'

# 保存并退出
curl -X POST http://localhost:8080/admin/shutdown
```

### Swift 版本（参考）

```bash
cd prototype-swift

# 编译
swift build

# 运行增量 PageRank 实验
cd Experiments
swift incremental_pagerank.swift
```

---

## 技术文档

详见：

- **[EdgeBlock 技术报告](core-research/docs/EdgeBlock-Technical-Report.md)** — arXiv 预印本草案
- **[Rust 实现指南](prototype-rust/README.md)** — 完整 API 参考、架构、示例
- **[事务设计](prototype-rust/TRANSACTION_DESIGN.md)** — WAL、MVCC、崩溃恢复设计
- **[项目原则](core-research/algorithm-research/PROJECT_PRINCIPLES.md)** — 设计哲学
- **[增量算法性能报告](core-research/experiment-reports/INCREMENTAL_PERF_VERIFICATION.md)** — 实际加速比测量
- **[EdgeBlock 重构基准](core-research/performance-tests/EDGEBLOCK_REFACTOR_BENCH.md)** — GPU buffer 缓存分析
- **[增量 PageRank 状态](core-research/experiment-reports/INCREMENTAL_PR_STATUS.md)** — 正确性修复记录
- **[性能对比](core-research/performance-tests/PERFORMANCE_COMPARISON.md)** — vs NetworkX, Neo4j
- **[Swift 技术报告](prototype-swift/Docs/technical_report.md)** — 原始 EdgeBlock 设计

---

## 未来工作

- [x] ~~性能对比~~：Axolotl vs NetworkX vs Neo4j~~ → [见基准](core-research/experiment-reports/INCREMENTAL_PERF_VERIFICATION.md)
- [x] ~~增量 BFS：CPU+GPU 协同（Rust）~~
- [x] ~~增量 SSSP：单源最短路径（Rust）~~
- [x] ~~PageRank：增量和全量版本（GPU）~~
- [x] ~~更多算法：连通分量、三角形计数~~
- [x] ~~持久化：二进制图格式~~
- [x] ~~简单查询接口：邻居、路径、排名~~
- [x] ~~REST API 服务器~~
- [x] ~~Python 绑定：PyO3 集成~~ → `pip install` 可用
- [ ] **增量 PageRank 性能优化**：GPU kernel 调用路径改进
- [ ] **更大的图**：在 10M+ 顶点图上测试
- [ ] **内存优化**：减少大图的内存占用
- [ ] **多 GPU 支持**：利用多个 GPU 核心
- [ ] **分布式支持**：多机图分片/复制
- [ ] **移植到其他架构**：Intel Arc、NVIDIA Grace（统一内存）
- [ ] **学术论文**：提交到会议（SIGMOD, VLDB, SC）

---

## 贡献

欢迎贡献！这是一个研究项目，探索统一内存架构下图算法的新思想。

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

**⚠️ 注意**：这是一个研究项目。代码质量是实验性的，API 可能会更改。使用风险自负。

**🎉 有趣的事实**：Axolotl 也被称为"墨西哥行走鱼"（虽然它们不是鱼，而是两栖动物）。它们在野外永远不会经历变态，永远保持幼虫形态 —— 就像这个项目将永远保持"原型"形式一样（希望不会）！
