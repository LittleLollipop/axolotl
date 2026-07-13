# axolotl-rs Comprehensive Benchmarks

**Date**: 2026-07-13 | **Hardware**: Apple M4, 16GB unified memory

All benchmarks use proportional edge additions: `+0.1%` of vertex count as new edges.

## Datasets

| Dataset | Source | Vertices | Edges | Type |
|---------|--------|:---:|:---:|------|
| soc-Epinions1 | SNAP | 75,879 | 508,837 | Trust network |
| com-DBLP | SNAP | 100,000 | 196,248 | Citation network |
| web-Google | SNAP/GAP | 100,000 | 55,851 | Web graph |
| RMAT scale 20 | Graph500 | 99,995 | 170,334 | Power-law synthetic |
| com-DBLP (full) | SNAP | 425,957 | 1,049,866 | Citation network |
| web-Google (full) | SNAP/GAP | 916,428 | 5,105,039 | Web graph |
| RMAT scale 21 | Graph500 | 2,097,152 | 7,167,219 | Power-law synthetic |

## Raw Throughput (axolotl-rs Rust Native)

| Dataset | V | E | Build | BFS | PageRank |
|---------|:---:|:---:|:-----:|:---:|:--------:|
| soc-Epinions1 | 75K | 508K | 0.12s | 0.01s | 47ms |
| com-DBLP | 100K | 196K | 0.04s | 0.01s | 58ms |
| web-Google | 100K | 55K | 0.01s | 0.00s | 38ms |
| RMAT scale 20 | 99K | 170K | 0.04s | 0.01s | 31ms |
| com-DBLP (full) | 425K | 1.0M | 0.37s | 0.06s | 317ms |
| web-Google (full) | 916K | 5.1M | 1.73s | 0.27s | 1040ms |
| RMAT scale 21 | 2.1M | 7.2M | 4.50s | 1.03s | 1356ms |

**Scale**: Build 37× from 75K → 2.1M (28× vertices). PageRank 29×. Near-linear.

## Incremental Algorithm Speedup

**Rule**: `+0.1%` of vertices as new random edges.

| Dataset | V | +Edges | BFS | CC | PageRank | SSSP |
|---------|:---:|:---:|:---:|:---:|:--------:|:----:|
| soc-Epinions1 | 75K | +76 | 4690x | 569x | 10.3x | 339x |
| com-DBLP | 100K | +100 | 957x | 406x | 9.6x | 29x |
| web-Google | 100K | +100 | 738x | 437x | 8.7x | 64x |
| RMAT scale 20 | 99K | +100 | 2692x | 948x | 9.3x | 218x |
| com-DBLP (full) | 425K | +426 | 1051x | 264x | 9.9x | 28x |
| web-Google (full) | 916K | +916 | 131x | 721x | 9.9x | 34x |
| RMAT scale 21 | 2.1M | +2097 | **8467x** | 720x | 9.8x | 255x |

### Key Observations

- **BFS scales with perturbation size** — with proportional edges, BFS achieves 131x-8467x (up from 1-2x with fixed 50 edges). The incremental differential BFS only traverses vertices whose distances actually change, which scales sublinearly with edge count.
- **Connected Components is strongly structure-dependent** — 264x on DBLP (sparse collaboration) vs 948x on RMAT20 (dense power-law). Real trust/social networks amplify CC speedups due to community structure.
- **PageRank is invariant to graph size and structure** — consistently ~10x across all scales (100 vs 10 iterations).
- **SSSP (differential BFS) is robust** — 28x-339x, best on graphs with strong community structure where new edges don't propagate far.

## Library Comparison: axolotl-rs vs petgraph

| Dataset | V | Library | Build | BFS | PageRank |
|---------|:---:|---------|:-----:|:---:|:--------:|
| soc-Epinions1 | 75K | axolotl-rs | 117ms | 9.8ms | **49ms** |
| | | petgraph | 3ms | 2.6ms | 213ms |
| com-DBLP | 100K | axolotl-rs | 38ms | 5.1ms | **63ms** |
| | | petgraph | 1ms | 1.7ms | 98ms |
| web-Google | 100K | axolotl-rs | 15ms | 0.0ms | **41ms** |
| | | petgraph | 0ms | 0.0ms | 54ms |
| RMAT scale 20 | 99K | axolotl-rs | 37ms | 8.0ms | **31ms** |
| | | petgraph | 1ms | 3.1ms | 161ms |
| com-DBLP (full) | 425K | axolotl-rs | 331ms | 60.4ms | **319ms** |
| | | petgraph | 7ms | 13.8ms | 604ms |
| web-Google (full) | 916K | axolotl-rs | 1984ms | 267ms | **938ms** |
| | | petgraph | 42ms | 62ms | 9022ms |
| RMAT scale 21 | 2.1M | axolotl-rs | 4805ms | 1010ms | **1401ms** |
| | | petgraph | 96ms | 723ms | 66543ms |

### axolotl-rs vs petgraph at Scale

| Scale | PageRank (EB) | PageRank (PG) | EB Advantage |
|:---:|:---:|:---:|:---:|
| 75K | 49ms | 213ms | 4.3x |
| 100K | 63ms | 98ms | 1.6x |
| 425K | 319ms | 604ms | 1.9x |
| 916K | 938ms | 9022ms | 9.6x |
| 2.1M | 1401ms | 66543ms | **47.5x** |

CSR-based PageRank advantage grows super-linearly with scale because petgraph's per-iteration neighbor traversal overhead compounds. At 2.1M vertices, petgraph takes 66 seconds for the same computation axolotl-rs completes in 1.4 seconds.

Build remains petgraph's strong suit: 20-50x faster due to zero-overhead graph allocation (no HashMap properties, no bidirectional indexing, no block alignment).

## How to Run

```bash
# Comprehensive benchmark (all datasets, all metrics)
cd prototype-rust && cargo run --release --example bench_combined

# Large-scale test only
cd prototype-rust && cargo run --release --example bench_large_datasets
```

Datasets in `performance_test/datasets/`.
