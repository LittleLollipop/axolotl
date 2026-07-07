// examples/server.rs
// Axolotl GraphDB REST API 服务器启动入口

use axolotl_rs::graph_db::{GraphDB, GraphMode};
use axolotl_rs::PropertyValue;
use std::collections::HashMap;

fn main() {
    println!("Initializing Axolotl GraphDB...");

    let mut db = GraphDB::new(GraphMode::InMemory);

    // 示例数据
    let mut alice = HashMap::new();
    alice.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
    alice.insert("age".to_string(), PropertyValue::Int(30));
    db.add_vertex(1, alice).unwrap();

    let mut bob = HashMap::new();
    bob.insert("name".to_string(), PropertyValue::String("Bob".to_string()));
    bob.insert("age".to_string(), PropertyValue::Int(28));
    db.add_vertex(2, bob).unwrap();

    let mut charlie = HashMap::new();
    charlie.insert("name".to_string(), PropertyValue::String("Charlie".to_string()));
    db.add_vertex(3, charlie).unwrap();

    db.add_edge(1, 2, 1.0, HashMap::new()).unwrap();
    db.add_edge(1, 3, 1.0, HashMap::new()).unwrap();
    db.add_edge(2, 3, 1.0, HashMap::new()).unwrap();

    println!("Loaded {} vertices, {} edges", db.vertex_count(), db.edge_count());
    axolotl_rs::server::serve("0.0.0.0:8080", db);
}
