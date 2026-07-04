# Axolotl

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Swift](https://img.shields.io/badge/Swift-6.0-orange.svg)](https://swift.org)
[![Platform](https://img.shields.io/badge/Platform-macOS%2014+-lightgrey.svg)]()

**Unified Memory Graph Algorithms for Apple Silicon**

Axolotl is a research prototype exploring graph algorithm optimizations for unified memory architectures (like Apple M4). It features:

- 🧱 **EdgeBlock**: A novel graph data structure optimized for GPU coalesced memory access
- 🚀 **CPU+GPU Collaboration**: First successful implementation of heterogeneous graph algorithms on unified memory
- ⚡ **Incremental Updates**: 244x speedup for incremental PageRank on dynamic graphs

---

## Quick Start

### Prerequisites

- macOS 14.0+
- Xcode 16.0+ (for Metal 3.2)
- Swift 6.0+

### Run Experiments

```bash
# Incremental PageRank (CPU+GPU collaborative)
cd Experiments
swift incremental_pagerank.swift

# PageRank comparison (EdgeBlock vs CSR)
swift pagerank.swift

# BFS comparison (EdgeBlock vs CSR)
swift edgeblock_vs_csr.swift
```

---

## Performance Results

### EdgeBlock vs CSR (GPU BFS)

| Vertex Count | EdgeBlock Time | CSR Time | Speedup |
|--------------|----------------|----------|---------|
| 10K | 0.0156s | 0.0188s | **1.20x** |
| 100K | 0.0677s | 0.0958s | **1.42x** |

### Incremental PageRank (CPU+GPU)

| Method | Time (ms) | Speedup |
|--------|-----------|---------|
| Full PageRank (10 iter) | 644.24 | 1.00x |
| Incremental PageRank | 2.66 | **244x** |

---

## Project Structure

```
axolotl/
├── Experiments/               # Experiment code and benchmarks
├── Docs/                      # Documentation
├── README.md
└── LICENSE
```

---

## Technical Report

See [`Docs/technical_report.md`](Docs/technical_report.md) for detailed design and benchmarks.

---

## License

MIT License
