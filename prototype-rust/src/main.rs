// src/main.rs
// Axolotl-RS: 命令行界面和演示

use axolotl_rs::*;
use std::collections::HashMap;

fn main() {
    println!("🦄 Axolotl-RS: High-Performance Graph Database");
    println!("{}", "=".repeat(60));

    // 创建图数据库
    let mut db = persistence::PersistentGraph::open("/tmp/axolotl_rs_demo.bin")
        .expect("Failed to open database");

    // 清空旧数据
    let vertex_ids: Vec<u64> = db.vertices.keys().copied().collect();
    for id in vertex_ids {
        db.delete_vertex(id);
    }

    println!("\n📊 Creating social network graph...");

    // 添加顶点
    let mut name_to_id: HashMap<&str, u64> = HashMap::new();

    let vertices = vec![
        ("Alice", "Data Scientist", 28),
        ("Bob", "Software Engineer", 32),
        ("Charlie", "Product Manager", 30),
        ("Diana", "Data Scientist", 27),
        ("Eve", "Software Engineer", 29),
    ];

    for (name, job, age) in vertices {
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String(name.to_string()));
        props.insert("job".to_string(), PropertyValue::String(job.to_string()));
        props.insert("age".to_string(), PropertyValue::Int(age));

        db.add_vertex(name_to_id.len() as u64 + 1, props);
        name_to_id.insert(name, name_to_id.len() as u64 + 1);
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
        db.add_edge(from_id, to_id, weight, HashMap::new());
    }

    println!("✅ Graph created!");
    println!("   Vertices: {}", db.vertices.len());
    println!("   Edges: {}", db.edges.len());

    // 保存
    db.save().expect("Failed to save");

    // 运行图算法
    println!("\n🧮 Running graph algorithms...");

    // 最短路径
    let alice_id = name_to_id["Alice"];
    let eve_id = name_to_id["Eve"];
    let path = db.shortest_path(alice_id, eve_id);
    println!("\n   📏 Shortest path (Alice -> Eve):");
    for (i, &vertex_id) in path.iter().enumerate() {
        if let Some(props) = db.find_vertex().with_id(vertex_id).execute_with_properties().first() {
            if let PropertyValue::String(name) = &props.1["name"] {
                if i < path.len() - 1 {
                    print!("{} -> ", name);
                } else {
                    println!("{}", name);
                }
            }
        }
    }

    // PageRank (CPU version)
    println!("\n   📈 PageRank (top 3):");
    let csr = db.to_csr();
    let pr = pagerank_correct::compute_pagerank_cpu(&csr, 100);
    let mut pr_vec: Vec<_> = pr.iter().enumerate().collect();
    pr_vec.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (i, (vertex_idx, score)) in pr_vec.iter().take(3).enumerate() {
        let vertex_id = csr.idx_to_vertex[*vertex_idx];
        if let Some(props) = db.find_vertex().with_id(vertex_id).execute_with_properties().first() {
            if let PropertyValue::String(name) = &props.1["name"] {
                println!("      {}: {:.4}", name, score);
            }
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("✨ Demo completed!");

    // 测试 mmap 格式转换
    println!("\n🗜️ Testing mmap format...");
    let mmap_path = "/tmp/axolotl_rs_demo.mmap";
    mmap_graph::MmapGraph::convert_from(&db, mmap_path)
        .expect("Failed to convert to mmap format");

    let mut mmap_g = mmap_graph::MmapGraph::open(mmap_path)
        .expect("Failed to open mmap graph");

    println!("   ✅ Converted to mmap format");
    println!("   Vertex count: {}", mmap_g.vertex_count());
    println!("   Edge count: {}", mmap_g.edge_count());

    // 验证 mmap 格式的数据
    if let Some(props) = mmap_g.get_vertex(1) {
        println!("   Alice props: {:?}", props);
    }

    let neighbors = mmap_g.out_neighbors(1);
    println!("   Alice's out-neighbors: {:?}", neighbors);

    // 清理
    let _ = std::fs::remove_file(mmap_path);

    println!("\n💡 Tips:");
    println!("   - Use GPU acceleration: `gpu::compute_full_pagerank()`");
    println!("   - Query API: `db.find_vertex().with_property(...)`");
    println!("   - Create index: `db.create_index(...)`");
    println!("   - Mmap format supports graphs larger than memory");
}
