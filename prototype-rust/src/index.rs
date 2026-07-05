// src/index.rs
// 哈希索引模块（精确匹配）
//
// 设计原则：
// - 哈希索引：O(1) 精确匹配查询
// - 自动维护：添加/删除顶点时自动更新索引
// - 可选索引：用户可以创建/删除索引，不影响原有逻辑
// - 查询集成：query.rs 自动使用索引（如果可用）

use std::collections::{HashMap, HashSet};
use crate::PropertyValue;

// ── 索引类型 ─────────────────────────

/// 索引类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    /// 哈希索引（精确匹配）
    Hash,
    /// B-Tree 索引（范围查询，预留）
    BTree,
}

/// 索引定义
#[derive(Debug, Clone)]
pub struct IndexDef {
    pub name: String,
    pub property_key: String,
    pub index_type: IndexType,
    /// 是否唯一索引
    pub unique: bool,
}

// ── 哈希索引 ─────────────────────────

/// 哈希索引：property_value -> [vertex_id]
///
/// 示例：
///   index_name["Alice"] = [1, 5, 9]  // name = "Alice" 的顶点有 1, 5, 9
///   index_age[30] = [2, 8]            // age = 30 的顶点有 2, 8
#[derive(Debug, Clone)]
struct HashIndex {
    /// 索引定义
    def: IndexDef,
    /// 值 -> 顶点 ID 列表（支持多顶点有相同属性值）
    map: HashMap<PropertyValue, Vec<u64>>,
    /// 顶点 ID -> 值（用于快速删除）
    reverse: HashMap<u64, PropertyValue>,
}

impl HashIndex {
    fn new(def: IndexDef) -> Self {
        HashIndex {
            def,
            map: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    /// 插入一个顶点
    fn insert(&mut self, vertex_id: u64, value: PropertyValue) {
        // 唯一索引检查
        if self.def.unique {
            if let Some(existing) = self.map.get(&value) {
                if !existing.is_empty() {
                    // 唯一索引冲突：删除旧值
                    self.remove(existing[0]);
                }
            }
        }

        self.map.entry(value.clone()).or_insert_with(Vec::new).push(vertex_id);
        self.reverse.insert(vertex_id, value);
    }

    /// 删除一个顶点
    fn remove(&mut self, vertex_id: u64) {
        if let Some(value) = self.reverse.remove(&vertex_id) {
            if let Some(ids) = self.map.get_mut(&value) {
                ids.retain(|&id| id != vertex_id);
                if ids.is_empty() {
                    self.map.remove(&value);
                }
            }
        }
    }

    /// 更新一个顶点（先删除，再插入）
    fn update(&mut self, vertex_id: u64, new_value: PropertyValue) {
        self.remove(vertex_id);
        self.insert(vertex_id, new_value);
    }

    /// 查询：精确匹配
    fn query_exact(&self, value: &PropertyValue) -> Vec<u64> {
        self.map.get(value)
            .map(|ids| ids.clone())
            .unwrap_or_else(Vec::new)
    }

    /// 查询：IN（多个值）
    fn query_in(&self, values: &[PropertyValue]) -> Vec<u64> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        for value in values {
            if let Some(ids) = self.map.get(value) {
                for &id in ids {
                    if seen.insert(id) {
                        result.push(id);
                    }
                }
            }
        }
        result
    }

    /// 返回所有索引的项数（不同值的数量）
    fn distinct_count(&self) -> usize {
        self.map.len()
    }

    /// 返回索引大小（总条目数）
    fn entry_count(&self) -> usize {
        self.reverse.len()
    }
}

// ── 索引管理器 ─────────────────────────

/// 索引管理器：管理所有索引
///
/// 用法：
/// ```rust
/// let mut idx_mgr = IndexManager::new();
///
/// // 创建索引
/// idx_mgr.create_index("idx_name", "name", IndexType::Hash, false)?;
///
/// // 重建索引（从现有数据）
/// idx_mgr.rebuild_all(&graph.vertices)?;
///
/// // 查询
/// let ids = idx_mgr.query_exact("name", &PropertyValue::String("Alice".to_string()));
/// ```
pub struct IndexManager {
    /// 索引名 -> 索引
    indexes: HashMap<String, HashIndex>,
    /// 属性键 -> 索引名（快速查找：某个属性有哪些索引）
    property_indexes: HashMap<String, Vec<String>>,
}

impl IndexManager {
    pub fn new() -> Self {
        IndexManager {
            indexes: HashMap::new(),
            property_indexes: HashMap::new(),
        }
    }

    // ── 索引管理 ─────────────────────────

