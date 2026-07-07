# Changelog

## v0.1.0-beta (2026-07-07)

### 🎉 Initial Beta Release

First public release of Axolotl-RS, the Rust implementation of the Axolotl high-performance graph database.

### ✨ Features

**Core Storage:**
- `GPUEdgeBlockGraph` — unified memory graph storage with fixed-size blocks (32 edges/block), optimized for GPU coalesced access
- `GraphDB` — unified interface supporting InMemory (EdgeBlock) and Mmap (read-only) modes
- `MmapGraph` — zero-copy memory-mapped graphs for datasets larger than RAM

**CRUD Operations:**
- Vertex CRUD: add, get, delete (with cascade edge cleanup)
- Edge CRUD: add, get, delete with weight and custom properties
- Edge properties storage (`EdgeData`: weight + HashMap of `PropertyValue`)
- Batch edge insertion

**Persistence:**
- AXEB binary format (4-section layout: topology + ID mapping + vertex props + edge props)
- Atomic writes (tmp → rename) for crash safety
- `from_file_or_new(path)` — auto-load with WAL recovery on startup
- `save_to(path)` / `save_to_file()` — explicit persistence
- AXOL format compatibility (legacy PersistentGraph)

**Crash Recovery:**
- WAL (Write-Ahead Log) with `Transaction`
- `WalRecovery` — automatic WAL replay on startup
- MVCC snapshot isolation (`MvccStore`)

**REST API Server:**
- 16 endpoints: CRUD, neighbors, algorithms, traversal, admin
- 16-thread pool with non-blocking accept
- Graceful shutdown (`POST /admin/shutdown`) with auto-save
- Manual save endpoint (`POST /admin/save`)
- Zero external dependencies (std-only HTTP parser)

**Algorithms:**
- Correct PageRank (dangling node handling, PR sum = 1.0) — CPU + GPU (Metal)
- Incremental PageRank — CPU + GPU (only update affected vertices)
- Incremental BFS — CPU + GPU
- Incremental SSSP (Single Source Shortest Path) — CPU + GPU
- Incremental Connected Components (Union-Find)
- Incremental Triangle Counting

**Graph Traversal:**
- `walk(start, max_depth, visitor)` — multi-hop BFS with callback
- `subgraph(seeds, max_depth)` — subgraph extraction with edge properties
- `find_paths(vertex_filter, edge_filter, path_length)` — DFS path pattern matching

**Python Bindings (PyO3):**
- `axolotl_rs.AxolotlGraph` — full Python API
- CRUD, PageRank, BFS, traversal, persistence
- `maturin build` → pip-installable wheel
- Native performance (no serialization overhead)

**Performance:**
- BFS incremental speedup: **283x–1580x** (vs full recomputation)
- Connected Components incremental speedup: **720x–4920x**
- GPU buffer cache reuse: **21–25%** savings across PageRank/BFS/SSSP
- 10K vertices, 50K edges: insert 289K edges/s, PageRank 50 iters in 1.2s

**Concurrency:**
- Algorithm lock optimization: CSR copy inside lock, computation outside
- Mutex-protected GraphDB for Python bindings

### 📊 Verification

- **77 unit tests** pass (all lib tests)
- PR sum correctness: **1.0000** (validated)
- REST API end-to-end tested (persistence round-trip)
- Python bindings end-to-end tested

### 📁 File Format

- `.axeb` — AXolotl EdgeBlock binary format (native)
- `.axol` — AXolotl Legacy format (compatibility)
- `.wal` — Write-Ahead Log entries

### ⚠️ Known Limitations

- Mmap mode is read-only (write operations return `NotSupported`)
- Index API is stubbed (not yet implemented with EdgeBlock)
- Incremental PageRank GPU path needs optimization (18.9x vs 244x target)
- 11 dead_code warnings (ported Swift code, cosmetic)
- No CI/CD pipeline yet

### 🔮 Next Steps

- Python package published to PyPI
- Incremental PageRank GPU kernel optimization
- CI/CD with GitHub Actions
- 10M+ vertex stress testing
- Property-based testing (proptest)
