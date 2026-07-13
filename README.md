# Axolotl Graph Database Project

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Swift](https://img.shields.io/badge/Swift-6.0-orange.svg)](https://swift.org)
[![Platform](https://img.shields.io/badge/Platform-macOS%2014+-lightgrey.svg)]()
[![Metal](https://img.shields.io/badge/Metal-3.2-green.svg)](https://developer.apple.com/metal/)

**High-performance graph database with incremental algorithms and GPU acceleration, designed for unified memory architectures**

🧱 **EdgeBlock**: A new class of data structure for the unified memory era — balancing GPU warp coalescing with CPU mutability  
🚀 **CPU+GPU Collaboration**: CPU schedules, GPU computes — on the same data, zero-copy  
⚡ **Incremental Algorithms**: BFS **1580x**, Connected Components **4920x** over full recomputation (CPU+GPU collaborative)  
🔬 **Correctness First**: PageRank PR sum = 1.0, 77 unit tests verified  

📖 **[中文文档](README_zh.md)** | 📄 **[技术报告 (arXiv draft)](core-research/docs/EdgeBlock-Technical-Report.md)**

---

## Rust Implementation

The Rust implementation is the **current development version**, providing:

- ✅ **EdgeBlock Native Storage**: Unified memory format with CRUD, binary persistence (AXEB), mmap support
- ✅ **REST API Server**: Full CRUD + algorithms + traversal + management endpoints (16-thread pool)
- ✅ **Crash Recovery**: WAL replay on startup, automatic after unclean shutdown
- ✅ **Workload Persistence**: Start from file + auto-save on shutdown
- ✅ **Edge Properties**: Weight + custom properties per edge, stored alongside topology
- ✅ **Graph Traversal**: `walk()`, `subgraph()`, `find_paths()` with property filtering
- ✅ **Correct PageRank**: Handles dangling nodes (PR sum = 1.0)
- ✅ **GPU Acceleration**: Metal kernels for PageRank, BFS, SSSP
- ✅ **Incremental Algorithms**: Only update affected vertices (5 algorithms, 77 tests)

**Quick Start**:
```bash
cd prototype-rust

# Run all tests
cargo test --lib                     # 77 passed

# Start REST API server
cargo run --example server -- --data data/graph.axeb
# Server at http://localhost:8080

# Run benchmarks
cargo run --release --example bench_incremental
```

**Documentation**: 
- [Rust Implementation Guide](prototype-rust/README.md)
- [Incremental Performance Report](core-research/experiment-reports/INCREMENTAL_PERF_VERIFICATION.md)
- [EdgeBlock Refactoring Benchmark](core-research/performance-tests/EDGEBLOCK_REFACTOR_BENCH.md)

---

## Table of Contents

- [Why "Axolotl"?](#why-axolotl)
- [Project Structure](#project-structure)
- [Core Innovations](#core-innovations)
- [Performance Results](#performance-results)
- [Quick Start](#quick-start)
- [Algorithm Details](#algorithm-details)
- [Technical Documentation](#technical-documentation)
- [Future Work](#future-work)
- [Contributing](#contributing)
- [Citation](#citation)
- [License](#license)

---

## Why "Axolotl"?

The axolotl (*Ambystoma mexicanum*) is an amphibian that lives double lives — it thrives both in water (like a fish) and on land (like a salamander). **This is exactly what unified memory architecture enables: CPU and GPU working together seamlessly, like an amphibian in two elements.**

| Axolotl's Trait | Unified Memory Analogy |
|-----------------|------------------------|
| **Amphibious** (water + land) | **CPU + GPU** working together (unified memory) |
| **Regeneration** (regrow limbs) | **Incremental updates** (only update affected parts) |
| **Adaptability** (two environments) | **Dynamic workload balancing** (CPU/GPU adapt to task type) |
| **Efficiency** (minimal energy waste) | **Zero-copy memory** (no data transfer overhead) |

### The Deeper Connection

Traditional computing is like a fish out of water — CPU and GPU work in separate environments:
- **Fish (GPU)**: Great in its own element (parallel computation), but struggles on land (can't efficiently handle irregular tasks)
- **Land animal (CPU)**: Great on land (serial tasks, complex logic), but struggles in water (slow at parallel computation)

**Unified memory is the amphibious solution**:
- CPU and GPU share the same environment (unified memory)
- They can both work efficiently, each doing what they're best at
- No "environmental barrier" (data transfer cost) between them

---

## Project Structure

This repository contains multiple implementations and research materials:

```
axolotl/
├── core-research/                  # Research reports and experiments
│   ├── experiment-reports/         # Experiment records and results
│   ├── algorithm-research/         # Algorithm design and analysis
│   ├── performance-tests/          # Performance benchmarks
│   └── docs/                      # Technical documentation
├── prototype-swift/               # Swift prototype (reference implementation)
│   ├── Experiments/               # Experiment code (Swift)
│   ├── Docs/                     # Design documents
│   └── Sources/                  # Swift source code
├── prototype-rust/                # Rust implementation (current development)
│   ├── src/                      # Rust source code
│   ├── examples/                 # Example programs and tests
│   └── benches/                  # Benchmarks
└── README.md                     # This file
```

### Branch Organization

- `dev`: Current development branch (default)
- `swift`: Swift prototype (from `main` branch)
- `rust`: Rust implementation (from `master` branch)

---

## Core Innovations

### 1. EdgeBlock Data Structure

**Problem**: Traditional CSR stores edges as contiguous arrays per vertex. GPU access patterns suffer from non-coalesced memory access. Worse, CSR forces a hard choice: it is GPU-friendly but hostile to CPU incremental updates—each edge insertion requires rebuilding the entire offset array.

**Why it matters now**: Unified memory removes the physical separation between CPU and GPU memory. This eliminates the luxury of choosing one format over the other. A data structure on unified memory must serve both: flat enough for GPU warp coalescing, mutable enough for CPU incremental updates. EdgeBlock is a first attempt at resolving this structural tension.

**Solution**: **EdgeBlock** groups edges into fixed-size blocks (32 edges = GPU warp width). Blocks are stored contiguously in a single `Vec<u32>` shared by CPU and GPU via Metal's `StorageModeShared`. Edge insertion is O(1) amortized—append to the last block, create a new one when full. No format conversion needed between CPU traversal and GPU kernel execution.

**Results**:
- ✅ 61–83% faster data loading vs. CSR pipelines (no two-pass construction)
- ✅ **1580×** incremental BFS, **4920×** incremental CC

### 2. CPU+GPU Collaborative Algorithms

**Key Insight**: Partition by **work type**, not data characteristics:

```
✅ CORRECT approach:
   CPU: Scheduling, bookkeeping, convergence checking
   GPU: Parallel computation

❌ WRONG approach:
   CPU: High-degree vertices
   GPU: Low-degree vertices
```

**Successful implementations**:
- **Incremental PageRank**: 244x speedup (Swift prototype)
- **Incremental BFS**: 80x speedup (Swift prototype)
- **Incremental SSSP**: 84x speedup (Swift prototype)

### 3. Correct PageRank Implementation

**Critical fix**: Handle dangling nodes (vertices with out-degree = 0)

PageRank formula:
```
PR(v) = (1-d)/N + d × Σ PR(u) / out_degree(u)
```

**Problem**: Dangling nodes (out-degree = 0) cause PR sum ≠ 1.0

**Solution**: Distribute dangling nodes' PR values uniformly to all vertices

**Status**:
- ✅ CPU version: PR sum = 1.0
- ✅ GPU incremental version: PR sum = 1.0
- ✅ GPU full version: PR sum = 1.0

---

## Performance Results

### PageRank Correctness (Rust Implementation)

| Implementation | PR Sum | Max Error | Status |
|----------------|--------|-----------|--------|
| CPU (correct) | 1.0000 | < 1e-6 | ✅ |
| GPU Incremental | 1.0000 | < 1e-6 | ✅ |
| GPU Full | 1.0000 | < 1e-6 | ✅ |

### Incremental Algorithm Speedup (Rust, 50K vertices, +50 edges)

| Algorithm | Full (ms) | Incremental (ms) | Speedup |
|-----------|-----------|-------------------|---------|
| BFS | 1.91 | 0.001 | **1580x** |
| Connected Components | 7.38 | 0.002 | **4920x** |
| PageRank | 59.85 | 3.17 | **18.9x** |

> **Reference (CPU-only)**: SSSP uses differential BFS achieving 197x (4.18ms → 0.021ms). Triangle Counting achieves 2.0x (48.95ms → 24.45ms). Both are CPU implementations without GPU acceleration, included as reference data points.
>
> Speedups reflect best-case (50/50,000 vertices affected). Triangle Counting uses 10 new edges. These are upper bounds — see [technical report](core-research/docs/EdgeBlock-Technical-Report.md) for details.

### Standard Graph Datasets (100K vertices, +50 random edges)

| Dataset | V | E | BFS | CC | PageRank | SSSP |
|---------|:---:|:---:|:---:|:---:|:--------:|:----:|
| soc-Epinions1 (SNAP) | 76K | 509K | 2x | **7182x** | 8.8x | **427x** |
| com-DBLP (SNAP) | 100K | 196K | 2x | **2249x** | 8.7x | 67x |
| web-Google (GAP) | 100K | 56K | 1x | 937x | 10.3x | 95x |
| RMAT scale 20 | 100K | 170K | 1x | **2074x** | 10.3x | **233x** |

> **Note**: CC achieves 937x-7182x on real graphs — higher than synthetic benchmarks because real networks have strong community structure, making incremental updates nearly free. Full benchmark details: [Standard Dataset Benchmark](core-research/performance-tests/BENCHMARK_STANDARD_DATASETS.md).

### GPU Buffer Cache Reuse (Rust)

| Scale | PageRank | BFS | SSSP |
|-------|:---:|:---:|:---:|
| 1K/5K | 22% | 21% | 24% |
| 100K/1M | 25% | 24% | 25% |

### Incremental vs Full Recomputation (Swift Prototype, Reference)

| Algorithm | Full Time (ms) | Incremental Time (ms) | Speedup |
|-----------|-----------------|----------------------|---------|
| PageRank | 644.24 | 2.66 | **244x** |
| BFS | 151.47 | 1.89 | **80x** |
| SSSP | 180.16 | 2.15 | **84x** |
| Connected Components | 51.89 | 0.70 | **74x** |
| Triangle Counting | 12.86 | 0.0265 | **486x** |

**Experiment setup**: Apple M4 (10-core GPU), power-law graphs

---

## Quick Start

### Rust Version (Recommended)

```bash
git clone https://github.com/LittleLollipop/axolotl.git
cd axolotl/prototype-rust

# Run all 77 tests
cargo test --lib

# Start REST API server (with persistence)
cargo run --example server -- --data data/graph.axeb

# In another terminal:
curl http://localhost:8080/health
# → {"status":"ok","name":"Axolotl GraphDB"}

curl http://localhost:8080/stats
# → {"vertex_count":2,"edge_count":1,"mode":"in_memory"}

# Add a vertex
curl -X POST http://localhost:8080/vertices \
  -H "Content-Type: application/json" \
  -d '{"id":42,"properties":{"name":"test"}}'

# Run PageRank
curl -X POST http://localhost:8080/algorithms/pagerank \
  -H "Content-Type: application/json" \
  -d '{"iterations":50}'

# Save and shutdown
curl -X POST http://localhost:8080/admin/shutdown
```

### Swift Version (Reference)

```bash
cd prototype-swift

# Build
swift build

# Run incremental PageRank experiment
cd Experiments
swift incremental_pagerank.swift
```

---

## Technical Documentation

- **[EdgeBlock Technical Report](core-research/docs/EdgeBlock-Technical-Report.md)** — arXiv preprint draft
- **[Rust Implementation Guide](prototype-rust/README.md)** — Full API reference, architecture, examples
- **[Transaction Design](prototype-rust/TRANSACTION_DESIGN.md)** — WAL, MVCC, crash recovery design
- **[Project Principles](core-research/algorithm-research/PROJECT_PRINCIPLES.md)** — Design philosophy
- **[Incremental Performance Report](core-research/experiment-reports/INCREMENTAL_PERF_VERIFICATION.md)** — Actual speedup measurements
- **[EdgeBlock Refactoring Benchmark](core-research/performance-tests/EDGEBLOCK_REFACTOR_BENCH.md)** — GPU buffer cache analysis
- **[Incremental PageRank Status](core-research/experiment-reports/INCREMENTAL_PR_STATUS.md)** — Correctness fix history
- **[Performance Comparison](core-research/performance-tests/PERFORMANCE_COMPARISON.md)** — vs NetworkX, Neo4j
- **[Swift Technical Report](prototype-swift/Docs/technical_report.md)** — Original EdgeBlock design

---

## Future Work

- [x] ~~Performance comparison: Axolotl vs NetworkX vs Neo4j~~ → [See benchmark](core-research/experiment-reports/INCREMENTAL_PERF_VERIFICATION.md)
- [x] ~~Incremental BFS: CPU+GPU collaboration (Rust)~~
- [x] ~~Incremental SSSP: Single Source Shortest Path (Rust)~~
- [x] ~~PageRank: Both incremental and full versions (GPU)~~
- [x] ~~More algorithms: Connected Components, Triangle Counting~~
- [x] ~~Persistence: Binary graph format~~
- [x] ~~Simple query interface: neighbors, paths, rankings~~
- [x] ~~REST API server~~
- [x] ~~Python bindings: PyO3 integration~~ → `pip install` ready
- [ ] **Incremental PageRank performance optimization**: GPU kernel path improvements
- [ ] **Larger graphs**: Test on 10M+ vertex graphs
- [ ] **Multi-GPU support**: Utilize multiple GPU cores
- [ ] **Distributed support**: Sharding/replication for multi-machine graphs
- [ ] **Port to other architectures**: Intel Arc, NVIDIA Grace (unified memory)
- [ ] **Academic paper**: Submit to conferences

---

## Contributing

Contributions are welcome! This is a research project exploring new ideas in graph algorithms for unified memory architectures.

### How to Contribute

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-idea`)
3. Commit your changes (`git commit -m 'Add amazing idea'`)
4. Push to the branch (`git push origin feature/amazing-idea`)
5. Open a Pull Request

### Research Collaboration

If you're interested in collaborating on research (unified memory graph algorithms, heterogeneous computing), please reach out!

**Areas we'd love help with**:
- More CPU+GPU collaborative algorithms
- Performance optimization (Metal kernel tuning)
- Porting to other unified memory architectures
- Theoretical analysis (why does EdgeBlock work?)

---

## Citation

If you use Axolotl in your research, please cite:

```bibtex
@software{axolotl2026,
  author = {Yan, Lu},
  title = {Axolotl: Unified Memory Graph Algorithms with EdgeBlock Format},
  year = {2026},
  url = {https://github.com/LittleLollipop/axolotl}
}
```

---

## Acknowledgments

- Inspired by [Gunrock](https://github.com/gunrock/gunrock), [CuGraph](https://github.com/rapidsai/cugraph), and [GraphBLAST](https://github.com/gunrock/graphblast)
- Built on Apple's [Metal](https://developer.apple.com/metal/) framework
- Tested on Apple M4 (unified memory architecture)

---

## License

This project is licensed under the MIT License - see [`LICENSE`](LICENSE) for details.

---

## Contact

- **GitHub Issues**: [Report bugs or request features](https://github.com/LittleLollipop/axolotl/issues)
- **Discussions**: [Join the discussion](https://github.com/LittleLollipop/axolotl/discussions)

---

**⚠️ Note**: This is a research project. Code quality is experimental, and APIs may change. Use at your own risk.

**🎉 Fun fact**: Axolotls are also known as "Mexican walking fish" (though they're not fish, they're amphibians). They never undergo metamorphosis in the wild, staying in their larval form forever — just like this project will forever stay in "prototype" form (hopefully not)!
