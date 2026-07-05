// src/query.rs
// Fluent Builder 查询 API（方案 A）
//
// 设计原则：
// - 类型安全，编译期检查
// - Rust 惯用链式调用
// - 底层复用 PersistentGraph 的 HashMaps
// - 复杂遍历自动转 CSR（如需 GPU 加速）

use std::collections::{HashMap, VecDeque};
use crate::PropertyValue;
use crate::persistence::{PersistentGraph, VertexRecord, EdgeRecord};

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
}

impl<'a> VertexQuery<'a> {
    pub fn new(graph: &'a PersistentGraph) -> Self {
        VertexQuery {
            graph,
            filters: Vec::new(),
            id_filter: None,
        }
    }

    /// 按 ID 过滤
    pub fn with_id(mut self, id: u64) -> Self {
        self.id_filter = Some(id);
        self
    }

    /// 按属性过滤（精确匹配）
    pub fn with_property(mut self, key: &str, value: PropertyValue) -> Self {
        let key = key.to_string();
        self.filters.push(Box::new(move |props| {
            props.get(&key) == Some(&value)
        }));
        self
    }

    /// 按属性过滤（自定义谓词）
    pub fn filter_by<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&HashMap<String, PropertyValue>) -> bool + 'a,
    {
        self.filters.push(Box::new(predicate));
        self
    }

    /// 执行查询，返回匹配的顶点 ID 列表
    pub fn execute(&self) -> Vec<u64> {
        let mut results: Vec<u64> = Vec::new();

        for (&id, vertex) in &self.graph.vertices {
            // ID 过滤
            if let Some(filter_id) = self.id_filter {
                if id != filter_id {
                    continue;
                }
            }

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

// ── 邻居查询 ─────────────────────────

/// 邻居查询结果
#[derive(Debug, Clone)]
pub struct Neighbor {
    pub vertex_id: u64,
    pub properties: HashMap<String, PropertyValue>,
    pub edge_weight: f64,
    pub edge_properties: HashMap<String, PropertyValue>,
}

/// 查邻居（直接返回，不用 Builder，因为参数简单）
impl PersistentGraph {
    /// 查顶点的所有出边邻居
    pub fn out_neighbors(&self, vertex_id: u64) -> Vec<Neighbor> {
        let mut neighbors = Vec::new();

        for (&(from, to), edge) in &self.edges {
            if from == vertex_id {
                let props = self.vertices.get(&to)
                    .map(|vr| vr.properties.clone())
                    .unwrap_or_default();
                neighbors.push(Neighbor {
                    vertex_id: to,
                    properties: props,
                    edge_weight: edge.weight,
                    edge_properties: edge.properties.clone(),
                });
            }
        }

        neighbors
    }

    /// 查顶点的所有入边邻居
    pub fn in_neighbors(&self, vertex_id: u64) -> Vec<Neighbor> {
        let mut neighbors = Vec::new();

        for (&(from, to), edge) in &self.edges {
            if to == vertex_id {
                let props = self.vertices.get(&from)
                    .map(|vr| vr.properties.clone())
                    .unwrap_or_default();
                neighbors.push(Neighbor {
                    vertex_id: from,
                    properties: props,
                    edge_weight: edge.weight,
                    edge_properties: edge.properties.clone(),
                });
            }
        }

        neighbors
    }

    /// 查顶点的所有邻居（出入边都算）
    pub fn all_neighbors(&self, vertex_id: u64) -> Vec<Neighbor> {
        let mut neighbors = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for n in self.out_neighbors(vertex_id) {
            neighbors.push(n.clone());
            seen.insert(n.vertex_id);
        }

        for n in self.in_neighbors(vertex_id) {
            if !seen.contains(&n.vertex_id) {
                neighbors.push(n);
            }
        }

        neighbors
    }
}

// ── 最短路径（BFS）─────────────────────────

impl PersistentGraph {
    /// 最短路径（BFS，无权/单位权）
    ///
    /// 返回路径上的顶点 ID 列表（包含 start 和 end）
    /// 如果不可达，返回空 Vec
    pub fn shortest_path(&self, start: u64, end: u64) -> Vec<u64> {
        if start == end {
            return vec![start];
        }

        // BFS
        let mut queue = VecDeque::new();
        let mut parent: HashMap<u64, u64> = HashMap::new();
        let mut visited = std::collections::HashSet::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            for neighbor in self.out_neighbors(current) {
                let nid = neighbor.vertex_id;
                if !visited.contains(&nid) {
                    visited.insert(nid);
                    parent.insert(nid, current);
                    queue.push_back(nid);

                    if nid == end {
                        // 重建路径
                        let mut path = Vec::new();
                        let mut cur = end;
                        while cur != start {
                            path.push(cur);
                            cur = parent[&cur];
                        }
                        path.push(start);
                        path.reverse();
                        return path;
                    }
                }
            }
        }

        Vec::new() // 不可达
    }

    /// 最短路径（按边权重，Dijkstra）
    ///
    /// 返回 (路径, 总权重)
    pub fn shortest_path_weighted(&self, start: u64, end: u64) -> (Vec<u64>, f64) {
        use std::collections::BinaryHeap;
        use std::cmp::Ordering;

        #[derive(Debug, Clone, Copy)]
        struct State {
            vertex: u64,
            cost: f64,
        }

        impl Eq for State {}
        impl PartialEq for State {
            fn eq(&self, other: &Self) -> bool {
                self.cost == other.cost
            }
        }
        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                // 注意：BinaryHeap 是最大堆，所以用 reverse ordering
                self.cost
                    .partial_cmp(&other.cost)
                    .unwrap_or(Ordering::Equal)
                    .reverse()
            }
        }
        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut dist: HashMap<u64, f64> = HashMap::new();
        let mut parent: HashMap<u64, u64> = HashMap::new();
        let mut heap = BinaryHeap::new();

        dist.insert(start, 0.0);
        heap.push(State { vertex: start, cost: 0.0 });

        while let Some(State { vertex, cost }) = heap.pop() {
            if vertex == end {
                // 重建路径
                let mut path = Vec::new();
                let mut cur = end;
                while cur != start {
                    path.push(cur);
                    cur = parent[&cur];
                }
                path.push(start);
                path.reverse();
                return (path, cost);
            }

            // 如果当前 cost 比记录的大，跳过
            if let Some(&d) = dist.get(&vertex) {
                if cost > d {
                    continue;
                }
            }

            for neighbor in self.out_neighbors(vertex) {
                let next = neighbor.vertex_id;
                let next_cost = cost + neighbor.edge_weight;

                let current_dist = dist.get(&next).copied().unwrap_or(f64::INFINITY);
                if next_cost < current_dist {
                    dist.insert(next, next_cost);
                    parent.insert(next, vertex);
                    heap.push(State { vertex: next, cost: next_cost });
                }
            }
        }

        (Vec::new(), f64::INFINITY) // 不可达
    }
}

