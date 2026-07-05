// src/incremental_cc.rs
// 增量 Connected Components（使用 Union-Find）
// 
// 翻译自：/tmp/axolotl_tmp/Experiments/incremental_connected_components.swift

/// Union-Find（并查集）数据结构
/// 
/// 使用路径压缩和按秩合并，每个操作平均 O(α(V)) ≈ 常数时间
pub struct UnionFind {
    parent: Vec<u32>,
    rank: Vec<u32>,
}

impl UnionFind {
    /// 创建新的 Union-Find（每个顶点独立成一个组件）
    pub fn new(vertex_count: usize) -> Self {
        let mut parent = Vec::with_capacity(vertex_count);
        let rank = vec![0; vertex_count];
        
        for i in 0..vertex_count {
            parent.push(i as u32);
        }
        
        UnionFind { parent, rank }
    }
    
    /// 查找根节点（带路径压缩）
    pub fn find(&mut self, x: u32) -> u32 {
        if self.parent[x as usize] != x {
            // 路径压缩
            self.parent[x as usize] = self.find(self.parent[x as usize]);
        }
        self.parent[x as usize]
    }
    
    /// 合并两个集合（按秩合并）
    /// 
    /// 返回：如果合并成功（两个顶点之前不在同一集合），返回 true
    pub fn union(&mut self, x: u32, y: u32) -> bool {
        let root_x = self.find(x);
        let root_y = self.find(y);
        
        if root_x == root_y {
            return false;  // 已经在同一个组件中
        }
        
        // 按秩合并
        if self.rank[root_x as usize] < self.rank[root_y as usize] {
            self.parent[root_x as usize] = root_y;
        } else if self.rank[root_x as usize] > self.rank[root_y as usize] {
            self.parent[root_y as usize] = root_x;
        } else {
            self.parent[root_y as usize] = root_x;
            self.rank[root_x as usize] += 1;
        }
        
        true  // 成功合并
    }
    
    /// 复制 Union-Find（用于增量更新）
    pub fn copy(&self) -> Self {
        UnionFind {
            parent: self.parent.clone(),
            rank: self.rank.clone(),
        }
    }
    
    /// 计算组件数量
    pub fn count_components(&mut self) -> usize {
        let mut roots = std::collections::HashSet::new();
        for v in 0..self.parent.len() {
            roots.insert(self.find(v as u32));
        }
        roots.len()
    }
}

/// 增量 Connected Components
pub struct IncrementalCC {
    /// Union-Find 数据结构
    uf: UnionFind,
}

impl IncrementalCC {
    /// 创建新的增量 Connected Components
    /// 
    /// 参数：vertex_count - 顶点数量
    pub fn new(vertex_count: usize) -> Self {
        let uf = UnionFind::new(vertex_count);
        
        IncrementalCC { uf }
    }
    
    /// 全量计算 Connected Components（处理所有边）
    /// 
    /// 参数：edges - 所有边（无向边）
    pub fn compute_full(&mut self, edges: &[(u64, u64)]) {
        for &(u, v) in edges {
            let u_idx = u as u32;
            let v_idx = v as u32;
            if u_idx < self.uf.parent.len() as u32 && v_idx < self.uf.parent.len() as u32 {
                self.uf.union(u_idx, v_idx);
            }
        }
    }
    
    /// 增量更新 Connected Components（只处理新增的边）
    /// 
    /// 参数：new_edges - 新增的边（无向边）
    /// 
    /// 返回：合并的组件数量
    pub fn update_incremental(&mut self, new_edges: &[(u64, u64)]) -> usize {
        let mut merged_count = 0;
        
        for &(u, v) in new_edges {
            let u_idx = u as u32;
            let v_idx = v as u32;
            if u_idx < self.uf.parent.len() as u32 && v_idx < self.uf.parent.len() as u32 {
                if self.uf.union(u_idx, v_idx) {
                    merged_count += 1;
                }
            }
        }
        
        merged_count
    }
    
    /// 获取组件数量
    pub fn count_components(&mut self) -> usize {
        self.uf.count_components()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_union_find() {
        // 创建 Union-Find（5 个顶点）
        let mut uf = UnionFind::new(5);
        
        // 初始状态：每个顶点独立成一个组件
        assert_eq!(uf.count_components(), 5);
        
        // 合并 0 和 1
        assert!(uf.union(0, 1));
        assert_eq!(uf.count_components(), 4);
        
        // 合并 1 和 2
        assert!(uf.union(1, 2));
        assert_eq!(uf.count_components(), 3);
        
        // 合并 3 和 4
        assert!(uf.union(3, 4));
        assert_eq!(uf.count_components(), 2);
        
        // 尝试合并 0 和 2（已经在同一个组件中）
        assert!(!uf.union(0, 2));
        assert_eq!(uf.count_components(), 2);
        
        println!("✅ Union-Find 测试通过！");
    }
    
    #[test]
    fn test_incremental_cc() {
        // 创建增量 Connected Components（5 个顶点）
        let mut cc = IncrementalCC::new(5);
        
        // 全量计算（添加边：0-1, 1-2, 3-4）
        let edges = vec![(0, 1), (1, 2), (3, 4)];
        cc.compute_full(&edges);
        
        // 应该有 2 个组件：{0,1,2} 和 {3,4}
        assert_eq!(cc.count_components(), 2);
        
        // 增量更新（添加边：2-3，合并两个组件）
        let new_edges = vec![(2, 3)];
        let merged = cc.update_incremental(&new_edges);
        
        // 应该合并了 1 次
        assert_eq!(merged, 1);
        
        // 现在应该有 1 个组件：{0,1,2,3,4}
        assert_eq!(cc.count_components(), 1);
        
        println!("✅ 增量 Connected Components 测试通过！");
    }
}
