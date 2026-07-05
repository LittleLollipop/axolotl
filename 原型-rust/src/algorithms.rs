// src/algorithms.rs
// Axolotl-RS: 图算法实现（支持增量计算）

use super::*;
use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet, VecDeque};

/// 图算法 trait（统一接口）
pub trait GraphAlgorithms {
    /// 最短路径（BFS）
    fn shortest_path(&self, start: VertexId, target: VertexId) -> Vec<VertexId>;
    
    /// PageRank（支持增量更新）
    fn pagerank(&mut self, damping_factor: f64, max_iterations: u32, tolerance: f64) -> HashMap<VertexId, f64>;
    
    /// Betweenness Centrality（近似算法，采样 k 个顶点）
    fn betweenness_centrality(&self) -> HashMap<VertexId, f64>;
    
    /// Betweenness Centrality（近似算法，指定采样数量）
    fn betweenness_centrality_approximate(&self, k: usize) -> HashMap<VertexId, f64>;
    
    /// 连通分量
    fn connected_components(&self) -> Vec<HashSet<VertexId>>;
    
    /// 社区检测（贪心算法）
    fn detect_communities_greedy(&self) -> HashMap<VertexId, u32>;
}

impl GraphAlgorithms for GraphDB {
    /// 最短路径（BFS - 简化版本，确保正确性）
    fn shortest_path(&self, start: VertexId, target: VertexId) -> Vec<VertexId> {
        if start == target {
            return vec![start];
        }
        
        // 使用 BFS 查找最短路径
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(VertexId, Vec<VertexId>)> = VecDeque::new();
        
        queue.push_back((start, vec![start]));
        visited.insert(start);
        
        while let Some((current, path)) = queue.pop_front() {
            if current == target {
                return path;
            }
            
            for neighbor in self.get_neighbors(current) {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    let mut new_path = path.clone();
                    new_path.push(neighbor);
                    queue.push_back((neighbor, new_path));
                }
            }
        }
        
