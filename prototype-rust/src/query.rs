// src/query.rs
// Fluent Builder 查询 API（方案 A）
//
// 设计原则：
// - 类型安全，编译期检查
// - Rust 惯用链式调用
// - 底层复用 PersistentGraph 的 HashMaps
// - 自动使用索引（如果可用）
// - 复杂遍历自动转 CSR（如需 GPU 加速）

use std::collections::{HashMap, VecDeque};
use crate::PropertyValue;
use crate::persistence::PersistentGraph;

// ── 顶点查询 Builder ─────────────────────────

/// 顶点查询构建器
///
/// 用法：
/// ```rust
/// let results = graph.find_vertex()
///     .with_property("name", PropertyValue::String("Alice".to_string()))
///     .with_property("age", PropertyValue::Int(30))
///     .execute();
/// ```
pub struct VertexQuery<'a> {
    graph: &'a PersistentGraph,
    filters: Vec<Box<dyn Fn(&HashMap<String, PropertyValue>) -> bool + 'a>>,
    id_filter: Option<u64>,
    /// 索引提示：(property_key, value) 列表（用于 with_property 精确匹配）
    index_hints: Vec<(String, PropertyValue)>,
}

impl<'a> VertexQuery<'a> {
    pub fn new(graph: &'a PersistentGraph) -> Self {
        VertexQuery {
            graph,
            filters: Vec::new(),
            id_filter: None,
            index_hints: Vec::new(),
        }
    }

    /// 按 ID 过滤
    pub fn with_id(mut self, id: u64) -> Self {
        self.id_filter = Some(id);
        self
    }

    /// 按属性过滤（精确匹配）
    ///
    /// 如果属性有索引，会自动使用索引加速。
    pub fn with_property(mut self, key: &str, value: PropertyValue) -> Self {
        let key = key.to_string();
        // 记录索引提示
        self.index_hints.push((key.clone(), value.clone()));
        // 保留原有过滤逻辑（用于无索引时的全表扫描，或索引结果的二次过滤）
        self.filters.push(Box::new(move |props| {
            props.get(&key) == Some(&value)
        }));
        self
    }

    /// 按属性过滤（自定义谓词）
    ///
    /// 注意：自定义谓词无法使用索引，会退化为全表扫描。
    pub fn filter_by<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&HashMap<String, PropertyValue>) -> bool + 'a,
    {
        self.filters.push(Box::new(predicate));
        self
    }

    /// 执行查询，返回匹配的顶点 ID 列表
    ///
    /// 优化策略：
    /// 1. 如果有 ID 过滤，直接返回该 ID（如果存在）
    /// 2. 如果有索引提示且索引存在，使用索引获取候选 ID，然后应用过滤
    /// 3. 否则，全表扫描
    pub fn execute(&self) -> Vec<u64> {
        // 策略 1：ID 过滤
        if let Some(filter_id) = self.id_filter {
            if self.graph.vertices.contains_key(&filter_id) {
                // 应用属性过滤
                let vr = &self.graph.vertices[&filter_id];
                let mut passed = true;
                for filter in &self.filters {
                    if !filter(&vr.properties) {
                        passed = false;
                        break;
                    }
                }
                if passed {
                    return vec![filter_id];
                }
            }
            return Vec::new();
        }

        // 策略 2：尝试使用索引
        if !self.index_hints.is_empty() {
            if let Some(ids) = self.try_index_lookup() {
                // 索引命中，对候选 ID 应用过滤
                return self.apply_filters_to_ids(ids);
            }
        }

        // 策略 3：全表扫描
        self.full_table_scan()
    }

    /// 尝试使用索引查找
    ///
    /// 返回 Some(ids) 如果索引命中，None 如果需要退化为全表扫描。
    fn try_index_lookup(&self) -> Option<Vec<u64>> {
        // 找到第一个有索引的 property key
        for (key, value) in &self.index_hints {
            if let Some(ids) = self.graph.index_manager.query_exact(key, value) {
                // 索引命中
                if !ids.is_empty() {
                    return Some(ids);
                } else {
                    // 索引存在，但查不到结果（属性值不存在）
                    return Some(Vec::new());
                }
            }
        }
        // 没有可用的索引
        None
    }

    /// 对候选 ID 列表应用所有过滤条件
    fn apply_filters_to_ids(&self, ids: Vec<u64>) -> Vec<u64> {
        let mut results = Vec::new();
        for id in ids {
            if let Some(vr) = self.graph.vertices.get(&id) {
                let mut passed = true;
                for filter in &self.filters {
                    if !filter(&vr.properties) {
                        passed = false;
                        break;
                    }
                }
                if passed {
                    results.push(id);
                }
            }
        }
        results
    }

    /// 全表扫描
    fn full_table_scan(&self) -> Vec<u64> {
        let mut results: Vec<u64> = Vec::new();

        for (&id, vertex) in &self.graph.vertices {
            // 属性过滤
            let mut passed = true;
            for filter in &self.filters {
                if !filter(&vertex.properties) {
                    passed = false;
                    break;
                }
            }

            if passed {
                results.push(id);
            }
        }

        results
    }

    /// 执行查询，返回匹配的顶点记录（含属性）
    pub fn execute_with_properties(&self) -> Vec<(u64, HashMap<String, PropertyValue>)> {
        self.execute()
            .into_iter()
            .map(|id| {
                let vr = &self.graph.vertices[&id];
                (id, vr.properties.clone())
            })
            .collect()
    }
}

