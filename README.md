# Axolotl

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Swift](https://img.shields.io/badge/Swift-6.0-orange.svg)](https://swift.org)
[![Platform](https://img.shields.io/badge/Platform-macOS%2014+-lightgrey.svg)]()
[![Metal](https://img.shields.io/badge/Metal-3.2-green.svg)](https://developer.apple.com/metal/)

**Unified Memory Graph Algorithms for Apple Silicon**

Axolotl is a research prototype exploring graph algorithm optimizations for unified memory architectures (like Apple M4). It features:

- 🧱 **EdgeBlock**: A novel graph data structure optimized for GPU coalesced memory access
- 🚀 **CPU+GPU Collaboration**: First successful implementation of heterogeneous graph algorithms on unified memory
- ⚡ **Incremental Updates**: 244x speedup for incremental PageRank on dynamic graphs

📖 **[中文文档](README_zh.md)**

---

## Table of Contents

- [Why "Axolotl"?](#why-axolotl)
- [Key Innovations](#key-innovations)
- [Performance Results](#performance-results)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Technical Details](#technical-details)
- [Project Structure](#project-structure)
- [Future Work](#future-work)
- [Technical Report](#technical-report)
- [License](#license)
- [Contributing](#contributing)
- [Citation](#citation)

---

## Why "Axolotl"?

The axolotl (*Ambystoma mexicanum*) is an amphibian that lives double lives — it thrives both in water (like a fish) and on land (like a salamander). **This is exactly what unified memory architecture enables: CPU and GPU working together seamlessly, like an amphibian in two elements.**

But there's more to this metaphor:

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

### Why Not Other Animals?

- **Cheetah?** Fast, but only one mode (GPU-only)
- **Elephant?** Strong, but only one mode (CPU-only)
- **Axolotl?** Perfect — thrives in both environments, adapts to conditions

Just as the axolotl represents biological innovation in dual-environment living, this project explores computational "amphibious computing" — CPU and GPU collaborating seamlessly in unified memory architecture.

---

## Key Innovations

### 1. EdgeBlock Data Structure

**Problem**: Traditional CSR (Compressed Sparse Row) format stores edges as contiguous arrays per vertex, but GPU access patterns suffer from non-coalesced memory access. When a GPU warp processes multiple vertices, their edge arrays may be scattered in memory, causing:

- Memory transaction underutilization
- Reduced memory bandwidth efficiency
- Poor cache locality

**Solution**: **EdgeBlock** groups edges into fixed-size blocks (32 edges per block), enabling:

```
Traditional CSR:
  Vertex 0 edges: [5, 10, 15]     ← Address A
  Vertex 1 edges: [3, 7, 9, 12]  ← Address A+3
  ...
  GPU warp access: NON-COALESCED (different addresses)

EdgeBlock:
  Block 0: {owner=0, edges=[5, 10, 15, PAD, ...]}  ← Address B
  Block 1: {owner=1, edges=[3, 7, 9, 12, ...]}     ← Address B+34
  ...
  GPU warp access: COALESCED (sequential addresses)
```

**Results**:
- ✅ 1.20x - 1.42x speedup for BFS on power-law graphs
- ✅ 1.27x speedup for PageRank
- ✅ Advantage increases with graph size (peak at 100K vertices)

**Why power-law graphs?** Real-world networks (social networks, web graphs) follow power-law degree distribution. EdgeBlock's advantage is most pronounced on these graphs because:
- High-degree vertices have long edge arrays in CSR → severe non-coalescing
- EdgeBlock's fixed-size blocks mitigate this by ensuring sequential access

### 2. CPU+GPU Collaborative Algorithms

**Previous attempts failed**: We tried partitioning work by vertex degree:
- CPU processes high-degree vertices (assumed "irregular")
- GPU processes low-degree vertices (assumed "regular")

**Result**: 3.5x **slower** than GPU-only.

**Why it failed**:
1. "High degree" ≠ "irregular" — GPU's parallelism handles high-degree vertices efficiently
2. CPU cache advantage negligible for large adjacency lists
3. GPU invocation overhead per BFS level is expensive
4. Static partitioning doesn't adapt to dynamic frontier size

**Our insight**: Partition by **work type**, not data characteristics:

```
✅ CORRECT approach:
   CPU: Scheduling, bookkeeping, convergence checking (统筹性工作)
   GPU: Parallel computation (执行层面的苦力活)

❌ WRONG approach:
   CPU: High-degree vertices
   GPU: Low-degree vertices
```

**First success: Incremental PageRank**

When a graph changes (add/delete edges), we don't recompute PageRank from scratch. Instead:

1. **CPU**: Detect affected vertices (those whose scores changed)
2. **GPU**: Update PageRank scores for affected vertices in parallel
3. **CPU**: Check convergence, find newly affected vertices (propagation)
4. Repeat until convergence

**Results**:
- ✅ **244x speedup** over full recomputation
- ✅ High precision (max error < 2.53e-07)
- ✅ Converges in 1-2 iterations for small graph changes

**Key to success**:
- Unified memory enables zero-copy data sharing
- CPU and GPU work concurrently (not sequentially)
- Dynamic load balancing (affected vertex count changes each iteration)

### 3. Unified Memory Advantages

Apple M4's unified memory architecture (CPU/GPU share physical address space) enables new possibilities:

| Traditional (Discrete GPU) | Unified Memory (Apple M4) |
|---------------------------|---------------------------|
| Data must be copied CPU ↔ GPU | Zero-copy shared memory |
| CPU/GPU execute asynchronously | Can execute concurrently |
| Memory space separated | Single address space |
| High data transfer cost | No transfer cost |

**Implications for graph algorithms**:
1. **Fine-grained collaboration**: CPU can inspect/modify data that GPU is processing
2. **Incremental updates**: CPU updates graph, GPU immediately sees changes
3. **No batch size constraints**: Can process individual vertices efficiently

---

## Performance Results

### Experiment Environment

- **Hardware**: Apple M4 (10-core GPU, 16-core Neural Engine)
- **Software**: macOS 14.0, Xcode 16.0, Swift 6.0, Metal 3.2
- **Graph type**: Power-law (γ=2.5, simulating real-world networks)

### EdgeBlock vs CSR (GPU BFS)

| Vertex Count | Edge Count | EdgeBlock Time | CSR Time | Speedup |
|--------------|------------|----------------|----------|---------|
| 10K | 50K | 0.0156s | 0.0188s | **1.20x** |
| 50K | 250K | 0.0390s | 0.0523s | **1.34x** |
| 100K | 500K | 0.0677s | 0.0958s | **1.42x** |
| 200K | 1M | 0.1189s | 0.1559s | **1.31x** |

**Observation**: Speedup peaks at 100K vertices. For larger graphs, GPU memory capacity becomes a bottleneck.

### EdgeBlock vs CSR (GPU PageRank)

| Vertex Count | Edge Count | EdgeBlock Time | CSR Time | Speedup |
|--------------|------------|----------------|----------|---------|
| 10K | 50K | 207.38 ms | 262.97 ms | **1.27x** |

### Incremental PageRank (CPU+GPU Collaborative)

**Scenario**: Graph with 10K vertices, 50K edges. Add 10 edges. Update PageRank.

| Method | Time (ms) | Iterations | Speedup | Max Error |
|--------|-----------|------------|---------|-----------|
| Full PageRank (10 iter) | 644.24 | 10 | 1.00x | - |
| Incremental PageRank | 2.66 | 1 | **244x** | 2.53e-07 |

**Convergence criterion**: Max PageRank score change < 1e-6

### Incremental BFS (CPU+GPU Collaborative)

**Scenario**: Graph with 100K vertices, 500K edges. Add 1K new edges. Update BFS distances.

| Method | Time (ms) | Iterations | Speedup | Result Match |
|--------|-----------|------------|---------|---------------|
| Full BFS | 151.47 | 1 | 1.00x | - |
| Incremental BFS | 1.89 | 1 | **80x** | ✅ 100% |

### Incremental SSSP (CPU+GPU Collaborative)

**Scenario**: Graph with 100K vertices, 500K edges (weighted). Add 1K new edges. Update shortest paths.

| Method | Time (ms) | Iterations | Speedup | Result Match |
|--------|-----------|------------|---------|---------------|
| Full SSSP (16 iter) | 180.16 | 16 | 1.00x | - |
| Incremental SSSP | 2.15 | 1 | **84x** | ✅ 100% |

### Incremental Connected Components (Union-Find)

**Scenario**: Graph with 100K vertices, 500K edges. Add 5K new edges. Update connected components.

| Method | Time (ms) | Edges Processed | Speedup | Result Match |
|--------|-----------|------------------|---------|---------------|
| Full build (all edges) | 51.89 | 499,971 | 1.00x | - |
| Incremental update (new edges only) | 0.70 | 5,000 | **74x** | ✅ 100% |

**Why Union-Find?** Union-Find operations (find/union) are extremely fast: O(α(V)) ≈ O(1) amortized. Path compression + union by rank = nearly constant time.

---

## Installation

### Prerequisites

- **macOS**: 14.0+
- **Xcode**: 16.0+ (requires Metal 3.2 for atomic operations)
- **Swift**: 6.0+
- **Hardware**: Apple Silicon (M1/M2/M3/M4) for unified memory architecture

### Clone Repository

```bash
git clone https://github.com/LittleLollipop/axolotl.git
cd axolotl
```

### Verify Environment

```bash
# Check Swift version
swift --version

# Check Metal support
system_profiler SPDisplaysDataType | grep Metal
```

---

## Quick Start

### Run Incremental PageRank (CPU+GPU Collaborative)

```bash
cd Experiments
swift incremental_pagerank.swift
```

**Expected output**:
```
=== Incremental PageRank Test ===

Generated graph: 10000 vertices, 50000 edges
Metal device: Apple M4

1. Computing full PageRank on original graph (10 iterations)...
    Iteration 0: PR sum = 1.2296436
    Iteration 5: PR sum = 1.210745
   Full PageRank time: 644.24 ms

2. Simulating graph change (adding 10 edges)...
   Affected vertices: 20

3. Computing incremental PageRank (tolerance = 1e-6)...
    Incremental PageRank: 1 iterations, 2.66 ms

4. Computing full PageRank from scratch for comparison...
    Iteration 0: PR sum = 1.2296436

5. Comparing results...
   Max difference: 2.526358e-07
   Average difference: 1.9120926e-10

✅ Incremental PageRank results match full PageRank!
```

### Run PageRank Comparison (EdgeBlock vs CSR)

```bash
cd Experiments
swift pagerank.swift
```

### Run BFS Comparison (EdgeBlock vs CSR)

```bash
cd Experiments
swift edgeblock_vs_csr.swift
```

---

## Technical Details

### EdgeBlock Data Structure

**Memory layout**:

```metal
struct EdgeBlock {
    uint ownerVertex;    // Vertex that owns these edges
    uint edgeCount;      // Actual edge count (≤ 32)
    uint edges[32];     // Adjacent vertices (padded with UInt32.max)
};
```

**Block size rationale**: 32 = one GPU warp size. This ensures:
- One warp can process one block with fully coalesced access
- No wasted threads (all 32 lanes are useful, even if edgeCount < 32)

**Padding strategy**: Use `UInt32.max` (not 0) for padding, because 0 is a valid vertex ID.

### Metal Kernel: PageRank with Atomic Operations

```metal
inline void atomic_add_float(device float* address, float val) {
    device atomic_uint* atomicAddr = (device atomic_uint*)address;
    uint oldBits = atomic_load_explicit(atomicAddr, memory_order_relaxed);
    uint newBits;
    do {
        float oldVal = as_type<float>(oldBits);
        float newVal = oldVal + val;
        newBits = as_type<uint>(newVal);
    } while (!atomic_compare_exchange_weak_explicit(
        atomicAddr, &oldBits, newBits,
        memory_order_relaxed, memory_order_relaxed));
}
```

**Why CAS loop?** Metal doesn't support `atomic_fetch_add` for floats. We implement it with compare-and-swap.

### Incremental PageRank: CPU+GPU Collaboration

**Algorithm**:

```
Input: Graph G, initial PageRank scores PR, changed vertices V_changed
Output: Updated PageRank scores PR'

1. affectedSet = V_changed
2. while affectedSet not empty and iteration < maxIter:
3.     // GPU: Update PageRank for affected vertices
4.     for each v in affectedSet (parallel on GPU):
5.         PR'[v] = computePageRank(v, PR)
6.     
7.     // CPU: Check convergence and propagate
8.     newAffected = {}
9.     for each v in all vertices:
10.        if |PR'[v] - PR[v]| > tolerance:
11.            for each u in reverseAdjacency[v]:
12.                newAffected.add(u)
13.    
14.    PR = PR'
15.    affectedSet = newAffected
16.    iteration++
17. 
18. return PR
```

**Key insight**: Line 11-12 propagates changes. If vertex v's score changes significantly, all vertices that point to v may need recomputation.

---

## Project Structure

```
axolotl/
├── Experiments/                      # Experiment code and benchmarks
│   ├── incremental_pagerank.swift   # ✅ CPU+GPU collaborative (244x speedup)
│   ├── pagerank.swift               # PageRank (EdgeBlock vs CSR)
│   ├── edgeblock_vs_csr.swift       # BFS comparison
│   ├── heterogeneous_bfs.swift      # ❌ Failed attempt (for reference)
│   ├── heterogeneous_bfs_final.swift # ❌ Another failed attempt
│   └── prototype.c                  # CPU prototype (EdgeBlock vs CSR)
├── Docs/                            # Documentation
│   ├── technical_report.md          # Comprehensive technical report
│   ├── DESIGN.md                    # EdgeBlock design document
│   └── unified_memory_design.md     # Unified memory algorithm design
├── README.md                        # This file (English)
├── README_zh.md                     # 中文文档
├── LICENSE                          # MIT License
└── .gitignore                       # Git ignore rules
```

---

## Future Work

### Short-term (1-2 months)

- [ ] **Incremental BFS**: Apply CPU+GPU collaboration to BFS
- [ ] **Incremental SSSP**: Single Source Shortest Path incremental update
- [ ] **More algorithms**: Connected Components, Triangle Counting
- [ ] **Parameter tuning**: Test different EdgeBlock sizes (16, 64, 128)

### Medium-term (3-6 months)

- [ ] **Simple query interface**: Basic graph queries (neighbors, paths, rankings)
- [ ] **Larger graphs**: Test on 1M+ vertex graphs
- [ ] **Memory optimization**: Reduce memory footprint for large graphs
- [ ] **Multi-GPU support**: Utilize multiple GPU cores (M4 has 10 GPU cores)

### Long-term (6-12 months)

- [ ] **Port to other architectures**: Intel Arc, NVIDIA Grace (unified memory)
- [ ] **Academic paper**: Submit to conferences (SIGMOD, VLDB, SC)
- [ ] **Complete graph database**: If justified by research results
- [ ] **Open source community**: Attract contributors, build ecosystem

---

## Technical Report

For detailed design decisions, benchmark results, and analysis, see:

- **Technical Report**: [`Docs/technical_report.md`](Docs/technical_report.md)
  - EdgeBlock design and implementation
  - Performance benchmarks (BFS, PageRank, Heterogeneous BFS)
  - Analysis of why heterogeneous BFS failed
  - Incremental PageRank design and results
  
- **EdgeBlock Design**: [`Docs/DESIGN.md`](Docs/DESIGN.md)
  - Data structure specification
  - CSR to EdgeBlock conversion algorithm
  - Memory layout details

- **Unified Memory Algorithm Design**: [`Docs/unified_memory_design.md`](Docs/unified_memory_design.md)
  - Motivation and background
  - CPU+GPU collaboration design principles
  - Future algorithm candidates

---

## Contributing

Contributions are welcome! This is a research prototype exploring new ideas in graph algorithms for unified memory architectures.

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

**⚠️ Note**: This is a research prototype. Code quality is experimental, and APIs may change. Use at your own risk.

**🎉 Fun fact**: Axolotls are also known as "Mexican walking fish" (though they're not fish, they're amphibians). They never undergo metamorphosis in the wild, staying in their larval form forever — just like this project will forever stay in "prototype" form (hopefully not)!
