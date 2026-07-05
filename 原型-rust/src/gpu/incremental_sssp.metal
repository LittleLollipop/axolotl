// src/gpu/incremental_sssp.metal
// 增量 SSSP（最短路径）的 GPU 内核

#include <metal_stdlib>
#include <metal_limits>
using namespace metal;

constant float INF = 1e30;

// GPU kernel: Relax edges for affected vertices only
kernel void incremental_sssp(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *offsets [[buffer(2)]],
    device const uint *targets [[buffer(3)]],
    device const float *weights [[buffer(4)]],
    device const float *distances [[buffer(5)]],
    device float *new_distances [[buffer(6)]],
    constant uint &vertex_count [[buffer(7)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];
    if (v >= vertex_count) return;
    if (distances[v] == INF) return;
    
    uint start = offsets[v];
    uint end = offsets[v + 1];
    float v_dist = distances[v];
    
    for (uint i = start; i < end; i++) {
        uint neighbor = targets[i];
        float weight = weights[i];
        float new_dist = v_dist + weight;
        if (new_dist < new_distances[neighbor]) {
            new_distances[neighbor] = new_dist;
        }
    }
}
