# Standard Graph Benchmark Report

**Date**: 2026-07-13 | **Hardware**: Apple M4, 10-core GPU | **Test**: +50 random edges, CPU incremental algorithms

## Datasets

| Dataset | Source | Type | Vertices | Edges | Avg Degree |
|---------|--------|------|:---:|:---:|:---:|
| soc-Epinions1 | SNAP | Trust network | 75,879 | 508,837 | 13.4 |
| com-DBLP | SNAP | Citation network | 100,000* | 196,248 | 3.9 |
| web-Google | SNAP/GAP | Web graph | 100,000* | 55,851 | 1.1 |
| RMAT scale 20 | Graph500 | Power-law synthetic | 99,995* | 170,334 | 3.4 |

> *Subsampled to first 100K vertices for fair comparison.

## Incremental Algorithm Speedup

### Summary Table

| Dataset | V | E | BFS | CC | PageRank | SSSP |
|---------|:---:|:---:|:---:|:---:|:--------:|:----:|
| soc-Epinions1 | 76K | 509K | 2x | **7182x** | 8.8x | **427x** |
| com-DBLP | 100K | 196K | 2x | **2249x** | 8.7x | 67x |
| web-Google | 100K | 56K | 1x | 937x | 10.3x | 95x |
| RMAT scale 20 | 100K | 170K | 1x | **2074x** | 10.3x | **233x** |

### Full vs Incremental Times (ms)

| Dataset | Algorithm | Full Time | Incr Time | Speedup |
|---------|-----------|:---:|:---:|:---:|
| **soc-Epinions1** | BFS | 0.62 | 0.26 | 2x |
| | CC | 7.40 | 0.001 | 7182x |
| | PageRank | 56.79 | 6.47 | 8.8x |
| | SSSP | 0.62 | 0.001 | 427x |
| **com-DBLP** | BFS | 0.98 | 0.57 | 2x |
| | CC | 4.15 | 0.002 | 2249x |
| | PageRank | 80.71 | 9.28 | 8.7x |
| | SSSP | 0.98 | 0.015 | 67x |
| **web-Google** | BFS | 0.54 | 0.40 | 1x |
| | CC | 1.59 | 0.002 | 937x |
| | PageRank | 57.23 | 5.56 | 10.3x |
| | SSSP | 0.54 | 0.006 | 95x |
| **RMAT scale 20** | BFS | 0.85 | 0.61 | 1x |
| | CC | 3.85 | 0.002 | 2074x |
| | PageRank | 81.38 | 7.90 | 10.3x |
| | SSSP | 0.85 | 0.004 | 233x |

## Synthetic Graph Comparison (50K vertices, 250K edges, uniform random)

| Algorithm | 50K Random | soc-Epinions1 | com-DBLP | web-Google | RMAT |
|-----------|:---:|:---:|:---:|:---:|:---:|
| BFS | 1580x | 2x | 2x | 1x | 1x |
| CC | 4920x | 7182x | 2249x | 937x | 2074x |
| PageRank | 18.9x | 8.8x | 8.7x | 10.3x | 10.3x |
| SSSP | 197x | 427x | 67x | 95x | 233x |

## Key Observations

### Connected Components Dominates Real Graphs
CC achieves 937x-7182x on real graphs, exceeding even the synthetic benchmark. Real trust/social networks have strong community structure — adding 50 random edges rarely bridges communities, so incremental UnionFind does almost no work.

### BFS Incremental Benefit is Minimal at Scale
On 100K-vertex graphs, adding 50 edges has negligible impact on BFS trees. Full BFS is already heavily optimized and incremental BFS cannot beat it at this perturbation ratio. BFS incremental shines only when the change affects a large fraction of vertices (like our synthetic 50K benchmark where 50/250K edges changed).

### SSSP Differential BFS is Robust
67x-427x across all real graphs. The differential approach (only propagate where distances actually decrease) eliminates the O(V) per-iteration scan that plagued the GPU version. Best on soc-Epinions1 (427x) where the trust graph's community structure localizes distance changes.

### PageRank is Structure-Independent
8.7x-10.3x across all graphs. The 10x speedup simply reflects 100 → 10 iterations needed for convergence on the updated graph. The damping factor (0.85) ensures uniform convergence rate regardless of graph structure.

### RMAT vs Random: Different Behaviors
RMAT's power-law degree distribution creates structural differences from uniform random graphs. CC speedup on RMAT (2074x) is lower than on random (4920x) because RMAT naturally clusters, making individual edge changes more disruptive to the partition.