// ── 边查询 Builder ─────────────────────────

/// 边查询构建器
///
/// 用法：
/// ```rust
/// let edges = graph.find_edges()
///     .from(1)
///     .with_edge_property("type", PropertyValue::String("knows".to_string()))
///     .execute();
/// ```
pub struct EdgeQuery<'a> {
    graph: &'a PersistentGraph,
    from_filter: Option<u64>,
    to_filter: Option<u64>,
    property_filters: Vec<Box<dyn Fn(&HashMap<String, PropertyValue>) -> bool + 'a>>,
}

impl<'a> EdgeQuery<'a> {
    pub fn new(graph: &'a PersistentGraph) -> Self {
        EdgeQuery {
            graph,
            from_filter: None,
            to_filter: None,
            property_filters: Vec::new(),
        }
    }

    /// 过滤起点
    pub fn from(mut self, id: u64) -> Self {
        self.from_filter = Some(id);
        self
    }

    /// 过滤终点
    pub fn to(mut self, id: u64) -> Self {
        self.to_filter = Some(id);
        self
    }

    /// 过滤边属性（精确匹配）
    pub fn with_edge_property(mut self, key: &str, value: PropertyValue) -> Self {
        let key = key.to_string();
        self.property_filters.push(Box::new(move |props| {
            props.get(&key) == Some(&value)
        }));
        self
    }

    /// 执行查询，返回匹配的边 ((from, to), weight, properties)
    pub fn execute(&self) -> Vec<((u64, u64), f64, HashMap<String, PropertyValue>)> {
        let mut results = Vec::new();

        for (&(from, to), edge) in &self.graph.edges {
            // from 过滤
            if let Some(f) = self.from_filter {
                if from != f {
                    continue;
                }
            }

            // to 过滤
            if let Some(t) = self.to_filter {
                if to != t {
                    continue;
                }
            }

            // 属性过滤
            let mut passed = true;
            for filter in &self.property_filters {
                if !filter(&edge.properties) {
                    passed = false;
                    break;
                }
            }

            if passed {
                results.push(((from, to), edge.weight, edge.properties.clone()));
            }
        }

        results
    }
}

// ── PersistentGraph 查询方法 ─────────────────────────

impl PersistentGraph {
    /// 开始顶点查询
    pub fn find_vertex(&self) -> VertexQuery<'_> {
        VertexQuery::new(self)
    }

    /// 开始边查询
    pub fn find_edges(&self) -> EdgeQuery<'_> {
        EdgeQuery::new(self)
    }

    /// 开始模式匹配查询
    pub fn match_pattern(&self) -> PatternQuery<'_> {
        PatternQuery::new(self)
    }
}

// ── 邻居查询 ─────────────────────────

