// src/pagerank_correct.rs
// 正确的 PageRank 实现（CPU 版本，用于验证）

use crate::csr_graph::CSRGraph;

pub fn compute_pagerank_cpu(csr: &CSRGraph, iterations: usize) -> Vec<f32> {
    let vertex_count = csr.vertex_count as usize;
    let damping = 0.85f32;
    
    // 初始化：均匀分布
    let mut pr = vec![1.0 / vertex_count as f32; vertex_count];
    
    // 预计算出度
    let mut out_degrees = vec![0u32; vertex_count];
    for v in 0..vertex_count {
        let start = csr.offsets[v] as usize;
        let end = csr.offsets[v + 1] as usize;
        out_degrees[v] = (end - start) as u32;
    }
    
    // 迭代
    for _ in 0..iterations {
        let mut new_pr = vec![0.0; vertex_count];
        
        // 计算每个顶点的贡献
        for v in 0..vertex_count {
            let start = csr.offsets[v] as usize;
            let end = csr.offsets[v + 1] as usize;
            
            // v 指向这些顶点，所以 v 的 PR 值要分割给出边邻居
            if out_degrees[v] > 0 {
                let contribution = pr[v] / out_degrees[v] as f32;
                for i in start..end {
                    let neighbor = csr.targets[i] as usize;
                    new_pr[neighbor] += contribution;
                }
            }
        }
        
        // 应用阻尼因子
        let base_score = (1.0 - damping) / vertex_count as f32;
        for v in 0..vertex_count {
            new_pr[v] = base_score + damping * new_pr[v];
        }
        
        pr = new_pr;
    }
    
    // 验证：PR 值之和应该 = 1.0
    let pr_sum: f32 = pr.iter().sum();
    println!("PR 值之和：{:.6} (应该是 1.0)", pr_sum);
    
    pr
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csr_graph::CSRGraph;
    
    #[test]
    fn test_pagerank_correct() {
        // 创建一个简单的图
        let mut csr = CSRGraph::new();
        
        for i in 0..5 {
            let mut props = std::collections::HashMap::new();
            props.insert("id".to_string(), crate::PropertyValue::Int(i as i64));
            csr.add_vertex(i, props);
        }
        
        let edges = vec![(0, 1), (0, 2), (1, 2), (2, 0), (3, 4)];
        csr.build_csr(&edges);
        
        // 计算 PageRank
        let pr = compute_pagerank_cpu(&csr, 100);
        
        // 验证：PR 值之和应该 = 1.0
        let pr_sum: f32 = pr.iter().sum();
        assert!((pr_sum - 1.0).abs() < 1e-4, "PR 值之和应该是 1.0，实际是 {}", pr_sum);
        
        println!("✅ 正确的 PageRank 实现测试通过！");
        println!("  PR 值：{:?}", pr);
        println!("  PR 值之和：{:.6}", pr_sum);
    }
}
