// src/csr_graph.rs
// CSR (Compressed Sparse Row) 格式的图（用于 GPU 加速）
// 
// 对应 Swift 版本的 CSRGraph 结构体（第 26-38 行）

use std::collections::HashMap;

/// 使用 CSR 格式的图（使用 u32 索引，与 GPU 兼容）
/// 
/// 对应 Swift 版本的第 26-38 行
#[derive(Debug)]
pub struct CSRGraph {
    /// 顶点列表
    pub vertices: HashMap<u64, VertexData>,
    /// CSR 格式的偏移数组（vertex_id -> 第一条边在 edges 中的索引）
    /// 使用 u32 索引（与 GPU 兼容）
    /// 
    /// 对应 Swift 版本的 offsets 字段
    pub offsets: Vec<u32>,
    /// CSR 格式的边数组（所有顶点的边连续存储）
    /// 存储的是目标顶点的索引（u32）
    /// 
    /// 对应 Swift 版本的 targets 字段
    pub targets: Vec<u32>,
    /// 顶点 ID 到索引的映射
    pub vertex_to_idx: HashMap<u64, u32>,
    /// 索引到顶点 ID 的映射
    pub idx_to_vertex: Vec<u64>,
    /// 顶点数量
    /// 
    /// 对应 Swift 版本的 vertexCount 字段
    pub vertex_count: u32,
    /// 边的总数
    /// 
    /// 对应 Swift 版本的 totalEdges 字段
    pub total_edges: u32,
    /// 反向 CSR 的偏移数组（存储入边）
    /// reverse_offsets[v] 到 reverse_offsets[v+1] 是 v 的入边
    pub reverse_offsets: Vec<u32>,
    /// 反向 CSR 的边数组（存储入边的源顶点）
    pub reverse_targets: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct VertexData {
    pub id: u64,
    pub properties: HashMap<String, PropertyValue>,
}

/// 属性值（简化版）
#[derive(Debug, Clone)]
pub enum PropertyValue {
    Int(i64),
    String(String),
    Bool(bool),
}

impl CSRGraph {
    /// 创建新的图
    pub fn new() -> Self {
        CSRGraph {
            vertices: HashMap::new(),
            offsets: Vec::new(),
            targets: Vec::new(),
            reverse_offsets: Vec::new(),  // 初始化反向 CSR
            reverse_targets: Vec::new(),  // 初始化反向 CSR
            vertex_to_idx: HashMap::new(),
            idx_to_vertex: Vec::new(),
            vertex_count: 0,
            total_edges: 0,
        }
    }
    
    /// 添加顶点
    pub fn add_vertex(&mut self, id: u64, properties: HashMap<String, PropertyValue>) {
        let vertex_data = VertexData { id, properties };
        self.vertices.insert(id, vertex_data);
    }
    
    /// 构建 CSR 格式（必须在添加所有边之后调用）
    /// 
    /// 对应 Swift 版本的第 112-124 行
    pub fn build_csr(&mut self, edges: &[(u64, u64)]) {
        // 收集所有顶点 ID 并排序
        let mut vertex_ids: Vec<u64> = self.vertices.keys().cloned().collect();
        vertex_ids.sort();
        
        // 创建映射
        self.vertex_to_idx.clear();
        self.idx_to_vertex.clear();
        
        for (idx, &vertex_id) in vertex_ids.iter().enumerate() {
            self.vertex_to_idx.insert(vertex_id, idx as u32);
            self.idx_to_vertex.push(vertex_id);
        }
        
        let n = vertex_ids.len();
        
        // 统计每个顶点的边数
        let mut edge_counts = vec![0; n];
        for &(from, _) in edges {
            if let Some(&idx) = self.vertex_to_idx.get(&from) {
                edge_counts[idx as usize] += 1;
            }
        }
        
        // 构建偏移数组（Swift 第 113-122 行）
        self.offsets = vec![0; n + 1];
        for i in 0..n {
            self.offsets[i + 1] = self.offsets[i] + edge_counts[i];
        }
        
        // 填充边数组（Swift 第 114-121 行）
        let mut current_offset: Vec<u32> = self.offsets[0..n].to_vec();
        self.targets = vec![0; self.offsets[n] as usize];
        
        for &(from, to) in edges {
            if let Some(&idx) = self.vertex_to_idx.get(&from) {
                let offset = current_offset[idx as usize];
                
                // 转换成索引
                if let Some(&to_idx) = self.vertex_to_idx.get(&to) {
                    self.targets[offset as usize] = to_idx;
                    current_offset[idx as usize] += 1;
                }
            }
        }
        
        // 更新顶点数和边数
        self.vertex_count = n as u32;
        self.total_edges = edges.len() as u32;
        
        // 构建反向 CSR（存储入边）
        let vertex_to_idx_clone = self.vertex_to_idx.clone();  // 克隆一份，避免借用冲突
        self.build_reverse_csr(edges, &vertex_to_idx_clone);
    }
    
    /// 构建反向 CSR（存储入边）
    fn build_reverse_csr(&mut self, edges: &[(u64, u64)], vertex_to_idx: &HashMap<u64, u32>) {
        let n = self.vertex_count as usize;
        
        // 统计每个顶点的入边数
        let mut in_degrees = vec![0; n];
        for &(_, to) in edges {
            if let Some(&idx) = vertex_to_idx.get(&to) {
                in_degrees[idx as usize] += 1;
            }
        }
        
        // 构建反向偏移数组
        self.reverse_offsets = vec![0; n + 1];
        for i in 0..n {
            self.reverse_offsets[i + 1] = self.reverse_offsets[i] + in_degrees[i];
        }
        
        // 填充反向边数组
        let mut current_offset: Vec<u32> = self.reverse_offsets[0..n].to_vec();
        self.reverse_targets = vec![0; self.reverse_offsets[n] as usize];
        
        for &(from, to) in edges {
            if let Some(&to_idx) = vertex_to_idx.get(&to) {
                let offset = current_offset[to_idx as usize];
                
                // 转换成索引
                if let Some(&from_idx) = vertex_to_idx.get(&from) {
                    self.reverse_targets[offset as usize] = from_idx;
                    current_offset[to_idx as usize] += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_csr_graph() {
        let mut graph = CSRGraph::new();
        
        // 添加顶点
        graph.add_vertex(1, HashMap::new());
        graph.add_vertex(2, HashMap::new());
        graph.add_vertex(3, HashMap::new());
        
        // 添加边
        let edges = vec![
            (1, 2),
            (1, 3),
            (2, 3),
        ];
        
        graph.build_csr(&edges);
        
        // 验证 CSR 格式
        assert_eq!(graph.vertex_count, 3);
        assert_eq!(graph.total_edges, 3);
        
        // 顶点 1 的边：1->2, 1->3
        let start = graph.offsets[0] as usize;
        let end = graph.offsets[1] as usize;
        assert_eq!(end - start, 2);
        
        println!("✅ CSRGraph 测试通过！");
    }
}