    /// 创建索引
    ///
    /// - name: 索引名称（唯一）
    /// - property_key: 属性键（如 "name", "age"）
    /// - index_type: 索引类型（目前只支持 Hash）
    /// - unique: 是否唯一索引
    pub fn create_index(
        &mut self,
        name: &str,
        property_key: &str,
        index_type: IndexType,
        unique: bool,
    ) -> Result<(), String> {
        if self.indexes.contains_key(name) {
            return Err(format!("索引已存在: {}", name));
        }

        let def = IndexDef {
            name: name.to_string(),
            property_key: property_key.to_string(),
            index_type,
            unique,
        };

        let idx = HashIndex::new(def);
        self.indexes.insert(name.to_string(), idx);
        self.property_indexes
            .entry(property_key.to_string())
            .or_insert_with(Vec::new)
            .push(name.to_string());

        Ok(())
    }

    /// 删除索引
    pub fn drop_index(&mut self, name: &str) -> Result<(), String> {
        if let Some(idx) = self.indexes.remove(name) {
            // 从 property_indexes 中移除
            let property_key = idx.def.property_key;
            if let Some(names) = self.property_indexes.get_mut(&property_key) {
                names.retain(|n| n != name);
                if names.is_empty() {
                    self.property_indexes.remove(&property_key);
                }
            }
            Ok(())
        } else {
            Err(format!("索引不存在: {}", name))
        }
    }

    /// 列出所有索引
    pub fn list_indexes(&self) -> Vec<IndexDef> {
        self.indexes.values().map(|idx| idx.def.clone()).collect()
    }

    /// 检查某个属性是否有索引
    pub fn has_index(&self, property_key: &str) -> bool {
        self.property_indexes
            .get(property_key)
            .map(|names| !names.is_empty())
            .unwrap_or(false)
    }

    // ── 索引维护（自动调用）─────────────────────────

    /// 添加顶点时更新索引
    pub fn on_vertex_added(&mut self, vertex_id: u64, properties: &HashMap<String, PropertyValue>) {
        for (property_key, value) in properties {
            if let Some(index_names) = self.property_indexes.get(property_key) {
                for name in index_names {
                    if let Some(idx) = self.indexes.get_mut(name) {
                        idx.insert(vertex_id, value.clone());
                    }
                }
            }
        }
    }

    /// 删除顶点时更新索引
    pub fn on_vertex_deleted(&mut self, vertex_id: u64, properties: &HashMap<String, PropertyValue>) {
        for (property_key, _value) in properties {
            if let Some(index_names) = self.property_indexes.get(property_key) {
                for name in index_names {
                    if let Some(idx) = self.indexes.get_mut(name) {
                        idx.remove(vertex_id);
                    }
                }
            }
        }
    }