// ── 模式匹配 ─────────────────────────

/// 模式匹配结果中的一条绑定
#[derive(Debug, Clone)]
pub struct Binding {
    pub vertex_id: u64,
    pub properties: HashMap<String, PropertyValue>,
}

/// 模式匹配的一条结果
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub bindings: HashMap<String, Binding>,
}

/// 模式匹配 Builder
///
/// 用法：
/// ```rust
/// let results = graph.match_pattern()
///     .bind("a", |props| props.get("name") == Some(&PropertyValue::String("Alice".to_string())))
///     .outgoing("knows")  // 边类型过滤（用 edge property "type"）
///     .bind("b", |_| true)
///     .execute();
/// ```
pub struct PatternQuery<'a> {
    graph: &'a PersistentGraph,
    steps: Vec<PatternStep<'a>>,
}

enum PatternStep<'a> {
    Bind {
        name: String,
        predicate: Box<dyn Fn(&HashMap<String, PropertyValue>) -> bool + 'a>,
    },
    Outgoing {
        edge_type: Option<String>,
    },
    Incoming {
        edge_type: Option<String>,
    },
}

impl<'a> PatternQuery<'a> {
    pub fn new(graph: &'a PersistentGraph) -> Self {
        PatternQuery {
            graph,
            steps: Vec::new(),
        }
    }

