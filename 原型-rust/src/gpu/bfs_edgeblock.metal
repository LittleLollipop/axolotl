// src/gpu/bfs_edgeblock.metal
// BFS 的 GPU 内核（使用 EdgeBlock 格式）
//
// 对应 Swift 版本的 `bfs_edgeblock` kernel

#include <metal_stdlib>
using namespace metal;

/// EdgeBlock 结构体（GPU 端）
struct EdgeBlock {
    uint ownerVertex;
    uint edgeCount;
    uint edges[32];  // BLOCK_CAPACITY = 32
};

/// GPU kernel: BFS using EdgeBlock format
kernel void bfs_edgeblock(
    device const uint *vertices [[buffer(0)]],
    device const uint *blockCounts [[buffer(1)]],
    device const EdgeBlock *blocks [[buffer(2)]],
    device const uint *frontier [[buffer(3)]],
    constant uint &frontierSize [[buffer(4)]],
    device uint *nextFrontier [[buffer(5)]],
    device uint *nextCount [[buffer(6)]],
    device uint *visited [[buffer(7)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= frontierSize) return;
    
    uint v = frontier[gid];
    uint blockStart = vertices[v];
    uint blockCount = blockCounts[v];
    
    for (uint i = 0; i < blockCount; i++) {
        EdgeBlock block = blocks[blockStart + i];
        for (uint j = 0; j < block.edgeCount; j++) {
            uint nb = block.edges[j];
            if (atomic_fetch_add_explicit((device atomic_uint*)&visited[nb], 1, memory_order_relaxed) == 0) {
                uint idx = atomic_fetch_add_explicit((device atomic_uint*)nextCount, 1, memory_order_relaxed);
                nextFrontier[idx] = nb;
            }
        }
    }
}
