// Axolotl Viewer — 图库查看器（只读）
// Tauri 2 后端：直接 link axolotl-rs 引擎，打开 .axeb 后一次性导出全图快照给前端。

use axolotl_rs::graph_db::{GraphDB, GraphMode};
use axolotl_rs::PropertyValue;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
pub struct NodeJson {
    pub id: u64,
    /// 属性里的字符串 id（如 lobster_root / ch001 / unit_xxx），缺省回退为数字 id
    pub sid: String,
    pub label: String,
    pub props: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Clone)]
pub struct EdgeJson {
    pub from: u64,
    pub to: u64,
    pub kind: String,
    pub weight: f64,
    pub props: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
pub struct GraphSnapshot {
    pub path: String,
    pub vertex_count: usize,
    pub edge_count: usize,
    pub vertices: Vec<NodeJson>,
    pub edges: Vec<EdgeJson>,
}

fn prop_to_json(pv: &PropertyValue) -> serde_json::Value {
    match pv {
        PropertyValue::Int(i) => serde_json::Value::from(*i),
        PropertyValue::String(s) => serde_json::Value::String(s.clone()),
        PropertyValue::Double(d) => serde_json::json!(d),
        PropertyValue::Bool(b) => serde_json::Value::from(*b),
        PropertyValue::Null => serde_json::Value::Null,
    }
}

fn props_to_json_map(props: &HashMap<String, PropertyValue>) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for (k, v) in props {
        map.insert(k.clone(), prop_to_json(v));
    }
    map
}

fn str_prop(props: &HashMap<String, PropertyValue>, key: &str) -> String {
    match props.get(key) {
        Some(PropertyValue::String(s)) => s.clone(),
        _ => String::new(),
    }
}

impl NodeJson {
    fn from_props(id: u64, props: &HashMap<String, PropertyValue>) -> Self {
        let mut sid = str_prop(props, "id");
        if sid.is_empty() {
            sid = id.to_string();
        }
        let mut label = str_prop(props, "label");
        if label.is_empty() {
            label = sid.clone();
        }
        NodeJson {
            id,
            sid,
            label,
            props: props_to_json_map(props),
        }
    }
}

impl EdgeJson {
    fn from_props(from: u64, to: u64, weight: f64, props: &HashMap<String, PropertyValue>) -> Self {
        let kind = str_prop(props, "kind");
        EdgeJson {
            from,
            to,
            kind,
            weight,
            props: props_to_json_map(props),
        }
    }
}

/// 打开 .axeb 图库并导出全量快照（只读，跳过 WAL 恢复，不触发任何写入/清理）
#[tauri::command]
fn open_graph(path: String) -> Result<GraphSnapshot, String> {
    let db = GraphDB::open_without_recovery(&path, GraphMode::InMemory)
        .map_err(|e| format!("打开图库失败: {:?}", e))?;

    let vertices: Vec<NodeJson> = db
        .iter_vertices()
        .map(|(vid, props)| NodeJson::from_props(vid, &props))
        .collect();

    let edges: Vec<EdgeJson> = db
        .iter_edges()
        .map(|(from, to, w, props)| EdgeJson::from_props(from, to, w, &props))
        .collect();

    let snapshot = GraphSnapshot {
        path,
        vertex_count: vertices.len(),
        edge_count: edges.len(),
        vertices,
        edges,
    };
    Ok(snapshot)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![open_graph])
        .run(tauri::generate_context!())
        .expect("error while running axolotl-viewer");
}