impl PersistentGraph {
    /// 查出边邻居（out-neighbors）
    pub fn out_neighbors(&self, vertex_id: u64) -> Vec<u64> {
        let mut neighbors = Vec::new();
        for (&(from, to), _) in &self.edges {
            if from == vertex_id {
                neighbors.push(to);
            }
        }
        neighbors
    }

    /// 查入边邻居（in-neighbors）
    pub fn in_neighbors(&self, vertex_id: u64) -> Vec<u64> {
        let mut neighbors = Vec::new();
        for (&(from, to), _) in &self.edges {
            if to == vertex_id {
                neighbors.push(from);
            }
        }
        neighbors
    }

    /// 查所有邻居（双向）
    pub fn all_neighbors(&self, vertex_id: u64) -> Vec<u64> {
        let mut neighbors = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for (&(from, to), _) in &self.edges {
            if from == vertex_id && seen.insert(to) {
                neighbors.push(to);
            }
            if to == vertex_id && seen.insert(from) {
                neighbors.push(from);
            }
        }

        neighbors
    }
}

// ── 最短路径（BFS）─────────────────────────

impl PersistentGraph {
    /// 最短路径（无权图）
    ///
    /// 返回路径 [start, v1, v2, ..., end]，如果不可达则返回空向量。
    pub fn shortest_path(&self, start: u64, end: u64) -> Vec<u64> {
        if start == end {
            return vec![start];
        }

        let mut queue = VecDeque::new();
        let mut visited = std::collections::HashSet::new();
        let mut parent: HashMap<u64, u64> = HashMap::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            for &neighbor in &self.out_neighbors(current) {
                if visited.insert(neighbor) {
                    parent.insert(neighbor, current);
                    if neighbor == end {
                        // 重建路径
                        let mut path = Vec::new();
                        let mut current = end;
                        while current != start {
                            path.push(current);
                            current = parent[&current];
                        }
                        path.push(start);
                        path.reverse();
                        return path;
                    }
                    queue.push_back(neighbor);
                }
            }
        }

        Vec::new()  // 不可达
    }

    /// 最短路径（带权图，Dijkstra）
    ///
    /// 返回 (路径, 总权重)
    pub fn shortest_path_weighted(&self, start: u64, end: u64) -> (Vec<u64>, f64) {
        use std::collections::BinaryHeap;
        use std::cmp::Ordering;

        #[derive(Debug, Clone, Copy)]
        struct State {
            vertex: u64,
            dist: f64,
        }

        impl PartialEq for State {
            fn eq(&self, other: &Self) -> bool {
                self.dist == other.dist
            }
        }
        impl Eq for State {}

        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                // 最小堆：按 dist 升序
                self.dist
                    .partial_cmp(&other.dist)
                    .unwrap_or(Ordering::Equal)
                    .reverse()
            }
        }

        if start == end {
            return (vec![start], 0.0);
        }

        let mut heap = BinaryHeap::new();
        let mut dist: HashMap<u64, f64> = HashMap::new();
        let mut parent: HashMap<u64, u64> = HashMap::new();

        dist.insert(start, 0.0);
        heap.push(State { vertex: start, dist: 0.0 });

        while let Some(State { vertex, dist: current_dist }) = heap.pop() {
            if vertex == end {
                // 重建路径
                let mut path = Vec::new();
                let mut current = end;
                while current != start {
                    path.push(current);
                    current = parent[&current];
                }
                path.push(start);
                path.reverse();
                return (path, current_dist);
            }

            if current_dist > *dist.get(&vertex).unwrap_or(&f64::INFINITY) {
                continue;
            }

            // 遍历出边
            for &(from, to) in self.edges.keys() {
                if from == vertex {
                    let edge = &self.edges[&(from, to)];
                    let new_dist = current_dist + edge.weight;
                    let old_dist = dist.get(&to).copied().unwrap_or(f64::INFINITY);
                    if new_dist < old_dist {
                        dist.insert(to, new_dist);
                        parent.insert(to, vertex);
                        heap.push(State { vertex: to, dist: new_dist });
                    }
                }
            }
        }

        (Vec::new(), f64::INFINITY)  // 不可达
    }
}

// ── 模式匹配（MVP）─────────────────────────

