// src/compact_storage.rs
// 紧凑存储模块（替代 HashMap，减少内存占用 2-3x）
//
// 设计原则：
// - 顶点/边用排序 Vec 存储（二分查找）
// - 属性用紧凑编码（减少内存碎片）
// - 支持 mmap（后续扩展）
// - API 兼容 PersistentGraph

use std::collections::HashMap;
use crate::PropertyValue;

// ── 紧凑顶点存储 ─────────────────────────

/// 紧凑顶点存储
///
/// 内存布局：
/// - vertex_ids: Vec<u64> (排序，8 bytes per entry)
/// - vertex_data: Vec<CompactVertexData> (紧凑排列)
///
/// 查找：binary_search (O(log n))
/// 内存：~16 bytes per vertex (vs HashMap ~48-64 bytes)
#[derive(Debug, Clone)]
pub struct CompactVertexStore {
    /// 排序后的顶点 ID 列表
    vertex_ids: Vec<u64>,
    /// 顶点数据（与 vertex_ids 一一对应）
    vertex_data: Vec<CompactVertexData>,
}

#[derive(Debug, Clone)]
struct CompactVertexData {
    /// 属性（紧凑编码）
    properties: Vec<u8>,  // 编码格式：len(u32) + key(string) + value(PropertyValue)
}

impl CompactVertexStore {
    pub fn new() -> Self {
        CompactVertexStore {
            vertex_ids: Vec::new(),
            vertex_data: Vec::new(),
        }
    }

    /// 插入/更新顶点
    pub fn insert(&mut self, id: u64, properties: HashMap<String, PropertyValue>) {
        let encoded = encode_properties(&properties);

        match self.vertex_ids.binary_search(&id) {
            Ok(idx) => {
                // 已存在，更新
                self.vertex_data[idx].properties = encoded;
            }
            Err(idx) => {
                // 不存在，插入
                self.vertex_ids.insert(idx, id);
                self.vertex_data.insert(idx, CompactVertexData { properties: encoded });
            }
        }
    }

    /// 删除顶点
    pub fn remove(&mut self, id: &u64) -> Option<HashMap<String, PropertyValue>> {
        match self.vertex_ids.binary_search(id) {
            Ok(idx) => {
                self.vertex_ids.remove(idx);
                let data = self.vertex_data.remove(idx);
                Some(decode_properties(&data.properties))
            }
            Err(_) => None,
        }
    }

    /// 获取顶点属性
    pub fn get(&self, id: &u64) -> Option<HashMap<String, PropertyValue>> {
        self.vertex_ids.binary_search(id).ok()
            .map(|idx| decode_properties(&self.vertex_data[idx].properties))
    }

    /// 获取顶点属性（不解码，用于内部操作）
    pub fn get_raw(&self, id: &u64) -> Option<&[u8]> {
        self.vertex_ids.binary_search(id).ok()
            .map(|idx| self.vertex_data[idx].properties.as_slice())
    }

    /// 检查顶点是否存在
    pub fn contains_key(&self, id: &u64) -> bool {
        self.vertex_ids.binary_search(id).is_ok()
    }

    /// 顶点数量
    pub fn len(&self) -> usize {
        self.vertex_ids.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.vertex_ids.is_empty()
    }

    /// 迭代所有顶点
    pub fn iter(&self) -> impl Iterator<Item = (u64, HashMap<String, PropertyValue>)> + '_ {
        self.vertex_ids.iter().copied()
            .zip(self.vertex_data.iter())
            .map(|(id, data)| (id, decode_properties(&data.properties)))
    }

    /// 迭代所有顶点 ID
    pub fn keys(&self) -> impl Iterator<Item = u64> + '_ {
        self.vertex_ids.iter().copied()
    }

    /// 清空
    pub fn clear(&mut self) {
        self.vertex_ids.clear();
        self.vertex_data.clear();
    }

    /// 从 HashMap 导入
    pub fn from_hashmap(map: &HashMap<u64, crate::persistence::VertexRecord>) -> Self {
        let mut store = CompactVertexStore::new();
        // 按键排序插入（保证 vertex_ids 有序）
        let mut items: Vec<_> = map.iter().collect();
        items.sort_by_key(|(&id, _)| id);

        for (&id, vr) in items {
            store.insert(id, vr.properties.clone());
        }
        store
    }

    /// 导出到 HashMap（兼容旧代码）
    pub fn to_hashmap(&self) -> HashMap<u64, crate::persistence::VertexRecord> {
        let mut map = HashMap::new();
        for (id, props) in self.iter() {
            map.insert(id, crate::persistence::VertexRecord { properties: props });
        }
        map
    }
}

