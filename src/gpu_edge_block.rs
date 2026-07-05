// src/gpu_edge_block.rs
// GPU 专用的 EdgeBlock 格式（扁平化数组）
// 
// 对应 Swift 版本的 `EdgeBlockGraph` 结构体
// 用于 GPU 加速（BFS、PageRank 等）

/// GPU EdgeBlock 格式的图
/// 
/// 数据布局（扁平化数组，适配 GPU 内存访问）：
/// - `vertices[v]` = 顶点 v 的第一个 EdgeBlock 索引（出边）
/// - `block_counts[v]` = 顶点 v 的 EdgeBlock 数量（出边）
/// - `blocks` = 扁平化的 EdgeBlock 数据（出边）
///   - 每个 EdgeBlock 占用 `(2 + BLOCK_CAPACITY)` 个 u32
///   - `blocks[i]` = ownerVertex（源顶点）
///   - `blocks[i + 1]` = edgeCount（实际边数，≤ BLOCK_CAPACITY）
///   - `blocks[i + 2 .. i + 2 + BLOCK_CAPACITY]` = 目标顶点（不足填充 0）
/// 
/// 反向 EdgeBlock（入边）：
/// - `reverse_vertices[v]` = 顶点 v 的第一个反向 EdgeBlock 索引
/// - `reverse_block_counts[v]` = 顶点 v 的反向 EdgeBlock 数量
/// - `reverse_blocks` = 扁平化的反向 EdgeBlock 数据
#[derive(Debug)]
pub struct GPUEdgeBlockGraph {
    /// 每个顶点的第一个 EdgeBlock 索引（出边）
    pub vertices: Vec<u32>,
    
    /// 每个顶点的 EdgeBlock 数量（出边）
    pub block_counts: Vec<u32>,
    
    /// 扁平化的 EdgeBlock 数据（出边）
    pub blocks: Vec<u32>,
    
    /// 每个顶点的第一个反向 EdgeBlock 索引（入边）
    pub reverse_vertices: Vec<u32>,
    
    /// 每个顶点的反向 EdgeBlock 数量（入边）
    pub reverse_block_counts: Vec<u32>,
    
    /// 扁平化的反向 EdgeBlock 数据（入边）
    pub reverse_blocks: Vec<u32>,
    
    /// 顶点数量
    pub vertex_count: u32,
    
    /// 边的总数
    pub total_edges: u32,
    
    /// Block 容量（固定 = 32，= warp 大小）
    pub block_capacity: usize,
}

impl GPUEdgeBlockGraph {
    /// Block 容量（固定 = 32，= warp 大小）
    pub const BLOCK_CAPACITY: usize = 32;
    
    /// 每个 EdgeBlock 占用的 u32 数量（header + edges）
    pub const BLOCK_SIZE_U32: usize = 2 + Self::BLOCK_CAPACITY;
    