/// 模式匹配步骤
pub enum PatternStep {
    /// 绑定一个顶点，可选过滤条件
    BindVertex {
        name: String,
        filter: Option<Box<dyn Fn(&HashMap<String, PropertyValue>) -> bool>>,
    },
    /// 出边
    Outgoing { edge_type: Option<String> },
    /// 入边
    Incoming { edge_type: Option<String> },
}

/// 模式匹配构建器
///
/// 用法：
/// ```rust
/// let results = graph.match_pattern()
///     .bind("a", |p| p.get("name") == Some(&PropertyValue::String("Alice".to_string())))
///     .outgoing(Some("knows"))
///     .bind("b", |_| true)
///     .execute();
/// ```
pub struct PatternQuery<'a> {
    graph: &'a PersistentGraph,
    steps: Vec<PatternStep>,
}

impl<'a> PatternQuery<'a> {
    pub fn new(graph: &'a PersistentGraph) -> Self {
        PatternQuery {
            graph,
            steps: Vec::new(),
        }
    }

    /// 绑定一个顶点
    pub fn bind<F>(mut self, name: &str, filter: F) -> Self
    where
        F: Fn(&HashMap<String, PropertyValue>) -> bool + 'static,
    {
        self.steps.push(PatternStep::BindVertex {
            name: name.to_string(),
            filter: Some(Box::new(filter)),
        });
        self
    }

    /// 出边
    pub fn outgoing(mut self, edge_type: Option<&str>) -> Self {
        self.steps.push(PatternStep::Outgoing {
            edge_type: edge_type.map(|s| s.to_string()),
        });
        self
    }

    /// 入边
    pub fn incoming(mut self, edge_type: Option<&str>) -> Self {
        self.steps.push(PatternStep::Incoming {
            edge_type: edge_type.map(|s| s.to_string()),
        });
        self
    }

    /// 执行模式匹配
    ///
    /// 返回匹配的结果：每个结果是 HashMap<绑定名, 顶点 ID>
    pub fn execute(&self) -> Vec<HashMap<String, u64>> {
        if self.steps.is_empty() {
            return Vec::new();
        }

        // 简化实现：只支持 BindVertex -> Outgoing -> BindVertex 模式
        // 完整实现需要回溯搜索
        self.execute_simple()
    }

    fn execute_simple(&self) -> Vec<HashMap<String, u64>> {
        let mut results = Vec::new();

        // 找到第一个 BindVertex
        let mut step_idx = 0;
        while step_idx < self.steps.len() {
            if let PatternStep::BindVertex { name, filter } = &self.steps[step_idx] {
                // 找到所有匹配第一个顶点的 ID
                let candidate_ids = if let Some(f) = filter {
                    self.graph
                        .vertices
                        .iter()
                        .filter(|(_, vr)| f(&vr.properties))
                        .map(|(&id, _)| id)
                        .collect::<Vec<_>>()
                } else {
                    self.graph.vertices.keys().copied().collect()
                };

                // 尝试匹配后续步骤
                for &id in &candidate_ids {
                    let mut binding = HashMap::new();
                    binding.insert(name.clone(), id);
                    if self.match_remaining(step_idx + 1, id, &mut binding) {
                        results.push(binding);
                    }
                }

                break;
            }
            step_idx += 1;
        }

        results
    }