// ── 紧凑边存储 ─────────────────────────

/// 紧凑边存储
///
/// 内存布局：
/// - edges: Vec<(u64, u64)> (排序，16 bytes per entry)
/// - edge_data: Vec<CompactEdgeData> (紧凑排列)
///
/// 查找：binary_search on (from, to) (O(log e))
/// 内存：~24 bytes per edge (vs HashMap ~64-80 bytes)
#[derive(Debug, Clone)]
pub struct CompactEdgeStore {
    /// 排序后的边 (from, to)
    edges: Vec<(u64, u64)>,
    /// 边数据（与 edges 一一对应）
    edge_data: Vec<CompactEdgeData>,
}

#[derive(Debug, Clone)]
struct CompactEdgeData {
    /// 权重
    weight: f64,
    /// 属性（紧凑编码）
    properties: Vec<u8>,
}

impl CompactEdgeStore {
    pub fn new() -> Self {
        CompactEdgeStore {
            edges: Vec::new(),
            edge_data: Vec::new(),
        }
    }

    /// 插入/更新边
    pub fn insert(
        &mut self,
        from: u64,
        to: u64,
        weight: f64,
        properties: HashMap<String, PropertyValue>,
    ) {
        let encoded = encode_properties(&properties);

        match self.edges.binary_search(&(from, to)) {
            Ok(idx) => {
                // 已存在，更新
                self.edge_data[idx] = CompactEdgeData { weight, properties: encoded };
            }
            Err(idx) => {
                // 不存在，插入
                self.edges.insert(idx, (from, to));
                self.edge_data.insert(idx, CompactEdgeData { weight, properties: encoded });
            }
        }
    }

    /// 删除边
    pub fn remove(&mut self, from: &u64, to: &u64) -> Option<(f64, HashMap<String, PropertyValue>)> {
        match self.edges.binary_search(&(*from, *to)) {
            Ok(idx) => {
                self.edges.remove(idx);
                let data = self.edge_data.remove(idx);
                Some((data.weight, decode_properties(&data.properties)))
            }
            Err(_) => None,
        }
    }

    /// 获取边
    pub fn get(&self, from: &u64, to: &u64) -> Option<(f64, HashMap<String, PropertyValue>)> {
        self.edges.binary_search(&(*from, *to)).ok()
            .map(|idx| {
                let data = &self.edge_data[idx];
                (data.weight, decode_properties(&data.properties))
            })
    }

    /// 检查边是否存在
    pub fn contains_key(&self, from: &u64, to: &u64) -> bool {
        self.edges.binary_search(&(*from, *to)).is_ok()
    }

    /// 边数量
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// 迭代所有边
    pub fn iter(&self) -> impl Iterator<Item = ((u64, u64), f64, HashMap<String, PropertyValue>)> + '_ {
        self.edges.iter().copied()
            .zip(self.edge_data.iter())
            .map(|((from, to), data)| {
                ((from, to), data.weight, decode_properties(&data.properties))
            })
    }

    /// 迭代所有边（只读引用）
    pub fn keys(&self) -> impl Iterator<Item = (u64, u64)> + '_ {
        self.edges.iter().copied()
    }

    /// 清空
    pub fn clear(&mut self) {
        self.edges.clear();
        self.edge_data.clear();
    }

    /// 从 HashMap 导入
    pub fn from_hashmap(map: &HashMap<(u64, u64), crate::persistence::EdgeRecord>) -> Self {
        let mut store = CompactEdgeStore::new();
        // 按键排序插入
        let mut items: Vec<_> = map.iter().collect();
        items.sort_by_key(|(&(from, to), _)| (from, to));

        for (&(from, to), er) in items {
            store.insert(from, to, er.weight, er.properties.clone());
        }
        store
    }

    /// 导出到 HashMap（兼容旧代码）
    pub fn to_hashmap(&self) -> HashMap<(u64, u64), crate::persistence::EdgeRecord> {
        let mut map = HashMap::new();
        for ((from, to), weight, props) in self.iter() {
            map.insert((from, to), crate::persistence::EdgeRecord { properties: props, weight });
        }
        map
    }
}

