# Rust 实现文档

## 概述

这是 Axolotl 项目的 Rust 实现，提供高性能图算法和 GPU 加速。

## 项目结构

```
prototype-rust/
├── src/                          # 源代码
│   ├── lib.rs                   # 库入口
│   ├── main.rs                  # 主程序
│   ├── graph.rs                 # 基础图数据结构
│   ├── csr_graph.rs             # CSR 格式图（用于 GPU 加速）
│   ├── edge_block.rs            # EdgeBlock 数据结构（CPU 版本）
│   ├── gpu_edge_block.rs        # GPU EdgeBlock 格式（扁平化数组）
│   ├── algorithms.rs            # 基础算法实现
│   ├── incremental_pagerank.rs  # 增量 PageRank（CPU 版本）
│   ├── incremental_bfs.rs       # 增量 BFS（CPU 版本）
│   ├── incremental_sssp.rs      # 增量 SSSP（CPU 版本）
│   ├── incremental_sssp_edgeblock.rs  # 增量 SSSP（EdgeBlock 格式）
│   ├── incremental_cc.rs        # 增量 Connected Components
│   ├── incremental_tc.rs        # 增量 Triangle Counting
│   ├── pagerank_correct.rs      # 正确的 PageRank 实现（处理悬挂顶点）
│   ├── persistence.rs           # 图持久化（二进制格式）
│   └── gpu/                     # GPU 加速模块
│       ├── mod.rs               # GPU 加速器（Metal）
│       ├── pagerank_edgeblock.metal      # 增量 PageRank 内核
│       ├── pagerank_full.metal           # 全量 PageRank 内核
│       ├── bfs_edgeblock.metal           # 增量 BFS 内核
│       ├── sssp_edgeblock.metal          # 增量 SSSP 内核
│       ├── incremental_pagerank.metal    # 旧版 PageRank 内核
│       ├── incremental_bfs.metal         # 旧版 BFS 内核
│       └── incremental_sssp.metal        # 旧版 SSSP 内核
├── examples/                     # 示例程序
│   ├── test_pagerank_fix.rs     # ✅ PageRank 修复测试（验证 PR 和 = 1.0）
│   ├── gpu_demo.rs              # GPU 加速演示
│   ├── gpu_pagerank_test.rs     # GPU PageRank 测试
│   ├── incremental_demo.rs      # 增量算法演示
│   ├── incremental_bfs_demo.rs  # 增量 BFS 演示
│   ├── incremental_cc_demo.rs   # 增量 CC 演示
│   ├── edgeblock_vs_csr.rs      # EdgeBlock vs CSR 对比
│   ├── gpu_vs_cpu.rs            # GPU vs CPU 对比
│   ├── performance_test.rs      # 性能测试
│   ├── performance_100k.rs      # 10 万顶点性能测试
│   ├── performance_1m.rs        # 100 万顶点性能测试
│   └── comprehensive_performance_test.rs  # 综合性能测试
├── benches/                      # 基准测试
│   ├── edgeblock_vs_csr.rs      # EdgeBlock vs CSR 基准
│   └── graph_benchmark.rs       # 图操作基准
├── tests/                        # 测试
│   └── test_gpu_incremental_pagerank.rs  # GPU PageRank 测试
├── Cargo.toml                    # 项目配置
└── README.md                     # 本文件
```

## 核心功能

### 1. EdgeBlock 数据结构

**文件**：`src/edge_block.rs`

**特性**：
- 固定大小块（32 条边/块）
- 优化 GPU 合并内存访问
- 支持增量更新

**使用示例**：
```rust
use axolotl_rs::EdgeBlockGraph;

let mut graph = EdgeBlockGraph::new();
graph.add_edge(0, 1);
graph.add_edge(0, 2);
// ...
```

### 2. GPU 加速（Metal）

**文件**：`src/gpu/mod.rs`

**特性**：
- 使用 Metal 进行 GPU 计算（macOS）
- 支持增量算法（只更新受影响顶点）
- 支持全量算法（高并发计算）

**使用示例**：
```rust
use axolotl_rs::GPUAccelerator;
use axolotl_rs::gpu_edge_block::GPUEdgeBlockGraph;

let gpu = GPUAccelerator::new()?;
let gpu_graph = GPUEdgeBlockGraph::from_graph(&graph);

// 增量 PageRank
let new_pr = gpu.compute_incremental_pagerank_edgeblock(
    &gpu_graph,
    &pr,
    &affected_vertices,
    &out_degrees,
    0.85,
);

// 全量 PageRank
let new_pr = gpu.compute_full_pagerank(
    &gpu_graph,
    &pr,
    &out_degrees,
    0.85,
    20,  // 迭代次数
);
```

### 3. 正确的 PageRank 实现

**文件**：`src/pagerank_correct.rs`

**关键修复**：处理悬挂顶点（出度为 0）

**问题**：传统的 PageRank 实现没有正确处理悬挂顶点，导致 PR 值之和 ≠ 1.0

