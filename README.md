# Axolotl Graph Database Project

High-performance graph database with incremental algorithms and GPU acceleration.

## Project Structure

```
axolotl/
├── core-research/          # Research reports, algorithm studies, performance tests
│   ├── experiment-reports/ # Experiment records and results
│   ├── algorithm-research/ # Algorithm design and analysis
│   ├── performance-tests/  # Performance benchmarks
│   └── docs/              # Technical documentation
├── prototype-swift/       # Swift prototype (reference implementation)
├── prototype-rust/        # Rust implementation (current development)
└── README.md             # This file
```

## Quick Start

### Rust Version (Recommended)

```bash
cd prototype-rust
cargo build --release
cargo run --release --example test_pagerank_fix
```

### Swift Version (Reference)

```bash
cd prototype-swift
swift build
swift run
```

## Core Features

- ✅ **Incremental Algorithms**: Only update affected vertices (PageRank, BFS, SSSP)
- ✅ **GPU Acceleration**: Use Metal for computation (macOS)
- ✅ **EdgeBlock Data Structure**: Optimized cache utilization
- ✅ **CPU/GPU Collaboration**: Unified memory, efficient concurrency

## Documentation

- [Project Principles](core-research/algorithm-research/PROJECT_PRINCIPLES.md)
- [Incremental PageRank Status](core-research/experiment-reports/INCREMENTAL_PR_STATUS.md)
- [Performance Comparison](core-research/performance-tests/PERFORMANCE_COMPARISON.md)

## License

MIT