    fn match_remaining(
        &self,
        step_idx: usize,
        current_vertex: u64,
        binding: &mut HashMap<String, u64>,
    ) -> bool {
        if step_idx >= self.steps.len() {
            return true;
        }

        match &self.steps[step_idx] {
            PatternStep::Outgoing { edge_type } => {
                // 找出边邻居
                let neighbors = self.graph.out_neighbors(current_vertex);

                if step_idx + 1 < self.steps.len() {
                    if let PatternStep::BindVertex { name, filter } = &self.steps[step_idx + 1] {
                        for &neighbor in &neighbors {
                            // 检查边类型过滤
                            if let Some(et) = edge_type {
                                let has_edge = self.graph.edges.keys().any(|(f, t)| {
                                    *f == current_vertex
                                        && *t == neighbor
                                        && self.graph.edges[&(*f, *t)]
                                            .properties
                                            .get("type")
                                            == Some(&PropertyValue::String(et.clone()))
                                });
                                if !has_edge {
                                    continue;
                                }
                            }

                            // 检查顶点过滤
                            if let Some(f) = filter {
                                if !f(&self.graph.vertices[&neighbor].properties) {
                                    continue;
                                }
                            }

                            binding.insert(name.clone(), neighbor);
                            if self.match_remaining(step_idx + 2, neighbor, binding) {
                                return true;
                            }
                            binding.remove(name);
                        }
                    }
                }

                false
            }
            PatternStep::Incoming { .. } => {
                // 类似 Outgoing，但查入边
                let neighbors = self.graph.in_neighbors(current_vertex);

                if step_idx + 1 < self.steps.len() {
                    if let PatternStep::BindVertex { name, filter } = &self.steps[step_idx + 1] {
                        for &neighbor in &neighbors {
                            if let Some(f) = filter {
                                if !f(&self.graph.vertices[&neighbor].properties) {
                                    continue;
                                }
                            }

                            binding.insert(name.clone(), neighbor);
                            if self.match_remaining(step_idx + 2, neighbor, binding) {
                                return true;
                            }
                            binding.remove(name);
                        }
                    }
                }

                false
            }
            _ => false,
        }
    }
}

// ── 测试 ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_graph() -> PersistentGraph {
        let mut g = PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: "/tmp/test_query.bin".to_string(),
            index_manager: crate::index::IndexManager::new(),
        };

        // 添加顶点
        let mut alice = HashMap::new();
        alice.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        alice.insert("age".to_string(), PropertyValue::Int(30));
        g.add_vertex(1, alice);

        let mut bob = HashMap::new();
        bob.insert("name".to_string(), PropertyValue::String("Bob".to_string()));
        bob.insert("age".to_string(), PropertyValue::Int(25));
        g.add_vertex(2, bob);

        let mut charlie = HashMap::new();
        charlie.insert("name".to_string(), PropertyValue::String("Charlie".to_string()));
        charlie.insert("age".to_string(), PropertyValue::Int(30));
        g.add_vertex(3, charlie);

        // 添加边
        let mut knows = HashMap::new();
        knows.insert("type".to_string(), PropertyValue::String("knows".to_string()));
        g.add_edge(1, 2, 1.0, knows.clone());
        g.add_edge(2, 3, 1.0, knows);

        g
    }

    #[test]
    fn test_find_vertex_by_property() {
        let g = make_graph();

        let results = g.find_vertex()
            .with_property("name", PropertyValue::String("Alice".to_string()))
            .execute();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], 1);
    }

    #[test]
    fn test_find_vertex_by_property_no_index() {
        let g = make_graph();

        // 没有索引，退化为全表扫描
        let results = g.find_vertex()
            .with_property("age", PropertyValue::Int(30))
            .execute();

        assert_eq!(results.len(), 2);  // Alice 和 Charlie 都是 30
    }

    #[test]
    fn test_find_vertex_with_index() {
        let mut g = make_graph();

        // 创建索引
        g.create_index("idx_name", "name", false).unwrap();

        // 使用索引
        let results = g.find_vertex()
            .with_property("name", PropertyValue::String("Alice".to_string()))
            .execute();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], 1);
    }

    #[test]
    fn test_out_neighbors() {
        let g = make_graph();

        let neighbors = g.out_neighbors(1);
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0], 2);
    }

    #[test]
    fn test_shortest_path() {
        let g = make_graph();

        let path = g.shortest_path(1, 3);
        assert_eq!(path, vec![1, 2, 3]);
    }

    #[test]
    fn test_find_edges() {
        let g = make_graph();

        let edges = g.find_edges()
            .from(1)
            .execute();

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0, (1, 2));
    }

    #[test]
    fn test_match_pattern() {
        let g = make_graph();

        let results = g.match_pattern()
            .bind("a", |p| p.get("name") == Some(&PropertyValue::String("Alice".to_string())))
            .outgoing(Some("knows"))
            .bind("b", |_| true)
            .execute();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["a"], 1);
        assert_eq!(results[0]["b"], 2);
    }
}
