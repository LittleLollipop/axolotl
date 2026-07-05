# Axolotl 图数据库项目

高性能图数据库，支持增量算法和 GPU 加速。

## 项目结构

```
axolotl/
├── 核心研究/           # 实验报告、算法研究、性能测试
│   ├── 实验报告/       # 实验记录和结果
│   ├── 算法研究/       # 算法设计和分析
│   └── 性能测试/       # 性能基准测试
├── 原型-swift/        # Swift 原型（参考实现）
├── 原型-rust/         # Rust 实现（当前开发版本）
└── docs/              # 项目文档
```

## 快速开始

### Rust 版本（推荐）

```bash
cd 原型-rust
cargo build --release
cargo run --release --example test_pagerank_fix
```

### Swift 版本（参考）

```bash
cd 原型-swift
swift build
swift run
```

## 核心特性

- ✅ **增量算法**：只更新受影响的顶点（PageRank、BFS、SSSP）
- ✅ **GPU 加速**：使用 Metal 加速计算（macOS）
- ✅ **EdgeBlock 数据结构**：优化缓存利用率
- ✅ **CPU/GPU 协同**：统一内存，高效并发

## 文档

- [项目原则](核心研究/算法研究/PROJECT_PRINCIPLES.md)
- [增量 PageRank 状态](核心研究/实验报告/INCREMENTAL_PR_STATUS.md)
- [性能对比](核心研究/性能测试/PERFORMANCE_COMPARISON.md)

## 许可证

MIT