    /// 更新顶点属性时更新索引
    pub fn on_vertex_updated(
        &mut self,
        vertex_id: u64,
        old_properties: &HashMap<String, PropertyValue>,
        new_properties: &HashMap<String, PropertyValue>,
    ) {
        // 找出变更的属性
        let all_keys: HashSet<&String> = old_properties.keys()
            .chain(new_properties.keys())
            .collect();

        for key in all_keys {
            let old_val = old_properties.get(key);
            let new_val = new_properties.get(key);

            if old_val != new_val {
                // 属性值变了，需要更新索引
                if let Some(index_names) = self.property_indexes.get(key) {
                    for name in index_names {
                        if let Some(idx) = self.indexes.get_mut(name) {
                            if let Some(new_v) = new_val {
                                idx.update(vertex_id, new_v.clone());
                            } else {
                                idx.remove(vertex_id);
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 查询 ─────────────────────────

    /// 精确匹配查询
    ///
    /// 返回匹配的顶点 ID 列表。
    /// 如果属性没有索引，返回 None（调用者需要全表扫描）。
    pub fn query_exact(&self, property_key: &str, value: &PropertyValue) -> Option<Vec<u64>> {
        let index_names = self.property_indexes.get(property_key)?;
        let name = index_names.first()?;
        let idx = self.indexes.get(name)?;
        Some(idx.query_exact(value))
    }

    /// IN 查询（多个值）
    pub fn query_in(&self, property_key: &str, values: &[PropertyValue]) -> Option<Vec<u64>> {
        let index_names = self.property_indexes.get(property_key)?;
        let name = index_names.first()?;
        let idx = self.indexes.get(name)?;
        Some(idx.query_in(values))
    }

    // ── 批量操作 ─────────────────────────

    /// 从现有数据重建所有索引
    pub fn rebuild_all(&mut self, vertices: &HashMap<u64, crate::persistence::VertexRecord>) {
        // 清空所有索引
        for idx in self.indexes.values_mut() {
            idx.map.clear();
            idx.reverse.clear();
        }

        // 重新插入
        for (&vertex_id, vertex) in vertices {
            self.on_vertex_added(vertex_id, &vertex.properties);
        }
    }

    /// 从现有数据重建指定属性的索引
    pub fn rebuild_property(
        &mut self,
        property_key: &str,
        vertices: &HashMap<u64, crate::persistence::VertexRecord>,
    ) {
        let index_names = match self.property_indexes.get(property_key) {
            Some(names) => names.clone(),
            None => return,
        };

        // 清空这些索引
        for name in &index_names {
            if let Some(idx) = self.indexes.get_mut(name) {
                idx.map.clear();
                idx.reverse.clear();
            }
        }

        // 重新插入
        for (&vertex_id, vertex) in vertices {
            if let Some(value) = vertex.properties.get(property_key) {
                for name in &index_names {
                    if let Some(idx) = self.indexes.get_mut(name) {
                        idx.insert(vertex_id, value.clone());
                    }
                }
            }
        }
    }
}

// ── 测试 ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_props(kvs: &[(&str, PropertyValue)]) -> HashMap<String, PropertyValue> {
        let mut m = HashMap::new();
        for (k, v) in kvs {
            m.insert(k.to_string(), v.clone());
        }
        m
    }

    #[test]
    fn test_hash_index_insert_query() {
        let mut idx = HashIndex::new(IndexDef {
            name: "idx_name".to_string(),
            property_key: "name".to_string(),
            index_type: IndexType::Hash,
            unique: false,
        });

        idx.insert(1, PropertyValue::String("Alice".to_string()));
        idx.insert(2, PropertyValue::String("Bob".to_string()));
        idx.insert(3, PropertyValue::String("Alice".to_string()));  // 重复值

        let alice_ids = idx.query_exact(&PropertyValue::String("Alice".to_string()));
        assert_eq!(alice_ids.len(), 2);
        assert!(alice_ids.contains(&1));
        assert!(alice_ids.contains(&3));

        let bob_ids = idx.query_exact(&PropertyValue::String("Bob".to_string()));
        assert_eq!(bob_ids, vec![2]);
    }

    #[test]
    fn test_hash_index_remove() {
        let mut idx = HashIndex::new(IndexDef {
            name: "idx_age".to_string(),
            property_key: "age".to_string(),
            index_type: IndexType::Hash,
            unique: false,
        });

        idx.insert(1, PropertyValue::Int(30));
        idx.insert(2, PropertyValue::Int(30));
        idx.insert(3, PropertyValue::Int(25));

        idx.remove(1);

        let ids = idx.query_exact(&PropertyValue::Int(30));
        assert_eq!(ids, vec![2]);  // 只有 2 了
    }

    #[test]
    fn test_index_manager_create_query() {
        let mut mgr = IndexManager::new();

        // 创建索引
        mgr.create_index("idx_name", "name", IndexType::Hash, false).unwrap();

        // 模拟添加顶点
        let props1 = make_props(&[("name", PropertyValue::String("Alice".to_string()))]);
        let props2 = make_props(&[("name", PropertyValue::String("Bob".to_string()))]);
        let props3 = make_props(&[("name", PropertyValue::String("Alice".to_string()))]);

        mgr.on_vertex_added(1, &props1);
        mgr.on_vertex_added(2, &props2);
        mgr.on_vertex_added(3, &props3);

        // 查询
        let result = mgr.query_exact("name", &PropertyValue::String("Alice".to_string()));
        assert!(result.is_some());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&1));
        assert!(ids.contains(&3));
    }

    #[test]
    fn test_index_manager_update() {
        let mut mgr = IndexManager::new();
        mgr.create_index("idx_age", "age", IndexType::Hash, false).unwrap();

        let old_props = make_props(&[("age", PropertyValue::Int(30))]);
        let new_props = make_props(&[("age", PropertyValue::Int(31))]);

        mgr.on_vertex_added(1, &old_props);

        // 更新
        mgr.on_vertex_updated(1, &old_props, &new_props);

        // 旧值查不到
        let old_result = mgr.query_exact("age", &PropertyValue::Int(30));
        assert!(old_result.is_some());
        assert!(old_result.unwrap().is_empty());

        // 新值能查到
        let new_result = mgr.query_exact("age", &PropertyValue::Int(31));
        assert!(new_result.is_some());
        assert_eq!(new_result.unwrap(), vec![1]);
    }

    #[test]
    fn test_index_manager_rebuild() {
        let mut mgr = IndexManager::new();
        mgr.create_index("idx_name", "name", IndexType::Hash, false).unwrap();

        // 模拟现有数据
        let mut vertices = HashMap::new();
        vertices.insert(1, crate::persistence::VertexRecord {
            properties: make_props(&[("name", PropertyValue::String("Alice".to_string()))]),
        });
        vertices.insert(2, crate::persistence::VertexRecord {
            properties: make_props(&[("name", PropertyValue::String("Bob".to_string()))]),
        });

        // 重建索引
        mgr.rebuild_all(&vertices);

        // 查询
        let result = mgr.query_exact("name", &PropertyValue::String("Alice".to_string()));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vec![1]);
    }

    #[test]
    fn test_no_index_fallback() {
        let mgr = IndexManager::new();

        // 没有索引，query_exact 返回 None
        let result = mgr.query_exact("name", &PropertyValue::String("Alice".to_string()));
        assert!(result.is_none());
    }
}
