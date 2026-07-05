// src/gpu/pagerank_edgeblock.metal
// PageRank 的 EdgeBlock 版本 GPU 内核
//
// 对应 Swift 版本的 PageRank GPU 内核

#include <metal_stdlib>
using namespace metal;

/// 增量 PageRank（EdgeBlock 版本）
///
/// 参数：
/// - affected_vertices: 受影响顶点列表（索引）
/// - affected_count: 受影响顶点数量
/// - reverse_vertices: 每个顶点的第一个反向 EdgeBlock 索引（入边）
/// - reverse_block_counts: 每个顶点的反向 EdgeBlock 数量
/// - reverse_blocks: 扁平化的反向 EdgeBlock 数据
/// - pr: 当前 PR 值
/// - new_pr: 输出：新的 PR 值
/// - damping_factor: 阻尼因子（通常 = 0.85）
/// - vertex_count: 顶点数量
kernel void pagerank_edgeblock(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *reverse_vertices [[buffer(2)]],
    device const uint *reverse_block_counts [[buffer(3)]],
    device const uint (*reverse_blocks)[34] [[buffer(4)]],
    device const float *pr [[buffer(5)]],
    device float *new_pr [[buffer(6)]],
    constant float &damping_factor [[buffer(7)]],
    constant uint &vertex_count [[buffer(8)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];  // 当前受影响的顶点
    if (v >= vertex_count) return;
    
    // 遍历入边（反向 EdgeBlock）
    float contribution = 0.0;
    uint start = reverse_vertices[v];
    uint count = reverse_block_counts[v];
    
    for (uint b = 0; b < count; b++) {
        uint block_idx = start + b;
        uint edge_count = reverse_blocks[block_idx][1];  // edgeCount
        
        for (uint i = 0; i < edge_count; i++) {
            uint source = reverse_blocks[block_idx][i + 2];  // 源顶点
            float source_pr = pr[source];
            
            // 找出源顶点的出度
            // 注意：这里需要源顶点的出度，但 EdgeBlock 中没有存储
            // 临时方案：假设每个顶点的出度是已知的（需要从 CPU 传入）
            // 或者：遍历源顶点的出边，计算贡献
            
            // 简化版本：假设出度 = 1（不正确，需要调整）
            // TODO: 需要传入出度数组
        }
    }
    
    // 应用阻尼因子
    float base_score = (1.0 - damping_factor) / float(vertex_count);
    new_pr[v] = base_score + damping_factor * contribution;
}

/// 增量 PageRank（EdgeBlock 版本，优化）
///
/// 改进：传入出度数组
kernel void pagerank_edgeblock_optimized(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *reverse_vertices [[buffer(2)]],
    device const uint *reverse_block_counts [[buffer(3)]],
    device const uint (*reverse_blocks)[34] [[buffer(4)]],
    device const float *pr [[buffer(5)]],
    device float *new_pr [[buffer(6)]],
    device const uint *out_degrees [[buffer(7)]],
    constant float &damping_factor [[buffer(8)]],
    constant uint &vertex_count [[buffer(9)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];  // 当前受影响的顶点
    if (v >= vertex_count) return;
    
    // 遍历入边（反向 EdgeBlock）
    float contribution = 0.0;
    uint start = reverse_vertices[v];
    uint count = reverse_block_counts[v];
    
    for (uint b = 0; b < count; b++) {
        uint block_idx = start + b;
        uint edge_count = reverse_blocks[block_idx][1];  // edgeCount
        
        for (uint i = 0; i < edge_count; i++) {
            uint source = reverse_blocks[block_idx][i + 2];  // 源顶点
            float source_pr = pr[source];
            uint source_out_degree = out_degrees[source];
            
            if (source_out_degree > 0) {
                contribution += source_pr / float(source_out_degree);
            }
        }
    }
    
    // 应用阻尼因子
    float base_score = (1.0 - damping_factor) / float(vertex_count);
    new_pr[v] = base_score + damping_factor * contribution;
}
