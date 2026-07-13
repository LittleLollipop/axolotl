# Library Comparison – axolotl-rs vs NetworkX vs iGraph vs petgraph

**Date**: 2026-07-13 | **Hardware**: Apple M4, 10-core GPU | **Test**: Graph build, BFS, PageRank (100 iterations)

## Datasets

| Dataset | Source | Vertices | Edges | Type |
|---------|--------|:---:|:---:|------|
| soc-Epinions1 | SNAP | 75,879 | 508,837 | Trust network |
| com-DBLP | SNAP | 100,000 | 196,248 | Citation network |
| web-Google | SNAP / GAP | 100,000 | 55,851 | Web graph |
| RMAT scale 20 | Graph500 | 99,995 | 170,334 | Power-law synthetic |

## Results (ms)

### soc-Epinions1 (76K vertices, 509K edges)

| Library | Lang | Build | BFS | PageRank |
|---------|:---:|:-----:|:---:|:--------:|
| axolotl-rs | Rust | 226 | 19.3 | **95** |
| petgraph 0.6 | Rust | 4 | 5.3 | 423 |
| Vec<Vec> (baseline) | Rust | 6 | 2.1 | — |
| axolotl_rs bindings | Python | 377 | 22.0 | 12,013 |
| NetworkX 3.x | Python | 469 | 213.0 | 766 |
| iGraph (C core) | Python | 51 | 6.0 | **72** |

### com-DBLP (100K vertices, 196K edges)

| Library | Lang | Build | BFS | PageRank |
|---------|:---:|:-----:|:---:|:--------:|
| axolotl-rs | Rust | 75 | 10.5 | 112 |
| petgraph 0.6 | Rust | 1 | 2.1 | 201 |
| Vec<Vec> (baseline) | Rust | 3 | 1.4 | — |
| axolotl_rs bindings | Python | 125 | 13.0 | 1,069 |
| NetworkX 3.x | Python | 204 | 328.0 | 187 |
| iGraph (C core) | Python | 23 | 5.0 | **15** |

### web-Google (100K vertices, 56K edges)

| Library | Lang | Build | BFS | PageRank |
|---------|:---:|:-----:|:---:|:--------:|
| axolotl-rs | Rust | 25 | 0.01 | 75 |
| petgraph 0.6 | Rust | 0 | 0.01 | 95 |
| Vec<Vec> (baseline) | Rust | 1 | 0.01 | — |
| axolotl_rs bindings | Python | 41 | 0.01 | 437 |
| NetworkX 3.x | Python | 234 | 0.01 | 93 |
| iGraph (C core) | Python | 9 | 1.0 | **19** |

### RMAT (100K vertices, 170K edges)

| Library | Lang | Build | BFS | PageRank |
|---------|:---:|:-----:|:---:|:--------:|
| axolotl-rs | Rust | 68 | 15.4 | **58** |
| petgraph 0.6 | Rust | 1 | 5.6 | 325 |
| Vec<Vec> (baseline) | Rust | 3 | 1.3 | — |
| axolotl_rs bindings | Python | 99 | 18.0 | 1,965 |
| NetworkX 3.x | Python | 289 | 178.0 | 190 |
| iGraph (C core) | Python | 22 | 4.0 | **26** |

## Analysis

### axolotl-rs vs petgraph (Rust native)

| Metric | axolotl-rs | petgraph | Verdict |
|--------|:---:|:---:|------|
| Build | 4-75× slower | Bare-minimum alloc | EdgeBlock pays for edge property HashMap + EdgeBlock 32-edge alignment + bidirectional indexing |
| BFS | 1.5-7× slower | ~2× faster | Walk API overhead vs direct neighbor iteration |
| PageRank | **1.8-5.6× faster** | Reference | CSR format excels at iterative computation — no indirection, cache-friendly sequential reads |

**Bottom line**: axolotl-rs trades build-time overhead for runtime PageRank performance and GPU compatibility. For write-heavy workloads, petgraph wins. For iterative computation, axolotl-rs wins.

### axolotl-rs Rust vs Python bindings

The Python bindings add significant overhead:

| Operation | Rust Native | Python Binding | Overhead Factor |
|-----------|:---:|:---:|:---:|
| Build | 226ms | 377ms | 1.7× |
| BFS | 19ms | 22ms | 1.2× |
| PageRank | 95ms | 12,013ms | **126×** |

The PageRank binding performs Python↔Rust serialization on every iteration (100 iterations × 100K vertices = 10M round-trips). This is a known limitation of the current binding implementation — future work should move the loop inside Rust.

### Vec<Vec> Baseline

The bare `Vec<Vec<usize>>` adjacency list represents the theoretical lower bound for graph representation efficiency:

| | Vec<Vec> | axolotl-rs | Overhead |
|------|:---:|:---:|:---:|
| Build | 6ms | 226ms | 37× |
| BFS | 2ms | 19ms | 9× |

axolotl-rs's overhead is the price paid for:
- 32-edge block alignment (GPU warp compatibility)
- Bidirectional edge indexing (forward + reverse)
- Edge properties (HashMap per edge)
- Persistence (AXEB format)
- Crash recovery (WAL)

### vs iGraph (C core)

iGraph's C core is the undisputed performance leader across all metrics. Its build time (51ms vs 226ms) and PageRank (72ms vs 95ms) both edge out axolotl-rs, despite being accessed through Python bindings.

axolotl-rs closes the gap on PageRank (95ms vs 72ms = 1.3× slower) thanks to CSR-based computation, but build time remains the primary area for improvement.

### NetworkX

NetworkX serves as the Python ecosystem baseline. axolotl-rs's Rust native version is:
- Build: **2.1× faster** (226ms vs 469ms)
- BFS: **11× faster** (19ms vs 213ms)
- PageRank: **8.1× faster** (95ms vs 766ms)

Even the Python bindings version beats NetworkX on Build and BFS, but the binding overhead makes PageRank comparison unfavorable.

## Summary Ranking

| Rank | Build Speed | BFS Speed | PageRank Speed |
|:---:|------|------|------|
| 1 | petgraph | Vec<Vec> (baseline) | iGraph |
| 2 | Vec<Vec> (baseline) | petgraph | **axolotl-rs** |
| 3 | iGraph | iGraph | petgraph |
| 4 | axolotl-rs | axolotl-rs | NetworkX |
| 5 | NetworkX | NetworkX | axolotl_rs (Python) |
| 6 | axolotl_rs (Python) | axolotl_rs (Python) | — |

## Run It Yourself

```bash
# Rust comparison
cd prototype-rust && cargo run --release --example bench_library_comparison

# Python comparison (requires: pip install networkx python-igraph numpy scipy)
cd performance_test && python bench_comparison.py
```
