# Axolotl Graph Database Project

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Swift](https://img.shields.io/badge/Swift-6.0-orange.svg)](https://swift.org)
[![Platform](https://img.shields.io/badge/Platform-macOS%2014+-lightgrey.svg)]()
[![Metal](https://img.shields.io/badge/Metal-3.2-green.svg)](https://developer.apple.com/metal/)

**High-performance graph database with incremental algorithms and GPU acceleration for unified memory architectures (Apple Silicon)**

Axolotl is a research project exploring graph algorithm optimizations for unified memory architectures. It provides:

- 🧱 **EdgeBlock**: A novel graph data structure optimized for GPU coalesced memory access
- 🚀 **CPU+GPU Collaboration**: CPU schedules tasks, GPU executes computations (unified memory)
- ⚡ **Incremental Algorithms**: Only update affected vertices (PageRank, BFS, SSSP)
- 🔬 **Correctness First**: PageRank PR sum = 1.0 (handles dangling nodes correctly)

📖 **[中文文档](README_zh.md)** (to be added)

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

**Problem**: Traditional CSR (Compressed Sparse Row) format stores edges as contiguous arrays per vertex, but GPU access patterns suffer from non-coalesced memory access.

**Solution**: **EdgeBlock** groups edges into fixed-size blocks (32 edges per block), enabling coalesced GPU memory access.

**Results** (Swift prototype):
- ✅ 1.20x - 1.42x speedup for BFS on power-law graphs
- ✅ 1.27x speedup for PageRank
- ✅ Advantage increases with graph size (peak at 100K vertices)

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
| CPU (incorrect) | 0.37 | - | ❌ Bug |
| CPU (correct) | 1.0000 | < 1e-6 | ✅ Fixed |
| GPU Incremental | 1.0000 | < 1e-6 | ✅ Fixed |
| GPU Full | 1.0000 | < 1e-6 | ✅ Fixed |

### Incremental vs Full Recomputation (Swift Prototype)

| Algorithm | Full Time (ms) | Incremental Time (ms) | Speedup |
|-----------|-----------------|----------------------|---------|
| PageRank | 644.24 | 2.66 | **244x** |
| BFS | 151.47 | 1.89 | **80x** |
| SSSP | 180.16 | 2.15 | **84x** |
| Connected Components | 51.89 | 0.70 | **74x** |
| Triangle Counting | 12.86 | 0.0265 | **486x** |

**Experiment setup**:
- Hardware: Apple M4 (10-core GPU)
- Graph: 100K vertices, 500K edges (power-law)
- Change: Add 10-1000 edges

---

## Quick Start

### Rust Version (Recommended)

```bash
# Clone repository
git clone https://github.com/LittleLollipop/axolotl.git
cd axolotl/prototype-rust

# Build
cargo build --release

# Run PageRank test (verify correctness)
cargo run --release --example test_pagerank_fix

# Expected output:
# === PageRank Fix Test ===
# 
# Dataset: 1000 vertices, 5000 edges
# Damping factor: 0.85
# Iterations: 20
# 
# Results:
#   CPU PR sum = 1.0000
#   GPU Incremental PR sum = 1.0000
#   GPU Full PR sum = 1.0000
# 
# ✅ All implementations produce correct PR sum (1.0)
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

## Algorithm Details

### PageRank with Dangling Nodes

**Problem**: Vertices with no outgoing edges (dangling nodes) cause PR value "loss"

**Solution**: Add dangling contribution to all vertices

```rust
// Pseudo-code
let dangling: Vec<usize> = vertices.where(out_degree == 0);

for iteration in 0..max_iter {
    let dangling_sum: f32 = dangling.iter().map(|&v| pr[v]).sum();
    let dangling_contribution = dangling_sum / vertex_count as f32;
    
    for v in 0..vertex_count {
        let contribution = compute_contribution(v, pr, out_degrees);
        new_pr[v] = (1.0 - damping) / vertex_count as f32 
            + damping * (contribution + dangling_contribution);
    }
}
```

### Incremental PageRank (CPU+GPU Collaborative)

**Algorithm**:
1. **CPU**: Detect affected vertices (those whose scores changed)
2. **GPU**: Update PageRank scores for affected vertices in parallel
3. **CPU**: Check convergence, find newly affected vertices (propagation)
4. Repeat until convergence

**Key insight**: Changes propagate through the graph. If vertex v's score changes, all vertices pointing to v may need recomputation.

### EdgeBlock GPU Kernel (Metal)

```metal
kernel void pagerank_edgeblock_optimized(
    device const uint *affected_vertices [[buffer(0)]],
    constant uint &affected_count [[buffer(1)]],
    device const uint *reverse_vertices [[buffer(2)]],
    device const uint *reverse_block_counts [[buffer(3)]],
    device const EdgeBlock *reverse_blocks [[buffer(4)]],
    device const float *pr [[buffer(5)]],
    device float *new_pr [[buffer(6)]],
    device const uint *out_degrees [[buffer(7)]],
    constant float &damping_factor [[buffer(8)]],
    constant uint &vertex_count [[buffer(9)]],
    constant float &dangling_contribution [[buffer(10)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];
    float contribution = 0.0;
    
    // Traverse reverse edges (incoming edges)
    uint block_start = reverse_vertices[v];
    uint block_end = block_start + reverse_block_counts[v];
    
    for (uint block_idx = block_start; block_idx < block_end; block_idx++) {
        EdgeBlock block = reverse_blocks[block_idx];
        for (uint i = 0; i < block.edge_count; i++) {
            uint source = block.edges[i];
            uint source_out_degree = out_degrees[source];
            if (source_out_degree > 0) {
                contribution += pr[source] / float(source_out_degree);
            }
        }
    }
    
    float base_score = (1.0 - damping_factor) / float(vertex_count);
    new_pr[v] = base_score + damping_factor * (contribution + dangling_contribution);
}
```

---

## Technical Documentation

For detailed design decisions, benchmark results, and analysis, see:

- **Project Principles**: [`core-research/algorithm-research/PROJECT_PRINCIPLES.md`](core-research/algorithm-research/PROJECT_PRINCIPLES.md)
  - Design principles and philosophical decisions
  - Why we chose certain approaches
  
- **Incremental PageRank Status**: [`core-research/experiment-reports/INCREMENTAL_PR_STATUS.md`](core-research/experiment-reports/INCREMENTAL_PR_STATUS.md)
  - PageRank bug fix process
  - Correctness verification
  
- **Performance Comparison**: [`core-research/performance-tests/PERFORMANCE_COMPARISON.md`](core-research/performance-tests/PERFORMANCE_COMPARISON.md)
  - Benchmark results
  - Comparison with NetworkX and Neo4j

- **Swift Technical Report**: [`prototype-swift/Docs/technical_report.md`](prototype-swift/Docs/technical_report.md)
  - EdgeBlock design and implementation
  - Performance benchmarks (BFS, PageRank)
  - Analysis of failed attempts

---

## Future Work

### Short-term (1-2 months)

- [ ] **Performance comparison**: Axolotl vs NetworkX vs Neo4j (end-to-end)
- [ ] **Incremental BFS**: Apply CPU+GPU collaboration to BFS (Rust)
- [ ] **Incremental SSSP**: Single Source Shortest Path incremental update (Rust)
- [ ] **Complete PageRank**: Both incremental and full versions (GPU)

### Medium-term (3-6 months)

- [ ] **More algorithms**: Connected Components, Triangle Counting, Community Detection
- [ ] **Larger graphs**: Test on 1M+ vertex graphs
- [ ] **Memory optimization**: Reduce memory footprint for large graphs
- [ ] **Multi-GPU support**: Utilize multiple GPU cores (M4 has 10 GPU cores)

### Long-term (6-12 months)

- [ ] **Simple query interface**: Basic graph queries (neighbors, paths, rankings)
- [ ] **Persistence**: Binary graph format for efficient storage/loading
- [ ] **Port to other architectures**: Intel Arc, NVIDIA Grace (unified memory)
- [ ] **Academic paper**: Submit to conferences (SIGMOD, VLDB, SC)

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
  author = {LittleLollipop},
  title = {Axolotl: Unified Memory Graph Algorithms for Apple Silicon},
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