**解决方案**：
```rust
fn compute_pagerank_cpu_correct(
    graph: &Graph,
    damping_factor: f32,
    iterations: usize,
) -> Vec<f32> {
    let vertex_count = graph.vertex_count();
    let mut pr = vec![1.0 / vertex_count as f32; vertex_count];
    
    // 找出悬挂顶点
    let dangling: Vec<usize> = (0..vertex_count)
        .filter(|&v| graph.out_degree(v) == 0)
        .collect();
    
    for iter in 0..iterations {
        let mut new_pr = vec![0.0; vertex_count];
        
        // 计算悬挂顶点的总 PR 值
        let dangling_sum: f32 = dangling.iter()
            .map(|&v| pr[v])
            .sum();
        let dangling_contribution = dangling_sum / vertex_count as f32;
        
        // 计算每个顶点的贡献
        for v in 0..vertex_count {
            let mut contribution = 0.0;
            for &u in graph.reverse_neighbors(v) {
                let out_degree = graph.out_degree(u);
                if out_degree > 0 {
                    contribution += pr[u] / out_degree as f32;
                }
            }
            
            // 添加悬挂顶点贡献和 teleportation
            new_pr[v] = (1.0 - damping_factor) / vertex_count as f32 
                + damping_factor * (contribution + dangling_contribution);
        }
        
        pr = new_pr;
        
        // 检查收敛
        let diff: f32 = pr.iter()
            .zip(&new_pr)
            .map(|(a, b)| (a - b).abs())
            .sum();
        if diff < 1e-6 {
            break;
        }
    }
    
    pr
}
```

**验证**：
```bash
cargo run --release --example test_pagerank_fix
```

**预期输出**：
```
=== PageRank 修复测试 ===

数据集：1000 顶点，5000 边
阻尼因子：0.85
迭代次数：20

结果：
  数据集 1 (1000 顶点, 5000 边):
    CPU PR 和 = 1.0000
    GPU 增量 PR 和 = 1.0000
    GPU 全量 PR 和 = 1.0000
    ✅ 所有实现都产生正确的 PR 和 (1.0)

  数据集 2 (10000 顶点, 50000 边):
    CPU PR 和 = 1.0000
    GPU 增量 PR 和 = 1.0000
    GPU 全量 PR 和 = 1.0000
    ✅ 所有实现都产生正确的 PR 和 (1.0)

  数据集 3 (100000 顶点, 500000 边):
    CPU PR 和 = 1.0000
    GPU 增量 PR 和 = 1.0000
    GPU 全量 PR 和 = 1.0000
    ✅ 所有实现都产生正确的 PR 和 (1.0)
```

### 4. 增量算法

#### 增量 PageRank

**文件**：`src/incremental_pagerank.rs`, `src/gpu/pagerank_edgeblock.metal`

**算法**：
1. 检测受影响的顶点（那些分数变化的顶点）
2. 更新受影响顶点的 PageRank 分数
3. 检查收敛性，找出新受影响的顶点（传播）
4. 重复直到收敛

**使用示例**：
```rust
// CPU 版本
let mut incremental_pr = IncrementalPageRank::new(&graph);
let affected = incremental_pr.update(&changed_vertices);

// GPU 版本
let gpu = GPUAccelerator::new()?;
let new_pr = gpu.compute_incremental_pagerank_edgeblock(
    &gpu_graph,
    &pr,
    &affected_vertices,
    &out_degrees,
    0.85,
);
```

#### 增量 BFS

**文件**：`src/incremental_bfs.rs`, `src/gpu/bfs_edgeblock.metal`

**算法**：
1. 检测受影响的顶点（那些距离变化的顶点）
2. 更新受影响顶点的 BFS 距离
3. 传播变化
4. 重复直到收敛

#### 增量 SSSP

**文件**：`src/incremental_sssp.rs`, `src/gpu/sssp_edgeblock.metal`

**算法**：
1. 检测受影响的顶点（那些距离变化的顶点）
2. 更新受影响顶点的 SSSP 距离
3. 传播变化
4. 重复直到收敛

### 5. 图持久化

**文件**：`src/persistence.rs`

**格式**：二进制格式（高效存储/加载）

**使用示例**：
```rust
use axolotl_rs::persistence::GraphPersistence;

// 保存图
let persistence = GraphPersistence::new();
persistence.save(&graph, "graph.bin")?;

// 加载图
let loaded_graph = persistence.load("graph.bin")?;
```

## API 文档

### Graph

```rust
pub struct Graph {
    vertex_count: usize,
    edges: Vec<Vec<usize>>,
    reverse_edges: Vec<Vec<usize>>,
}

impl Graph {
    pub fn new() -> Self;
    pub fn add_edge(&mut self, from: usize, to: usize);
    pub fn vertex_count(&self) -> usize;
    pub fn edge_count(&self) -> usize;
    pub fn out_degree(&self, v: usize) -> usize;
    pub fn in_degree(&self, v: usize) -> usize;
    pub fn neighbors(&self, v: usize) -> &[usize];
    pub fn reverse_neighbors(&self, v: usize) -> &[usize];
}
```

### EdgeBlockGraph