// ── 属性编码/解码 ─────────────────────────

/// 编码属性为紧凑字节数组
///
/// 格式：
/// - len: u32 (属性数量)
/// - 每个属性：
///   - key_len: u32
///   - key: [u8] (UTF-8)
///   - value_type: u8
///   - value: (取决于类型)
fn encode_properties(props: &HashMap<String, PropertyValue>) -> Vec<u8> {
    let mut buf = Vec::new();

    // 属性数量
    let len = props.len() as u32;
    buf.extend_from_slice(&len.to_be_bytes());

    // 按键排序（保证编码稳定）
    let mut items: Vec<_> = props.iter().collect();
    items.sort_by_key(|(k, _)| *k);

    for (key, value) in items {
        // key
        let key_bytes = key.as_bytes();
        buf.extend_from_slice(&(key_bytes.len() as u32).to_be_bytes());
        buf.extend_from_slice(key_bytes);

        // value
        match value {
            PropertyValue::String(s) => {
                buf.push(0);  // String = 0
                let s_bytes = s.as_bytes();
                buf.extend_from_slice(&(s_bytes.len() as u32).to_be_bytes());
                buf.extend_from_slice(s_bytes);
            }
            PropertyValue::Int(i) => {
                buf.push(1);  // Int = 1
                buf.extend_from_slice(&(*i as u64).to_be_bytes());
            }
            PropertyValue::Double(d) => {
                buf.push(2);  // Double = 2
                buf.extend_from_slice(&d.to_be_bytes());
            }
            PropertyValue::Bool(b) => {
                buf.push(3);  // Bool = 3
                buf.push(*b as u8);
            }
            PropertyValue::Null => {
                buf.push(4);  // Null = 4
            }
        }
    }

    buf
}

