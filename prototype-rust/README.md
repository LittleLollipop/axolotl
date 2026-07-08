# Axolotl-RS — Rust 实现文档

> **版本**: 0.1.0 | **测试**: 77 passing | **平台**: macOS (Metal GPU) / Linux (CPU)

## 概述

Axolotl-RS 是 Axolotl 图数据库项目的 Rust 实现，当前主力开发版本。提供高性能图存储、GPU 加速算法、REST API 网络层。

## 核心架构

```
REST API (src/server.rs)
    │
    ▼
GraphDB (src/graph_db.rs)       ← 统一接口
    │
    ├── GPUEdgeBlockGraph (src/gpu_edge_block.rs)    ← 主存储
    │       ├── EdgeData (边属性：weight + properties)
    │       ├── AXEB 二进制持久化
    │       └── CSR 兼容转换
    │
    ├── MmapGraph (src/mmap_graph.rs)                ← 只读大图
    │
    └── 事务层
            ├── Transaction (src/transaction.rs)     ← WAL + 回滚
            ├── MVCC (src/mvcc.rs)                   ← 快照隔离
            └── Recovery (src/recovery.rs)           ← WAL 重放
```

## 快速开始

```bash
# 编译
cd prototype-rust
cargo build --release

# 运行所有测试
cargo test --lib
# 输出: test result: ok. 77 passed

# 启动 REST API 服务器
cargo run --example server -- --data data/graph.axeb
# 服务启动在 http://localhost:8080
```

## 模块列表

| 模块 | 路径 | 功能 |
|------|------|------|
| **gpu_edge_block** | `src/gpu_edge_block.rs` | 主存储：EdgeBlock 格式，CRUD，持久化，遍历 |
| **graph_db** | `src/graph_db.rs` | 统一接口：InMemory / Mmap 模式切换 |
| **server** | `src/server.rs` | REST API：CRUD、算法、遍历、管理 |
| **recovery** | `src/recovery.rs` | Crash Recovery：WAL 扫描+重放 |
| **transaction** | `src/transaction.rs` | 事务：WAL、提交、回滚、自动回滚 |
| **mvcc** | `src/mvcc.rs` | MVCC 快照隔离（Copy-on-Write） |
| **persistence** | `src/persistence.rs` | PersistentGraph 兼容层（AXOL 格式） |
| **mmap_graph** | `src/mmap_graph.rs` | mmap 只读大图支持 |
| **csr_graph** | `src/csr_graph.rs` | CSR 格式（GPU 算法兼容） |
| **pagerank_correct** | `src/pagerank_correct.rs` | CPU 正确 PageRank（PR 和=1.0） |
| **incremental_pagerank** | `src/incremental_pagerank.rs` | 增量 PageRank（CPU/GPU） |
| **incremental_bfs** | `src/incremental_bfs.rs` | 增量 BFS（CPU/GPU） |
| **incremental_sssp** | `src/incremental_sssp.rs` | 增量 SSSP（CPU/GPU） |
| **incremental_cc** | `src/incremental_cc.rs` | 增量 Connected Components（Union-Find） |
| **incremental_tc** | `src/incremental_tc.rs` | 增量 Triangle Counting |
| **index** | `src/index.rs` | 哈希索引（精确匹配） |
| **compact_storage** | `src/compact_storage.rs` | 紧凑存储（减少内存 2-3x） |
| **gpu** | `src/gpu/mod.rs` | GPU 加速（Metal kernels） |

## GraphDB API

```rust
use axolotl_rs::graph_db::{GraphDB, GraphMode};
use axolotl_rs::PropertyValue;
use std::collections::HashMap;

// 创建/加载数据库（自动 WAL 恢复）
let mut db = GraphDB::from_file_or_new("data/graph.axeb")?;

// 顶点 CRUD
let mut props = HashMap::new();
props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
db.add_vertex(1, props)?;
let v = db.get_vertex(1)?;
db.delete_vertex(1)?;

// 边 CRUD（含属性）
let mut edge_props = HashMap::new();
edge_props.insert("label".to_string(), PropertyValue::String("knows".to_string()));
db.add_edge(1, 2, 0.85, edge_props)?;
let (weight, props) = db.get_edge(1, 2).unwrap();
db.delete_edge(1, 2)?;

// 查询
let neighbors = db.out_neighbors(1);         // → [2, 3]
let count = db.vertex_count();               // → 3
let csr = db.to_csr();                       // → CSRGraph（GPU 兼容）

// 遍历
let visited = db.walk(1, 2, |from, depth, to| { println!("{from}→{to}"); });
let (vertices, edges) = db.subgraph(&[1], 3);
let paths = db.find_paths(None, None, 2);   // 所有 2-hop 路径

// 保存
db.save_to_file()?;                          // 保存到绑定的 data_file
db.save_to("backup.axeb")?;                  // 保存到指定路径
```

