// src/gpu/incremental_bfs.metal
// 增量 BFS 的 GPU 内核

#include <metal_stdlib>
using namespace metal;

// GPU kernel: Update distances for affected vertices only
kernel void incremental_bfs(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *offsets [[buffer(2)]],
    device const uint *targets [[buffer(3)]],
    device const uint *distances [[buffer(4)]],
    device uint *new_distances [[buffer(5)]],
    constant uint &vertex_count [[buffer(6)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];
    if (v >= vertex_count) return;
    
    // 读取 v 的当前距离
    uint v_dist = distances[v];
    if (v_dist == uint(0xFFFFFFFF)) return;  // v 不可达（使用最大值）
    
    uint start = offsets[v];
    uint end = offsets[v + 1];
    uint new_dist = v_dist + 1;
    
    for (uint i = start; i < end; i++) {
        uint neighbor = targets[i];
        // 如果邻居的当前距离大于 new_dist，则更新
        if (new_distances[neighbor] > new_dist) {
            new_distances[neighbor] = new_dist;
        }
    }
}