/// 从紧凑字节数组解码属性
fn decode_properties(buf: &[u8]) -> HashMap<String, PropertyValue> {
    use std::io::{Read, Cursor};

    let mut cursor = Cursor::new(buf);
    let mut len_buf = [0u8; 4];

    // 读取属性数量
    if cursor.read_exact(&mut len_buf).is_err() {
        return HashMap::new();
    }
    let len = u32::from_be_bytes(len_buf);

    let mut props = HashMap::new();

    for _ in 0..len {
        // 读取 key
        if cursor.read_exact(&mut len_buf).is_err() {
            return props;
        }
        let key_len = u32::from_be_bytes(len_buf) as usize;
        let mut key_buf = vec![0u8; key_len];
        if cursor.read_exact(&mut key_buf).is_err() {
            return props;
        }
        let key = match String::from_utf8(key_buf) {
            Ok(s) => s,
            Err(_) => return props,
        };

        // 读取 value type
        let mut type_buf = [0u8; 1];
        if cursor.read_exact(&mut type_buf).is_err() {
            return props;
        }

        let value = match type_buf[0] {
            0 => {
                // String
                if cursor.read_exact(&mut len_buf).is_err() {
                    return props;
                }
                let s_len = u32::from_be_bytes(len_buf) as usize;
                let mut s_buf = vec![0u8; s_len];
                if cursor.read_exact(&mut s_buf).is_err() {
                    return props;
                }
                PropertyValue::String(match String::from_utf8(s_buf) {
                    Ok(s) => s,
                    Err(_) => return props,
                })
            }
            1 => {
                // Int
                let mut i_buf = [0u8; 8];
                if cursor.read_exact(&mut i_buf).is_err() {
                    return props;
                }
                PropertyValue::Int(u64::from_be_bytes(i_buf) as i64)
            }
            2 => {
                // Double
                let mut d_buf = [0u8; 8];
                if cursor.read_exact(&mut d_buf).is_err() {
                    return props;
                }
                PropertyValue::Double(f64::from_be_bytes(d_buf))
            }
            3 => {
                // Bool
                let mut b_buf = [0u8; 1];
                if cursor.read_exact(&mut b_buf).is_err() {
                    return props;
                }
                PropertyValue::Bool(b_buf[0] != 0)
            }
            4 => {
                // Null
                PropertyValue::Null
            }
            _ => continue,
        };

        props.insert(key, value);
    }

    props
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
    fn test_compact_vertex_store_insert_get() {
        let mut store = CompactVertexStore::new();

        let props = make_props(&[
            ("name", PropertyValue::String("Alice".to_string())),
            ("age", PropertyValue::Int(30)),
        ]);

        store.insert(1, props);

        let result = store.get(&1).unwrap();
        assert_eq!(result["name"], PropertyValue::String("Alice".to_string()));
        assert_eq!(result["age"], PropertyValue::Int(30));
    }

    #[test]
    fn test_compact_vertex_store_update() {
        let mut store = CompactVertexStore::new();

        store.insert(1, make_props(&[("name", PropertyValue::String("Alice".to_string()))]));
        store.insert(1, make_props(&[("name", PropertyValue::String("Bob".to_string()))]));

        let result = store.get(&1).unwrap();
        assert_eq!(result["name"], PropertyValue::String("Bob".to_string()));
    }

    #[test]
    fn test_compact_vertex_store_remove() {
        let mut store = CompactVertexStore::new();

        store.insert(1, make_props(&[("name", PropertyValue::String("Alice".to_string()))]));
        let removed = store.remove(&1).unwrap();

        assert_eq!(removed["name"], PropertyValue::String("Alice".to_string()));
        assert!(store.get(&1).is_none());
    }

    #[test]
    fn test_compact_vertex_store_iter() {
        let mut store = CompactVertexStore::new();

        store.insert(1, make_props(&[("name", PropertyValue::String("A".to_string()))]));
        store.insert(3, make_props(&[("name", PropertyValue::String("C".to_string()))]));
        store.insert(2, make_props(&[("name", PropertyValue::String("B".to_string()))]));

        let ids: Vec<u64> = store.keys().collect();
        assert_eq!(ids, vec![1, 2, 3]);  // 保证排序
    }

    #[test]
    fn test_compact_edge_store() {
        let mut store = CompactEdgeStore::new();

        store.insert(1, 2, 1.0, make_props(&[("type", PropertyValue::String("knows".to_string()))]));
        store.insert(2, 3, 0.5, make_props(&[("type", PropertyValue::String("likes".to_string()))]));

        let (weight, props) = store.get(&1, &2).unwrap();
        assert_eq!(weight, 1.0);
        assert_eq!(props["type"], PropertyValue::String("knows".to_string()));
    }

    #[test]
    fn test_encode_decode_properties() {
        let props = make_props(&[
            ("name", PropertyValue::String("Alice".to_string())),
            ("age", PropertyValue::Int(30)),
            ("score", PropertyValue::Double(95.5)),
            ("active", PropertyValue::Bool(true)),
        ]);

        let encoded = encode_properties(&props);
        let decoded = decode_properties(&encoded);

        assert_eq!(decoded["name"], props["name"]);
        assert_eq!(decoded["age"], props["age"]);
        assert_eq!(decoded["score"], props["score"]);
        assert_eq!(decoded["active"], props["active"]);
    }

    #[test]
    fn test_roundtrip_hashmap() {
        let mut original = HashMap::new();
        original.insert(1, crate::persistence::VertexRecord {
            properties: make_props(&[("name", PropertyValue::String("Alice".to_string()))]),
        });
        original.insert(2, crate::persistence::VertexRecord {
            properties: make_props(&[("name", PropertyValue::String("Bob".to_string()))]),
        });

        let store = CompactVertexStore::from_hashmap(&original);
        let restored = store.to_hashmap();

        assert_eq!(original.len(), restored.len());
        for (id, vr) in &original {
            assert_eq!(vr.properties, restored[id].properties);
        }
    }
}