        vec![] // 未找到路径
    }
    
    /// PageRank（支持增量更新）⭐ 核心优势
    fn pagerank(&mut self, damping_factor: f64, max_iterations: u32, tolerance: f64) -> HashMap<VertexId, f64> {
        let n = self.vertex_count() as f64;
        
        // 检查缓存是否有效
        if let Some(ref cache) = self.pagerank_cache {
            // 如果图未修改，直接返回缓存
            return cache.clone();
        }
        
        // 初始化 PageRank 值
        let mut pr = HashMap::new();
        let initial_value = 1.0 / n;
        for vertex_id in self.vertices.keys() {
            pr.insert(*vertex_id, initial_value);
        }
        
        // 幂迭代
        for _ in 0..max_iterations {
            let mut new_pr = HashMap::new();
            
            // 初始化为新值（dangling 节点处理）
            for vertex_id in self.vertices.keys() {
                new_pr.insert(*vertex_id, (1.0 - damping_factor) / n);
            }
            
            // 计算新的 PageRank 值
            for (vertex_id, pr_value) in &pr {
                let neighbors = self.get_neighbors(*vertex_id);
                if neighbors.is_empty() {
                    // dangling 节点：贡献均匀分布到所有节点
                    let contribution = pr_value * damping_factor / n;
                    for other_id in self.vertices.keys() {
                        *new_pr.get_mut(other_id).unwrap() += contribution;
                    }
                } else {
                    // 非 dangling 节点：贡献分布到邻居
                    let contribution = pr_value * damping_factor / neighbors.len() as f64;
                    for neighbor in neighbors {
                        *new_pr.get_mut(&neighbor).unwrap() += contribution;
                    }
                }
            }
            
            // 检查收敛
            let mut converged = true;
            for (vertex_id, pr_value) in &pr {
                if (new_pr[vertex_id] - pr_value).abs() > tolerance {
                    converged = false;
                    break;
                }
            }
            
            pr = new_pr;
            
            if converged {
                break;
            }
        }
        
        // 更新缓存
        self.pagerank_cache = Some(pr.clone());
        
        pr
    }
    
    /// Betweenness Centrality (优化版本)
    /// 使用近似算法：只采样 k 个顶点作为源点
    fn betweenness_centrality(&self) -> HashMap<VertexId, f64> {
        let k = (self.vertices.len() as f64).sqrt().ceil() as usize; // 采样 sqrt(n) 个顶点
        self.betweenness_centrality_approximate(k)
    }
    
    /// Betweenness Centrality (近似算法)
    /// 只采样 k 个顶点作为源点，大幅提升性能
    fn betweenness_centrality_approximate(&self, k: usize) -> HashMap<VertexId, f64> {
        let mut betweenness = HashMap::new();
        for (vertex_id, _) in &self.vertices {
            betweenness.insert(*vertex_id, 0.0);
        }
        
        let vertex_ids: Vec<VertexId> = self.vertices.keys().cloned().collect();
        let sample_size = k.min(vertex_ids.len());
        
        // 采样 k 个顶点
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let sampled_vertices: Vec<VertexId> = vertex_ids
            .choose_multiple(&mut rng, sample_size)
            .cloned()
            .collect();
        
        // 对每个采样顶点运行 BFS
        for &s in &sampled_vertices {
            let mut stack = Vec::new();
            let mut predecessors: HashMap<VertexId, Vec<VertexId>> = HashMap::new();
            let mut distance: HashMap<VertexId, i32> = HashMap::new();
            let mut sigma: HashMap<VertexId, f64> = HashMap::new();
            
            // 初始化
            for &vertex_id in &vertex_ids {
                distance.insert(vertex_id, -1);
                sigma.insert(vertex_id, 0.0);
            }
            distance.insert(s, 0);
            sigma.insert(s, 1.0);
            
            let mut queue = VecDeque::new();
            queue.push_back(s);
            
            while let Some(v) = queue.pop_front() {
                stack.push(v);
                
                let neighbors: Vec<VertexId> = self.get_neighbors(v).into_iter().collect();
                
                for neighbor in neighbors {
                    if *distance.get(&neighbor).unwrap_or(&-1) < 0 {
                        queue.push_back(neighbor);
                        let new_dist = distance[&v] + 1;
                        distance.insert(neighbor, new_dist);
                    }
                    
                    if *distance.get(&neighbor).unwrap_or(&-1) == distance[&v] + 1 {
                        let sigma_v = sigma[&v];
                        *sigma.get_mut(&neighbor).unwrap() += sigma_v;
                        predecessors.entry(neighbor).or_insert_with(Vec::new).push(v);
                    }
                }
            }
            
            // 累积依赖
            let mut delta: HashMap<VertexId, f64> = HashMap::new();
            for &vertex_id in &vertex_ids {
                delta.insert(vertex_id, 0.0);
            }
            
            while let Some(w) = stack.pop() {
                if let Some(preds) = predecessors.get(&w) {
                    for &v in preds {
                        let sigma_v = sigma[&v];
                        let sigma_w = sigma[&w];
                        let delta_w = delta[&w];
                        let contribution = (sigma_v / sigma_w) * (1.0 + delta_w);
                        *delta.get_mut(&v).unwrap() += contribution;
                    }
                }
                
                if w != s {
                    *betweenness.get_mut(&w).unwrap() += delta[&w];
                }
            }
        }
        
        // 归一化（因为只采样了 k 个顶点）
        let scale = (vertex_ids.len() as f64) / (sample_size as f64);
        for value in betweenness.values_mut() {
            *value *= scale;
        }
        
        // 无向图需要除以 2
        for value in betweenness.values_mut() {
            *value /= 2.0;
        }
        
        betweenness
    }
    
    /// 连通分量
    fn connected_components(&self) -> Vec<HashSet<VertexId>> {
        let mut visited = HashSet::new();
        let mut components = Vec::new();
        
        for &vertex_id in self.vertices.keys() {
            if !visited.contains(&vertex_id) {
                let mut component = HashSet::new();
                let mut queue = VecDeque::new();
                
                queue.push_back(vertex_id);
                visited.insert(vertex_id);
                component.insert(vertex_id);
                
                while let Some(v) = queue.pop_front() {
                    for neighbor in self.get_neighbors(v) {
                        if !visited.contains(&neighbor) {
                            visited.insert(neighbor);
                            component.insert(neighbor);
                            queue.push_back(neighbor);
                        }
                    }
                }
                
                components.push(component);
            }
        }
        
        components
    }
    
    /// 社区检测（贪心算法 - 模块化优化）
    fn detect_communities_greedy(&self) -> HashMap<VertexId, u32> {
        // 初始化：每个顶点是一个社区
        let mut communities: HashMap<VertexId, u32> = HashMap::new();
        let mut community_id = 0;
        
        for vertex_id in self.vertices.keys() {
            communities.insert(*vertex_id, community_id);
            community_id += 1;
        }
        
        let m = self.edge_count() as f64; // 边数
        let mut improved = true;
        
        while improved {
            improved = false;
            
            // 顺序遍历顶点（确定性算法）
            let vertices: Vec<VertexId> = self.vertices.keys().cloned().collect();
            
            for vertex_id in vertices {
                let current_community = communities[&vertex_id];
                
                // 计算移除当前顶点后的模块化变化
                let neighbors = self.get_neighbors(vertex_id);
                if neighbors.is_empty() {
                    continue;
                }
                
                // 计算当前顶点与各个社区的边权重
                let mut community_weights = HashMap::new();
                for neighbor in neighbors {
                    let neighbor_community = communities[&neighbor];
                    let weight = self.get_edge(vertex_id, neighbor).map(|e| e.weight).unwrap_or(1.0);
                    *community_weights.entry(neighbor_community).or_insert(0.0) += weight;
                }
                
                // 尝试移动到最好的社区
                let mut best_community = current_community;
                let mut best_gain = 0.0;
                
                for (&community, &weight) in &community_weights {
                    if community == current_community {
                        continue;
                    }
                    
                    // 计算模块化增益（简化版）
                    let gain = weight / (2.0 * m);
                    if gain > best_gain {
                        best_gain = gain;
                        best_community = community;
                    }
                }
                
                if best_community != current_community && best_gain > 0.0 {
                    communities.insert(vertex_id, best_community);
                    improved = true;
                }
            }
        }
        
        communities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_shortest_path() {
        let mut db = GraphDB::new();
        
        // 创建简单图：1 - 2 - 3
        let v1 = db.add_vertex(HashMap::new()).unwrap();
        let v2 = db.add_vertex(HashMap::new()).unwrap();
        let v3 = db.add_vertex(HashMap::new()).unwrap();
        
        db.add_edge(v1, v2, HashMap::new(), 1.0).unwrap();
        db.add_edge(v2, v3, HashMap::new(), 1.0).unwrap();
        
        let path = db.shortest_path(v1, v3);
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], v1);
        assert_eq!(path[1], v2);
        assert_eq!(path[2], v3);
    }
    
    #[test]
    fn test_pagerank() {
        let mut db = GraphDB::new();
        
        // 创建简单图
        let v1 = db.add_vertex(HashMap::new()).unwrap();
        let v2 = db.add_vertex(HashMap::new()).unwrap();
        let v3 = db.add_vertex(HashMap::new()).unwrap();
        
        db.add_edge(v1, v2, HashMap::new(), 1.0).unwrap();
        db.add_edge(v2, v3, HashMap::new(), 1.0).unwrap();
        
        let pr = db.pagerank(0.85, 100, 1e-6);
        assert_eq!(pr.len(), 3);
        
        // 验证缓存
        assert!(db.pagerank_cache.is_some());
    }
}
