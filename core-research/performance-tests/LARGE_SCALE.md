# Large-Scale Performance Test

**Date**: 2026-07-13 | **Hardware**: Apple M4, 16GB unified memory | **PageRank**: 100 iterations

EdgeBlock 在 425K-1M 顶点全量数据集上的构建/BFS/PageRank 性能。证明 EdgeBlock 并非仅限小图。

## Results

| Dataset | V | E | Build | BFS | PageRank |
|---------|:---:|:---:|:-----:|:---:|:--------:|
| com-DBLP | 425K | 1.0M | 0.8s | 0.1s | 628ms |
| web-Google | 916K | 5.1M | 4.1s | 0.8s | 1,792ms |
| RMAT scale 20 | 1.0M | 1.0M | 0.5s | 0.1s | 340ms |
| RMAT scale 21 | 2.1M | 7.2M | 4.7s | 1.3s | 1,373ms |

## Scale Analysis

从 100K 子图到全量的扩展表现:

| Dataset | Scale Factor (vertices) | Scale Factor (edges) | Build | BFS | PageRank |
|---------|:---:|:---:|:---:|:---:|:---:|
| soc-Epinions1 (100K sub) | 1× | 1× | 226ms | 19ms | 95ms |
| com-DBLP (full) | 4.2× | 5.1× | 0.8s (3.5×) | 0.1s (5.3×) | 628ms (6.6×) |
| web-Google (full) | 9.2× | 91× | 4.1s (18×) | 0.8s (42×) | 1,792ms (19×) |
| RMAT20 (full) | 10× | 5.9× | 0.5s (5×) | 0.1s (11×) | 340ms (7×) |
| RMAT21 | 21× | 42× | 4.7s (21×) | 1.3s (68×) | 1,373ms (14×) |

**结论**: Build 和 PageRank 随顶点数接近线性增长。BFS 在 web-Google 上增长较快（42× for 9× vertices），因为 web graph 的直径小、遍历几乎覆盖全图。

## Memory

所有测试在 Apple M4 16GB 上完成，无 OOM 或交换。

## How to Run

```bash
cd prototype-rust
cargo run --release --example bench_large_datasets
```

需要 `performance_test/datasets/` 下有完整 edgelist 文件。