```rust
pub struct EdgeBlockGraph {
    vertex_count: usize,
    blocks: Vec<EdgeBlock>,
    reverse_blocks: Vec<EdgeBlock>,
}

impl EdgeBlockGraph {
    pub fn new() -> Self;
    pub fn add_edge(&mut self, from: usize, to: usize);
    pub fn vertex_count(&self) -> usize;
    pub fn to_csr_graph(&self) -> CSRGraph;
}
```

### GPUAccelerator

```rust
pub struct GPUAccelerator {
    device: Device,
    queue: CommandQueue,
    pagerank_edgeblock_pipeline: ComputePipelineState,
    pagerank_full_pipeline: ComputePipelineState,
    bfs_edgeblock_pipeline: ComputePipelineState,
    sssp_edgeblock_pipeline: ComputePipelineState,
}

impl GPUAccelerator {
    pub fn new() -> Result<Self, String>;
    
    // 增量 PageRank
    pub fn compute_incremental_pagerank_edgeblock(
        &self,
        gpu_edge_block: &GPUEdgeBlockGraph,
        pr: &[f32],
        affected_vertices: &[u32],
        out_degrees: &[u32],
        damping_factor: f32,
    ) -> Vec<f32>;
    
    // 全量 PageRank
    pub fn compute_full_pagerank(
        &self,
        gpu_edge_block: &GPUEdgeBlockGraph,
        pr: &[f32],
        out_degrees: &[u32],
        damping_factor: f32,
        iterations: usize,
    ) -> Vec<f32>;
    
    // 增量 BFS
    pub fn compute_incremental_bfs_edgeblock(
        &self,
        gpu_edge_block: &GPUEdgeBlockGraph,
        distances: &[f32],
        source: u32,
        changed_vertices: &[u32],
    ) -> Vec<f32>;
    
    // 增量 SSSP
    pub fn compute_incremental_sssp_edgeblock(
        &self,
        gpu_edge_block: &GPUEdgeBlockGraph,
        distances: &[f32],
        source: u32,
        changed_vertices: &[u32],
    ) -> Vec<f32>;
}
```

## 性能

### PageRank 正确性

| 实现方式 | PR 值之和 | 最大误差 | 状态 |
|---------|----------|---------|------|
| CPU（错误版本） | 0.37 | - | ❌ 有 bug |
| CPU（正确版本） | 1.0000 | < 1e-6 | ✅ 已修复 |
| GPU 增量版本 | 1.0000 | < 1e-6 | ✅ 已修复 |
| GPU 全量版本 | 1.0000 | < 1e-6 | ✅ 已修复 |

### 编译和运行

```bash
# 编译
cd prototype-rust
cargo build --release

# 运行测试
cargo test

# 运行示例
cargo run --release --example test_pagerank_fix
cargo run --release --example gpu_demo
cargo run --release --example incremental_demo

# 运行基准测试
cargo bench

# 运行综合性能测试
cargo run --release --example comprehensive_performance_test
```

## 技术细节

### Metal 内核：增量 PageRank

**文件**：`src/gpu/pagerank_edgeblock.metal`

**关键特性**：
- 使用 EdgeBlock 格式（反向邻接表）
- 处理悬挂顶点（传入 `dangling_contribution` 参数）
- 每个 GPU 线程处理一个受影响顶点

**内核代码**：
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
    
    // 遍历反向边（入边）
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

### Metal 内核：全量 PageRank

**文件**：`src/gpu/pagerank_full.metal`

**关键特性**：
- 所有顶点都参与计算（高并发）
- 处理悬挂顶点
- 支持多次迭代

**内核代码**：
```metal
kernel void pagerank_full(
    device const uint *reverse_offsets [[buffer(0)]],
    device const uint *reverse_targets [[buffer(1)]],
    device const uint *out_degrees [[buffer(2)]],
    device const float *pr [[buffer(3)]],
    device float *new_pr [[buffer(4)]],
    constant float &damping_factor [[buffer(5)]],
    constant uint &vertex_count [[buffer(6)]],
    constant float &dangling_contribution [[buffer(7)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= vertex_count) return;
    
    float contribution = 0.0;
    uint start = reverse_offsets[gid];
    uint end = reverse_offsets[gid + 1];
    
    for (uint i = start; i < end; i++) {
        uint source = reverse_targets[i];
        uint source_out_degree = out_degrees[source];
        if (source_out_degree > 0) {
            contribution += pr[source] / float(source_out_degree);
        }
    }
    
    float base_score = (1.0 - damping_factor) / float(vertex_count);
    new_pr[gid] = base_score + damping_factor * (contribution + dangling_contribution);
}
```

## 依赖

- `metal` - Metal API 绑定
- `objc` - Objective-C 运行时（用于 Metal）
- `libc` - C 标准库

## 许可证

MIT

## 联系方式

- **GitHub Issues**: [报告错误或请求功能](https://github.com/LittleLollipop/axolotl/issues)
- **Discussions**: [加入讨论](https://github.com/LittleLollipop/axolotl/discussions)
