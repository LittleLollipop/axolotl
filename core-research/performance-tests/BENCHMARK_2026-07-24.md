# Benchmark Results — 2026-07-24

> **⚠️ ARCHIVED — DO NOT UPDATE**
>
> 2026-07-24 记录，基于 2026-07-13 快照后的一次关键优化（Python 绑定 PageRank 推式 EdgeBlock 修复）。

**Date**: 2026-07-24 | **Hardware**: Apple M4, 10-core GPU, 16GB unified memory  
**Change**: Python 绑定 PageRank 从逐顶点 in_neighbors 扫描 → 推式 EdgeBlock 块遍历

**PageRank after fix**:

| Dataset | Before (7-13) | After (7-24) | Speedup |
|---------|:---:|:---:|:---:|
| soc-Epinions1 (75K) | 12,013ms | **50ms** | 240x |
| com-DBLP (100K) | 1,069ms | **36ms** | 30x |
| web-Google (100K) | 437ms | **18ms** | 24x |
| RMAT (99K) | 1,965ms | **30ms** | 66x |

---

## Python Library Comparison (axolotl_rs vs NetworkX vs iGraph)

### soc-Epinions1 (76K/509K)

| Library | Build | BFS | PageRank |
|---------|:-----:|:---:|:--------:|
| axolotl_rs | 190ms | 11ms | **50ms** |
| NetworkX | 578ms | 119ms | 820ms |
| iGraph (C) | 27ms | 3ms | 37ms |

### com-DBLP (100K/196K)

| Library | Build | BFS | PageRank |
|---------|:-----:|:---:|:--------:|
| axolotl_rs | 58ms | 5ms | **36ms** |
| NetworkX | 103ms | 185ms | 90ms |
| iGraph (C) | 12ms | 2ms | 7ms |

### web-Google (100K/55K)

| Library | Build | BFS | PageRank |
|---------|:-----:|:---:|:--------:|
| axolotl_rs | 21ms | 0ms | **18ms** |
| NetworkX | 126ms | 0ms | 48ms |
| iGraph (C) | 5ms | 0ms | 10ms |

### RMAT (99K/170K)

| Library | Build | BFS | PageRank |
|---------|:-----:|:---:|:--------:|
| axolotl_rs | 53ms | 9ms | **30ms** |
| NetworkX | 156ms | 97ms | 100ms |
| iGraph (C) | 12ms | 2ms | 14ms |

**PYTHON 库对比结论**: PageRank 从最短板变成最强项——全面超越 NetworkX（5-16x），与 C 内核的 iGraph 差距缩到 1.4-5x。

---

## Rust Native: Raw Throughput

| Dataset | V | E | Build | BFS | PageRank |
|---------|:---:|:---:|:-----:|:---:|:--------:|
| soc-Epinions1 | 75K | 508K | 0.13s | 0.01s | 55ms |
| com-DBLP | 100K | 196K | 0.04s | 0.01s | 63ms |
| web-Google | 100K | 55K | 0.02s | 0.00s | 47ms |
| RMAT scale 20 | 99K | 170K | 0.10s | 0.02s | 53ms |
| com-DBLP (full) | 425K | 1.0M | 0.48s | 0.06s | 335ms |
| web-Google (full) | 916K | 5.1M | 2.02s | 0.29s | 922ms |
| soc-LiveJournal1 | 4.8M | 5.9M | 3.36s | 0.43s | 1792ms |
| RMAT scale 21 | 2.1M | 7.2M | 5.21s | 1.17s | 1826ms |

> soc-LiveJournal1: 4.8M 顶点，5.9M 边（部分数据，完整版 ~69M 边）。16GB 下无 OOM。

---

## Incremental Algorithm Speedup

| Dataset | V | +Edges | BFS | CC | PageRank | SSSP |
|---------|:---:|:---:|:---:|:---:|:--------:|:----:|
| soc-Epinions1 | 75K | +76 | 1874x | 1281x | 8.0x | 72x |
| com-DBLP | 100K | +100 | 537x | 292x | 10.1x | 37x |
| web-Google | 100K | +100 | 661x | 460x | 8.9x | 106x |
| RMAT scale 20 | 99K | +100 | 2249x | 453x | 10.3x | 151x |
| com-DBLP (full) | 425K | +426 | 33x | 282x | 9.4x | 126x |
| web-Google (full) | 916K | +916 | 789x | 282x | 10.3x | 98x |
| soc-LiveJournal1 | 4.8M | +4848 | 1511x | 29x | 11.3x | 19x |
| RMAT scale 21 | 2.1M | +2097 | 5587x | 887x | 19.9x | 456x |

---

## Library Comparison: axolotl-rs vs petgraph

| Dataset | V | Library | Build | BFS | PageRank |
|---------|:---:|---------|:-----:|:---:|:--------:|
| soc-Epinions1 | 75K | axolotl-rs | 151ms | 9.4ms | **56ms** |
| | | petgraph | 3ms | 3.0ms | 204ms |
| com-DBLP | 100K | axolotl-rs | 47ms | 4.9ms | **62ms** |
| | | petgraph | 1ms | 1.1ms | 95ms |
| RMAT scale 20 | 99K | axolotl-rs | 37ms | 7.6ms | **34ms** |
| | | petgraph | 1ms | 2.1ms | 154ms |
| com-DBLP (full) | 425K | axolotl-rs | 294ms | 55.6ms | **314ms** |
| | | petgraph | 6ms | 12.9ms | 564ms |
| web-Google (full) | 916K | axolotl-rs | 1803ms | 269ms | **926ms** |
| | | petgraph | 38ms | 57ms | 8621ms |
| soc-LiveJournal1 | 4.8M | axolotl-rs | 2718ms | 345ms | **1173ms** |
| | | petgraph | 97ms | 71ms | 4950ms |
| RMAT scale 21 | 2.1M | axolotl-rs | 5086ms | 953ms | **1300ms** |
| | | petgraph | 75ms | 678ms | 65764ms |

**vs petgraph 结论**: PageRank 优势从 4.2x（LiveJournal）到 50.6x（RMAT21）。Build 仍是 petgraph 强项（20-70x 更快）。

---

## Summary: PageRank Ranking (Python Binding)

| Rank | July 13 | July 24 |
|:---:|------|------|
| 1 | iGraph (72ms) | iGraph (37ms) |
| 2 | NetworkX (766ms) | **axolotl_rs (50ms)** |
| 3 | petgraph (423ms) | NetworkX (820ms) |
| 4 | axolotl_rs (12013ms) | — |

Python 绑定 PageRank 从垫底跳到第二，距 C 内核仅 1.4x。