    /// 从 CSR 格式转换为 GPU EdgeBlock 格式
    /// 
    /// 对应 Swift 版本的 `csrToEdgeBlock()` 函数
    pub fn from_csr(
        csr_offsets: &[u32],
        csr_targets: &[u32],
        vertex_count: u32,
    ) -> Self {
        let vertex_count = vertex_count as usize;
        
        // ========== 构建正向 EdgeBlock（出边）==========
        let mut vertices = vec![0u32; vertex_count];
        let mut block_counts = vec![0u32; vertex_count];
        let mut blocks = Vec::new();
        
        for v in 0..vertex_count {
            let start = csr_offsets[v] as usize;
            let end = csr_offsets[v + 1] as usize;
            let degree = end - start;
            
            if degree == 0 {
                continue;
            }
            
            // 记录这个顶点的第一个 EdgeBlock 索引
            vertices[v] = (blocks.len() / Self::BLOCK_SIZE_U32) as u32;
            
            // 分成多个 Block
            let mut offset = start;
            while offset < end {
                let edges_in_this_block = usize::min(Self::BLOCK_CAPACITY, end - offset);
                
                // 添加 EdgeBlock header
                blocks.push(v as u32);  // ownerVertex
                blocks.push(edges_in_this_block as u32);  // edgeCount
                
                // 添加边
                for j in 0..edges_in_this_block {
                    blocks.push(csr_targets[offset + j]);
                }
                
                // 填充剩余空间（GPU 期望固定大小）
                for _ in edges_in_this_block..Self::BLOCK_CAPACITY {
                    blocks.push(0);
                }
                
                offset += edges_in_this_block;
                block_counts[v] += 1;
            }
        }
        
        // ========== 构建反向 EdgeBlock（入边）==========
        // 首先构建反向 CSR
        let mut reverse_offsets = vec![0u32; vertex_count + 1];
        let mut reverse_targets = Vec::new();
        
        // 计算每个顶点的入度
        let mut in_degrees = vec![0u32; vertex_count];
        for v in 0..vertex_count {
            let start = csr_offsets[v] as usize;
            let end = csr_offsets[v + 1] as usize;
            for i in start..end {
                let target = csr_targets[i] as usize;
                in_degrees[target] += 1;
            }
        }
        
        // 构建反向 offsets
        reverse_offsets[0] = 0;
        for v in 0..vertex_count {
            reverse_offsets[v + 1] = reverse_offsets[v] + in_degrees[v];
        }
        
        // 构建反向 targets
        reverse_targets.resize(reverse_offsets[vertex_count] as usize, 0u32);
        let mut current_offset = vec![0u32; vertex_count];
        
        for v in 0..vertex_count {
            let start = csr_offsets[v] as usize;
            let end = csr_offsets[v + 1] as usize;
            for i in start..end {
                let target = csr_targets[i] as usize;
                let pos = (reverse_offsets[target] + current_offset[target]) as usize;
                reverse_targets[pos] = v as u32;
                current_offset[target] += 1;
            }
        }
        
        // 构建反向 EdgeBlock
        let mut reverse_vertices = vec![0u32; vertex_count];
        let mut reverse_block_counts = vec![0u32; vertex_count];
        let mut reverse_blocks = Vec::new();
        
        for v in 0..vertex_count {
            let start = reverse_offsets[v] as usize;
            let end = reverse_offsets[v + 1] as usize;
            let degree = end - start;
            
            if degree == 0 {
                continue;
            }
            
            // 记录这个顶点的第一个反向 EdgeBlock 索引
            reverse_vertices[v] = (reverse_blocks.len() / Self::BLOCK_SIZE_U32) as u32;
            
            // 分成多个 Block
            let mut offset = start;
            while offset < end {
                let edges_in_this_block = usize::min(Self::BLOCK_CAPACITY, end - offset);
                
                // 添加反向 EdgeBlock header
                reverse_blocks.push(v as u32);  // ownerVertex（目标顶点）
                reverse_blocks.push(edges_in_this_block as u32);  // edgeCount
                
                // 添加边（源顶点）
                for j in 0..edges_in_this_block {
                    reverse_blocks.push(reverse_targets[offset + j]);
                }
                
                // 填充剩余空间
                for _ in edges_in_this_block..Self::BLOCK_CAPACITY {
                    reverse_blocks.push(0);
                }
                
                offset += edges_in_this_block;
                reverse_block_counts[v] += 1;
            }
        }
        
        // 计算总边数
        let mut total_edges = 0;
        for v in 0..vertex_count {
            total_edges += (csr_offsets[v + 1] - csr_offsets[v]) as u32;
        }
        
        GPUEdgeBlockGraph {
            vertices,
            block_counts,
            blocks,
            reverse_vertices,
            reverse_block_counts,
            reverse_blocks,
            vertex_count: vertex_count as u32,
            total_edges,
            block_capacity: Self::BLOCK_CAPACITY,
        }
    }
}
