// src/gpu/sssp_edgeblock.metal
// SSSP 的 EdgeBlock 版本 GPU 内核（简化版：无权图）
//
// 假设所有边的权重都是 1.0

#include <metal_stdlib>
using namespace metal;

/// 增量 SSSP（EdgeBlock 版本，简化版：无权图）
///
/// 参数：
/// - affected_vertices: 受影响顶点列表（索引）
/// - affected_count: 受影响顶点数量
/// - vertices: 每个顶点的第一个 EdgeBlock 索引（出边）
/// - block_counts: 每个顶点的 EdgeBlock 数量
/// - blocks: 扁平化的 EdgeBlock 数据
/// - distances: 当前距离数组
/// - new_distances: 输出：新的距离数组
/// - vertex_count: 顶点数量
kernel void sssp_edgeblock_unweighted(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *vertices [[buffer(2)]],
    device const uint *block_counts [[buffer(3)]],
    device const uint (*blocks)[34] [[buffer(4)]],
    device const uint *distances [[buffer(5)]],
    device uint *new_distances [[buffer(6)]],
    constant uint &vertex_count [[buffer(7)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];  // 当前受影响的顶点
    if (v >= vertex_count) return;
    
    uint v_dist = distances[v];
    if (v_dist == 0xFFFFFFFF) return;  // 不可达（使用 UINT32_MAX 表示无穷大）
    
    // 遍历出边（正向 EdgeBlock）
    uint start = vertices[v];
    uint count = block_counts[v];
    
    for (uint b = 0; b < count; b++) {
        uint block_idx = start + b;
        uint edge_count = blocks[block_idx][1];  // edgeCount
        
        for (uint i = 0; i < edge_count; i++) {
            uint neighbor = blocks[block_idx][i + 2];  // 目标顶点
            
            // 无权图：权重 = 1
            uint new_dist = v_dist + 1;
            if (new_distances[neighbor] > new_dist) {
                new_distances[neighbor] = new_dist;
            }
        }
    }
}
