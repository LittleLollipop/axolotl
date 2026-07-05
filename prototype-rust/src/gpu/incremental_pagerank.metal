// src/gpu/incremental_pagerank.metal
// 增量 PageRank 的 GPU 内核（使用反向 CSR）

#include <metal_stdlib>
using namespace metal;

// GPU kernel: Update PageRank for affected vertices only
kernel void incremental_pagerank(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *reverse_offsets [[buffer(2)]],  // 反向 CSR 的偏移数组
    device const uint *reverse_targets [[buffer(3)]],  // 反向 CSR 的边数组
    device const uint *offsets [[buffer(4)]],  // 正向 CSR 的偏移数组（用于获取出度）
    device const float *pr [[buffer(5)]],
    device float *new_pr [[buffer(6)]],
    constant uint &vertex_count [[buffer(7)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];
    if (v >= vertex_count) return;
    
    // 使用反向 CSR 找出 v 的入边邻居
    float contribution = 0.0f;
    uint start = reverse_offsets[v];
    uint end = reverse_offsets[v + 1];
    
    for (uint i = start; i < end; i++) {
        uint neighbor = reverse_targets[i];  // 有边从 neighbor 指向 v
        
        // 使用正向 CSR 获取 neighbor 的出度
        uint neighbor_start = offsets[neighbor];
        uint neighbor_end = offsets[neighbor + 1];
        uint out_degree = neighbor_end - neighbor_start;
        
        if (out_degree > 0) {
            contribution += pr[neighbor] / float(out_degree);
        }
    }
    
    float damping = 0.85f;
    float new_value = (1.0f - damping) / float(vertex_count) + damping * contribution;
    
    // Update this vertex's score
    new_pr[v] = new_value;
}