    /// 绑定一个顶点变量
    pub fn bind<F>(mut self, name: &str, predicate: F) -> Self
    where
        F: Fn(&HashMap<String, PropertyValue>) -> bool + 'a,
    {
        self.steps.push(PatternStep::Bind {
            name: name.to_string(),
            predicate: Box::new(predicate),
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
    /// 当前实现：只支持简单链状模式 (a)-[r:type]->(b)
    /// 复杂模式（多分支、循环）后续扩展
    pub fn execute(&self) -> Vec<MatchResult> {
        // 简化实现：只处理 (a)-[outgoing]->(b) 这种两顶点模式
        // 完整实现需要回溯搜索，这里先做 MVP

        let mut results = Vec::new();

        // 找到第一个 Bind 和第一个 Outgoing/Incoming，然后找到第二个 Bind
        // 简化：假设模式是 Bind -> Outgoing -> Bind
        let mut bind_names = Vec::new();
        let mut edge_direction = None; // "outgoing" or "incoming"
        let mut edge_type_filter = None;
        let mut bind_predicates = Vec::new();

        for step in &self.steps {
            match step {
                PatternStep::Bind { name, predicate } => {
                    bind_names.push(name.clone());
                    bind_predicates.push(predicate);
                }
                PatternStep::Outgoing { edge_type } => {
                    edge_direction = Some("outgoing");
                    edge_type_filter = edge_type.clone();
                }
                PatternStep::Incoming { edge_type } => {
                    edge_direction = Some("incoming");
                    edge_type_filter = edge_type.clone();
                }
            }
        }

        if bind_names.len() != 2 || edge_direction.is_none() {
            // 不支持的模式，返回空
            return results;
        }

        let a_predicate = bind_predicates[0];
        let b_predicate = bind_predicates[1];

        // 枚举所有 a 顶点
        for (&a_id, a_vertex) in &self.graph.vertices {
            if !a_predicate(&a_vertex.properties) {
                continue;
            }

            // 根据方向找邻居
            let neighbors = match edge_direction {
                Some("outgoing") => self.graph.out_neighbors(a_id),
                Some("incoming") => self.graph.in_neighbors(a_id),
                _ => continue,
            };

            for n in neighbors {
                // 边类型过滤
                if let Some(ref etype) = edge_type_filter {
                    if n.edge_properties.get("type") != Some(&PropertyValue::String(etype.clone())) {
                        continue;
                    }
                }

                // b 顶点过滤
                if !b_predicate(&n.properties) {
                    continue;
                }

                // 匹配成功
                let mut bindings = HashMap::new();
                bindings.insert(bind_names[0].clone(), Binding {
                    vertex_id: a_id,
                    properties: a_vertex.properties.clone(),
                });
                bindings.insert(bind_names[1].clone(), Binding {
                    vertex_id: n.vertex_id,
                    properties: n.properties.clone(),
                });

                results.push(MatchResult { bindings });
            }
        }

        results
    }
}

// ── PersistentGraph 扩展方法 ─────────────────────────

impl PersistentGraph {
    /// 创建顶点查询
    pub fn find_vertex(&self) -> VertexQuery {
        VertexQuery::new(self)
    }

    /// 创建边查询
    pub fn find_edges(&self) -> EdgeQuery {
        EdgeQuery::new(self)
    }

    /// 创建模式匹配查询
    pub fn match_pattern(&self) -> PatternQuery {
        PatternQuery::new(self)
    }
}

// ── 测试 ───────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_graph() -> PersistentGraph {
        let test_path = "/tmp/test_query_graph.bin";

        // 删除旧文件
        let _ = fs::remove_file(test_path);

        let mut g = PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: test_path.to_string(),
        };

        // 顶点
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
        charlie.insert("age".to_string(), PropertyValue::Int(35));
        g.add_vertex(3, charlie);

        // 边: 1->2, 1->3, 2->3
        let mut knows = HashMap::new();
        knows.insert("type".to_string(), PropertyValue::String("knows".to_string()));
        g.add_edge(1, 2, 1.0, knows.clone());

        g.add_edge(1, 3, 2.0, knows.clone());

        let mut likes = HashMap::new();
        likes.insert("type".to_string(), PropertyValue::String("likes".to_string()));
        g.add_edge(2, 3, 1.5, likes);

        g
    }

    #[test]
    fn test_find_vertex_by_property() {
        let g = setup_test_graph();

        let results = g.find_vertex()
            .with_property("name", PropertyValue::String("Alice".to_string()))
            .execute();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], 1);
    }

    #[test]
    fn test_find_vertex_by_id() {
        let g = setup_test_graph();

        let results = g.find_vertex()
            .with_id(2)
            .execute();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], 2);
    }

    #[test]
    fn test_out_neighbors() {
        let g = setup_test_graph();

        let neighbors = g.out_neighbors(1);
        assert_eq!(neighbors.len(), 2);

        let neighbor_ids: Vec<u64> = neighbors.iter().map(|n| n.vertex_id).collect();
        assert!(neighbor_ids.contains(&2));
        assert!(neighbor_ids.contains(&3));
    }

    #[test]
    fn test_shortest_path() {
        let g = setup_test_graph();

        let path = g.shortest_path(1, 3);
        assert_eq!(path, vec![1, 3]); // 直接边 1->3
    }

    #[test]
    fn test_shortest_path_unreachable() {
        let g = setup_test_graph();

        // 3 没有出边，所以 3->1 不可达
        let path = g.shortest_path(3, 1);
        assert!(path.is_empty());
    }

    #[test]
    fn test_find_edges() {
        let g = setup_test_graph();

        let edges = g.find_edges()
            .from(1)
            .with_edge_property("type", PropertyValue::String("knows".to_string()))
            .execute();

        assert_eq!(edges.len(), 2); // 1->2 和 1->3 都是 knows
    }

    #[test]
    fn test_match_pattern() {
        let g = setup_test_graph();

        let results = g.match_pattern()
            .bind("a", |props| {
                props.get("name") == Some(&PropertyValue::String("Alice".to_string()))
            })
            .outgoing(Some("knows"))
            .bind("b", |_| true)
            .execute();

        assert_eq!(results.len(), 2); // Alice knows Bob, Alice knows Charlie

        // 验证结果包含 Bob 和 Charlie
        let b_ids: Vec<u64> = results.iter()
            .map(|r| r.bindings["b"].vertex_id)
            .collect();
        assert!(b_ids.contains(&2)); // Bob
        assert!(b_ids.contains(&3)); // Charlie
    }
}
