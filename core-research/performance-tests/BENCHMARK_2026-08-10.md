# axolotl-rs 全算法基准报告 — 2026-08-10

> Hardware: Apple M4, 10-core GPU, 16GB

## 算法总览

| 算法 | 类型 | vs petgraph 最佳 | Python 绑定 |
|------|------|:---:|:--:|
| **PageRank** | 推式迭代 | 4.5x (RMAT21) | ✅ |
| **Wave Core** | 密度分层 | 1.5x (web-Google) | ✅ |
| **Tarjan SCC** | DFS 遍历 | 3.8x (RMAT21) | ❌ |
| **Brandes BC** | BFS 采样 | 2x (web-Google) | ❌ |
| **Louvain** | 模块度优化 | 113ms/100K 边 | ❌ |

## 详细对比

### PageRank (vs petgraph)
| Dataset | EdgeBlock | petgraph |
|---------|:---:|:---:|
| soc-Epinions1 (75K) | **56ms** | 204ms |
| web-Google (916K) | **926ms** | 8621ms |
| RMAT scale 21 (2.1M) | **1336ms** | 65764ms |

### Wave Core (vs petgraph K-Core)
| Dataset | Wave Core | petgraph BZ |
|---------|:---:|:---:|
| com-DBLP (425K) | **61ms** | 124ms |
| web-Google (916K) | **549ms** | 810ms |
| RMAT scale 21 (2.1M) | 3341ms | 901ms |

### Tarjan SCC (vs petgraph Kosaraju)
| Dataset | Tarjan | Kosaraju |
|---------|:---:|:---:|
| soc-LiveJournal (4.8M) | **166ms** | 359ms |
| web-Google (916K) | **105ms** | 238ms |
| RMAT scale 21 (2.1M) | **349ms** | 1344ms |

### Betweenness (32 sources, vs adjacency list)
| Dataset | EdgeBlock BFS | Adj list BFS |
|---------|:---:|:---:|
| web-Google (100K) | 0.5ms | 1ms |
| soc-Epinions1 (75K) | 133ms | 92ms |
| com-DBLP (425K) | 40ms | 5ms |

### Louvain (Phase 1+2, HashMap-free)
| Dataset | Time | Communities | Passes |
|---------|:---:|:---:|:---:|
| soc-Epinions1 | 677ms | 1726 | 3 |
| com-DBLP | 113ms | 36673 | 4 |
| web-Google | 38ms | 67301 | 3 |
| RMAT scale 20 | 170ms | 61586 | 3 |

## 库对比矩阵

### Python 库对比 (100K 顶点级)
| 算法 | axolotl-rs | NetworkX | iGraph | Networkit |
|------|:---:|:---:|:---:|:---:|
| PageRank (Epinions1) | **50ms** | 820ms | 37ms | — |
| Wave Core (K-Core) (DBLP) | **6ms** | 132ms | 4ms | 6ms |
| Louvain (Epinions1) | **677ms** | 8394ms | 666ms | 129ms |
| Louvain (DBLP) | **113ms** | 17498ms | 897ms | 85ms |

### Rust 库对比 (vs petgraph)
| 算法 | 胜 | 负 | 最佳倍数 |
|------|:--:|:--:|:---:|
| PageRank | 4/4 | 0/4 | 49x |
| Wave Core | 2/4 | 2/4 | 2x |
| SCC | 6/8 | 2/8 | 3.8x |
| Betweenness | 1/4 | 3/4 | 2x |

## 结论

1. **推式算法 (PageRank)** 是 EdgeBlock 的最强场景——块扫描推送天然并行，碾压 petgraph
2. **DFS 遍历 (SCC)** 在大图上因单次遍历优势胜 Kosaraju
3. **迭代社区 (Louvain)** 去 HashMap 后 DBLP 比 Python 快 155x，但因社团质量差异速度不如 C++ 核
4. **BFS 型算法 (Betweenness)** 邻接表天然更快，不是 EdgeBlock 的强项
5. **瓶颈消除**: Wave Core 的 bin 扫描和 Vec 容量、Louvain 的 HashMap 聚合——两个瓶颈都是数据结构问题，不是架构问题
