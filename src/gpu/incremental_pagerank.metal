// src/gpu/incremental_pagerank.metal
// 增量 PageRank 的 GPU 内核（来自 Swift 版本）

#include <metal_stdlib>
using namespace metal;

// GPU kernel: Update PageRank for affected vertices only
kernel void incremental_pagerank(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *offsets [[buffer(2)]],
    device const uint *targets [[buffer(3)]],
    device const float *pr [[buffer(4)]],
    device float *new_pr [[buffer(5)]],
    constant uint &vertex_count [[buffer(6)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];
    if (v >= vertex_count) return;
    
    float contribution = 0.0f;
    uint start = offsets[v];
    uint end = offsets[v + 1];
    
    for (uint i = start; i < end; i++) {
        uint neighbor = targets[i];
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