## REST API

| 方法 | 路径 | 功能 |
|------|------|------|
| GET | `/health` | 健康检查 |
| GET | `/stats` | 图统计（顶点数、边数、模式） |
| POST | `/vertices` | 添加顶点 `{"id":N,"properties":{...}}` |
| GET | `/vertices/:id` | 获取顶点属性 |
| DELETE | `/vertices/:id` | 删除顶点（级联清理边） |
| POST | `/edges` | 添加边 `{"from":N,"to":M,"weight":1.0}` |
| DELETE | `/edges/:from/:to` | 删除边 |
| GET | `/neighbors/:id` | 出边邻居 |
| GET | `/neighbors/:id/in` | 入边邻居 |
| POST | `/algorithms/pagerank` | PageRank `{"iterations":100}` |
| POST | `/algorithms/bfs` | BFS `{"source":0}` |
| POST | `/traverse/walk` | 多跳遍历 `{"start":N,"max_depth":3}` |
| POST | `/traverse/subgraph` | 子图提取 `{"seeds":[...],"max_depth":2}` |
| POST | `/traverse/find_paths` | 路径匹配 `{"path_length":2,"max_results":100}` |
| POST | `/admin/save` | 手动保存到文件 |
| POST | `/admin/shutdown` | 安全关闭（自动保存后退出） |

## 持久化

**AXEB 格式**（EdgeBlock 原生二进制）：

```
Header: "AXEB" | version: u32 | vertex_count: u32 | total_edges: u64

Section 1 — Topology:
  vertices: [u32]
  block_counts: [u32]
  blocks: [u32]           # 正边
  reverse_vertices: [u32]
  reverse_block_counts: [u32]
  reverse_blocks: [u32]   # 反边

Section 2 — Vertex Properties:
  count: u32
  for each: (n_keys: u16, key: str, value: PropertyValue)

Section 3 — Edge Properties:
  count: u32
  for each: from: u64, to: u64, weight: f32, (n_keys: u16, key: str, value: PropertyValue)
```

**流程**：
- `save(path)` → AXEB 原子写入（先写 .tmp 再 rename）
- `open(path)` → 加载 AXEB（回退 AXOL 兼容）
- `from_file_or_new(path)` → `open()` + WAL 恢复，不存在则创建空库
- POST `/admin/shutdown` → 自动保存 → 退出

## 并发模型

- **存储**: 无锁（EdgeBlock 由调用者管理同步）
- **服务器**: `Arc<RwLock<GraphDB>>` + 16 线程池 + 非阻塞 accept
- **PageRank**: 锁内 `to_csr()`（60ms）→ 释放锁 → 锁外计算（60ms）
- **写操作**: 单写锁保护，写操作本身 < 1ms

## 性能

### 增量算法加速比（Rust 实际测量）

| 算法 | 目标 | 1K | 10K | 50K | 状态 |
|------|:---:|:---:|:---:|:---:|:---:|
| PageRank | 244x | 0.1x | 7.5x | 18.9x | ⚠️ GPU 开销 |
| BFS | 80x | 31x | **283x** | **1580x** | ✅ 远超 |
| CC | 74x | 88x | **720x** | **4920x** | ✅ 远超 |
| SSSP | — | 75x | **423x** | **197x** | ✅ 差分 BFS |
| Triangle Counting | — | 2.1x | 2.0x | **1.9x** | — |

详见：[增量算法性能验证报告](core-research/experiment-reports/INCREMENTAL_PERF_VERIFICATION.md)

### GPU Buffer 缓存复用

| 规模 | PageRank | BFS | SSSP |
|------|:---:|:---:|:---:|
| 1K/5K | 22% | 21% | 24% |
| 10K/50K | 25% | 24% | 24% |
| 100K/1M | 25% | 24% | 25% |

详见：[EdgeBlock 重构基准](core-research/performance-tests/EDGEBLOCK_REFACTOR_BENCH.md)

## 示例程序

```bash
cargo run --example server              # REST API 服务器
cargo run --example bench_incremental   # 增量算法性能验证
cargo run --example test_pagerank_fix   # PageRank 正确性测试
```

## 依赖

- Rust 1.75+
- macOS 14+ (GPU/Metal)
- Linux (CPU-only)

```toml
serde = "1"            # 序列化
serde_json = "1"       # JSON
thiserror = "2"        # 错误处理
rand = "0.9"           # 随机图生成
memmap2 = "0.9"        # 内存映射大图
```

## 许可证

MIT
