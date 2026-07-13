# Benchmark Results — 2026-07-13

> **⚠️ ARCHIVED — DO NOT UPDATE**
>
> These results were recorded on 2026-07-13 with axolotl-rs v0.1.0-beta.
> Future code changes may improve performance; create a new dated file for new results.

**Hardware**: Apple M4, 10-core GPU, 16GB unified memory  
**PageRank**: 100 iterations, damping=0.85  
**Incremental rule**: `+0.1%` of vertices as random new edges

---

## Raw Throughput (EdgeBlock Rust Native)

| Dataset | V | E | Build | BFS | PageRank |
|---------|:---:|:---:|:-----:|:---:|:--------:|
| soc-Epinions1 | 75K | 508K | 0.12s | 0.01s | 47ms |
| com-DBLP | 100K | 196K | 0.04s | 0.01s | 58ms |
| web-Google | 100K | 55K | 0.01s | 0.00s | 38ms |
| RMAT scale 20 | 99K | 170K | 0.04s | 0.01s | 31ms |
| com-DBLP (full) | 425K | 1.0M | 0.37s | 0.06s | 317ms |
| web-Google (full) | 916K | 5.1M | 1.73s | 0.27s | 1040ms |
| RMAT scale 21 | 2.1M | 7.2M | 4.50s | 1.03s | 1356ms |

---

## Incremental Algorithm Speedup

| Dataset | V | +Edges | BFS | CC | PageRank | SSSP |
|---------|:---:|:---:|:---:|:---:|:--------:|:----:|
| soc-Epinions1 | 75K | +76 | 4690x | 569x | 10.3x | 339x |
| com-DBLP | 100K | +100 | 957x | 406x | 9.6x | 29x |
| web-Google | 100K | +100 | 738x | 437x | 8.7x | 64x |
| RMAT scale 20 | 99K | +100 | 2692x | 948x | 9.3x | 218x |
| com-DBLP (full) | 425K | +426 | 1051x | 264x | 9.9x | 28x |
| web-Google (full) | 916K | +916 | 131x | 721x | 9.9x | 34x |
| RMAT scale 21 | 2.1M | +2097 | 8467x | 720x | 9.8x | 255x |

- **BFS**: speedup grows with perturbation size (131x–8467x). Differential BFS only traverses vertices with actual distance changes.
- **CC**: strongly structure-dependent. Denser power-law graphs (948x) outperform sparse citation networks (264x).
- **PageRank**: ~10x across all scales (100 → 10 iterations). Converges independently of graph structure.
- **SSSP**: differential BFS, 28x–339x. Best on community-structured graphs where distance changes are localized.

---

## Library Comparison: EdgeBlock vs petgraph (Rust)

| Dataset | V | Library | Build | BFS | PageRank |
|---------|:---:|---------|:-----:|:---:|:--------:|
| soc-Epinions1 | 75K | EdgeBlock | 117ms | 9.8ms | **49ms** |
| | | petgraph | 3ms | 2.6ms | 213ms |
| com-DBLP | 100K | EdgeBlock | 38ms | 5.1ms | **63ms** |
| | | petgraph | 1ms | 1.7ms | 98ms |
| web-Google | 100K | EdgeBlock | 15ms | 0.0ms | **41ms** |
| | | petgraph | 0ms | 0.0ms | 54ms |
| RMAT scale 20 | 99K | EdgeBlock | 37ms | 8.0ms | **31ms** |
| | | petgraph | 1ms | 3.1ms | 161ms |
| com-DBLP (full) | 425K | EdgeBlock | 331ms | 60.4ms | **319ms** |
| | | petgraph | 7ms | 13.8ms | 604ms |
| web-Google (full) | 916K | EdgeBlock | 1984ms | 267ms | **938ms** |
| | | petgraph | 42ms | 62ms | 9022ms |
| RMAT scale 21 | 2.1M | EdgeBlock | 4805ms | 1010ms | **1401ms** |
| | | petgraph | 96ms | 723ms | 66543ms |

EdgeBlock PageRank vs petgraph advantage:

| Scale | EB | petgraph | Ratio |
|:---:|:---:|:---:|:---:|
| 75K | 49ms | 213ms | 4.3x |
| 100K | 63ms | 98ms | 1.6x |
| 425K | 319ms | 604ms | 1.9x |
| 916K | 938ms | 9022ms | 9.6x |
| 2.1M | 1401ms | 66543ms | **47.5x** |

Build: petgraph 20–50x faster (zero-overhead allocation).

---

## Library Comparison: EdgeBlock vs Networkit vs iGraph (Python APIs)

| Dataset | V | Library | Build | BFS | PageRank |
|---------|:---:|---------|:-----:|:---:|:--------:|
| com-DBLP (full) | 425K | Networkit (C++) | 116ms | 32ms | **16ms** |
| | | iGraph (C) | 62ms | 21ms | 42ms |
| web-Google (full) | 916K | Networkit (C++) | 495ms | 110ms | **264ms** |
| | | iGraph (C) | 743ms | 78ms | 577ms |
| RMAT scale 21 | 2.1M | Networkit (C++) | 888ms | 231ms | **262ms** |
| | | iGraph (C) | 2328ms | 117ms | 827ms |

**C++ comparison notes**: Networkit's PageRank uses convergence-based early termination (tol=1e-6) rather than fixed 100 iterations. EdgeBlock's PageRank (via Rust CSR) is 1401ms at RMAT21 — 5.3x slower than Networkit but 1.7x slower than iGraph. The gap narrows with scale.

---

## Python Binding Overhead

| RMAT scale 21 | Rust Native | Python Binding | Overhead |
|---------|:---:|:---:|:---:|
| Build | 4805ms | — | — |
| BFS | 1010ms | — | — |
| PageRank | 1401ms | ~200s (est.) | **140x** |

Python binding PageRank performs per-iteration serialization, making large-scale use impractical. A future optimization should move the iteration loop inside Rust.
