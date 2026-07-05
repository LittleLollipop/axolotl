// src/gpu/pagerank_full.metal
// GPU 全量 PageRank（正确处理悬挂顶点）
// 参考：pagerank_edgeblock.metal 的 pagerank_edgeblock_optimized 内核

#include <metal_stdlib>
using namespace metal;

/// GPU 全量 PageRank（正确处理悬挂顶点）
///
/// 参数：
/// - reverse_offsets: 反向 CSR 偏移数组（入边）
/// - reverse_targets: 反向 CSR 目标数组（入边源顶点）
/// - out_degrees: 每个顶点的出度
/// - pr: 当前 PR 值数组（输入）
/// - new_pr: 新的 PR 值数组（输出）
/// - damping_factor: 阻尼因子（通常 0.85）
/// - vertex_count: 顶点数量
/// - dangling_contribution: 悬挂顶点的贡献（均匀分布）
kernel void pagerank_full(
    device const uint *reverse_offsets [[buffer(0)]],
    device const uint *reverse_targets [[buffer(1)]],
    device const uint *out_degrees [[buffer(2)]],
    device const float *pr [[buffer(3)]],
    device float *new_pr [[buffer(4)]],
    constant float &damping_factor [[buffer(5)]],
    constant uint &vertex_count [[buffer(6)]],
    device const float *dangling_contribution [[buffer(7)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= vertex_count) return;
    
    // 计算 base_score
    float base_score = (1.0 - damping_factor) / float(vertex_count);
    
    // 计算贡献（遍历入边）—— Kahan 求和，提升 f32 精度
    float contribution = 0.0;
    float c = 0.0;  // Kahan 补偿项
    uint start = reverse_offsets[gid];
    uint end = reverse_offsets[gid + 1];
    
    for (uint i = start; i < end; i++) {
        uint source = reverse_targets[i];
        uint source_out_degree = out_degrees[source];
        
        if (source_out_degree > 0) {
            float input = pr[source] / float(source_out_degree);
            float y = input - c;
            float t = contribution + y;
            c = (t - contribution) - y;
            contribution = t;
        }
    }
    
    // 应用阻尼因子，加上悬挂顶点的贡献
    new_pr[gid] = base_score + damping_factor * (contribution + dangling_contribution[0]);
}
