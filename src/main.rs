// src/main.rs
// Axolotl-RS: 命令行界面

use axolotl_rs::*;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    println!("🦄 Axolotl-RS: High-Performance Graph Database");
    println!("{}", "=".repeat(60));

    // 创建图数据库
    let mut db = GraphDB::new();

    // 添加顶点（社交网络）
    println!("\n📊 Creating social network graph...");
    
    let vertices = vec![
        ("Alice", "Data Scientist", 28),
        ("Bob", "Software Engineer", 32),
        ("Charlie", "Product Manager", 30),
        ("Diana", "Data Scientist", 27),
        ("Eve", "Software Engineer", 29),
    ];

    let mut name_to_id: HashMap<&str, VertexId> = HashMap::new();

    for (name, job, age) in vertices {
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String(name.to_string()));
        props.insert("job".to_string(), PropertyValue::String(job.to_string()));
        props.insert("age".to_string(), PropertyValue::Int(age));

        let vertex_id = db.add_vertex(props).unwrap();
        name_to_id.insert(name, vertex_id);
    }

    // 添加边（朋友关系）
    let edges = vec![
        ("Alice", "Bob", 0.9),
        ("Alice", "Charlie", 0.8),
        ("Bob", "Charlie", 0.7),
        ("Diana", "Alice", 0.95),
        ("Eve", "Bob", 0.85),
    ];

    for (from_name, to_name, weight) in edges {
        let from_id = name_to_id[from_name];
        let to_id = name_to_id[to_name];
        db.add_edge(from_id, to_id, HashMap::new(), weight)
            .unwrap();
    }

    println!("✅ Graph created!");
    println!("   Vertices: {}", db.vertex_count());
    println!("   Edges: {}", db.edge_count());

    // 运行图算法
    println!("\n🧮 Running graph algorithms...");

    // 最短路径
    let alice_id = name_to_id["Alice"];
    let eve_id = name_to_id["Eve"];
    let path = db.shortest_path(alice_id, eve_id);
    println!("\n   📏 Shortest path (Alice -> Eve):");
    for vertex_id in &path {
        if let Some(vertex) = db.get_vertex(*vertex_id) {
            if let Some(PropertyValue::String(name)) = vertex.properties.get("name") {
                print!("{} -> ", name);
            }
        }
    }
    println!("Done");

    // PageRank
    println!("\n   📈 PageRank (top 3):");
    let pr = db.pagerank(0.85, 100, 1e-6);
    let mut pr_vec: Vec<_> = pr.iter().collect();
    pr_vec.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (vertex_id, score) in pr_vec.iter().take(3) {
        if let Some(vertex) = db.get_vertex(**vertex_id) {
            if let Some(PropertyValue::String(name)) = vertex.properties.get("name") {
                println!("      {}: {:.4}", name, score);
            }
        }
    }

    // Betweenness Centrality
    println!("\n   🌉 Betweenness Centrality (top 3):");
    let betweenness = db.betweenness_centrality();
    let mut bt_vec: Vec<_> = betweenness.iter().collect();
    bt_vec.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (vertex_id, score) in bt_vec.iter().take(3) {
        if let Some(vertex) = db.get_vertex(**vertex_id) {
            if let Some(PropertyValue::String(name)) = vertex.properties.get("name") {
                println!("      {}: {:.4}", name, score);
            }
        }
    }

    // 连通分量
    println!("\n   🔗 Connected Components:");
    let components = db.connected_components();
    println!("      Number of components: {}", components.len());

    // 导出可视化
    println!("\n🎨 Exporting visualization...");

    // 导出 DOT
    let dot_path = PathBuf::from("/tmp/axolotl_rs_social.dot");
    db.export_dot(&dot_path, false).unwrap();
    println!("   ✅ DOT file exported: {:?}", dot_path);

    // 导出 HTML
    let html_path = PathBuf::from("/tmp/axolotl_rs_social.html");
    db.export_html(&html_path).unwrap();
    println!("   ✅ HTML file exported: {:?}", html_path);

    // 保存 JSON
    println!("\n💾 Saving to JSON...");
    let json_path = PathBuf::from("/tmp/axolotl_rs_social.json");
    db.save_json(&json_path).unwrap();
    println!("   ✅ JSON saved: {:?}", json_path);

    println!("\n{}", "=".repeat(60));
    println!("✨ Demo completed!");
    println!("\n💡 Tips:");
    println!("   - Open HTML file in browser for interactive visualization");
    println!("   - Use Graphviz to render DOT: dot -Tpng input.dot -o output.png");
    println!("   - Run benchmarks: cargo bench");
}
