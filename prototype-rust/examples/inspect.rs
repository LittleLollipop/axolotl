// examples/inspect.rs — 图库文件只读检查工具（验证/调试用）
// 用法: cargo run --example inspect -- <path.axeb> [--edges]
use axolotl_rs::graph_db::{GraphDB, GraphMode};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: cargo run --example inspect -- <path.axeb> [--edges]");
        std::process::exit(1);
    }
    let path = &args[1];
    let show_edges = args.iter().any(|a| a == "--edges");

    // 只读打开：跳过 WAL 恢复/清理，绝不写盘
    let db = GraphDB::open_without_recovery(path, GraphMode::InMemory)
        .map_err(|e| format!("打开失败: {:?}", e))
        .unwrap();

    let mut vertices = Vec::new();
    for (vid, props) in db.iter_vertices() {
        vertices.push((vid, props));
    }
    let mut edges = Vec::new();
    for (from, to, w, props) in db.iter_edges() {
        edges.push((from, to, w, props));
    }

    println!("=== {} ===", path);
    println!("顶点数: {}", vertices.len());
    println!("边数:   {}", edges.len());

    // 按 domain 统计
    let mut by_domain: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut by_status: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (_, props) in &vertices {
        let d = props.get("domain").map(|v| match v {
            axolotl_rs::PropertyValue::String(s) => s.as_str(),
            _ => "?",
        }).unwrap_or("(无)");
        let s = props.get("status").map(|v| match v {
            axolotl_rs::PropertyValue::String(s) => s.as_str(),
            _ => "?",
        }).unwrap_or("(无)");
        *by_domain.entry(d).or_insert(0) += 1;
        *by_status.entry(s).or_insert(0) += 1;
    }
    println!("--- domain 分布 ---");
    for (k, v) in &by_domain { println!("  {:12} {}", k, v); }
    println!("--- status 分布 ---");
    for (k, v) in &by_status { println!("  {:12} {}", k, v); }

    // 边 kind 统计（验证 iter_edges 补丁：应能拿到 kind 属性）
    let mut by_kind: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut no_kind = 0;
    for (_, _, _, props) in &edges {
        match props.get("kind") {
            Some(axolotl_rs::PropertyValue::String(s)) => *by_kind.entry(s.as_str()).or_insert(0) += 1,
            _ => no_kind += 1,
        }
    }
    println!("--- 边 kind 分布 ---");
    for (k, v) in &by_kind { println!("  {:12} {}", k, v); }
    if no_kind > 0 { println!("  (无 kind 属性: {})", no_kind); }

    // 采样打印前 8 个顶点
    println!("--- 前 8 个顶点 ---");
    for (vid, props) in vertices.iter().take(8) {
        let sid = props.get("id").map(|v| match v {
            axolotl_rs::PropertyValue::String(s) => s.clone(),
            _ => format!("{:?}", v),
        }).unwrap_or_default();
        let label = props.get("label").map(|v| match v {
            axolotl_rs::PropertyValue::String(s) => s.clone(),
            _ => format!("{:?}", v),
        }).unwrap_or_default();
        println!("  {:>3}  id={:<24} label={}", vid, sid, label);
    }

    if show_edges {
        println!("--- 前 12 条边 ---");
        for (f, t, w, props) in edges.iter().take(12) {
            let kind = props.get("kind").map(|v| match v {
                axolotl_rs::PropertyValue::String(s) => s.clone(),
                _ => format!("{:?}", v),
            }).unwrap_or_else(|| "(无)".to_string());
            println!("  {} -> {}  w={:.3}  kind={}", f, t, w, kind);
        }
    }
}
