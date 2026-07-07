// examples/server.rs
// Axolotl GraphDB REST API 服务器启动入口
//
// 用法: cargo run --example server [-- --data ./my-graph.axeb]
//
// 启动时自动从文件加载（含 WAL 恢复），不存在则创建空库。
// 停止时按 Ctrl+D 或发送空行 → 自动保存到文件。

use axolotl_rs::graph_db::{GraphDB, GraphMode};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data_file = if args.len() > 2 && args[1] == "--data" {
        args[2].clone()
    } else {
        "data/graph.axeb".to_string()
    };

    // 确保数据目录存在
    std::fs::create_dir_all(
        std::path::Path::new(&data_file).parent().unwrap_or(std::path::Path::new("."))
    ).ok();

    println!("Axolotl GraphDB Server");
    println!("  Data file: {}", data_file);

    let db = GraphDB::from_file_or_new(&data_file).expect("Failed to initialize database");

    println!("  Vertices: {}, Edges: {}", db.vertex_count(), db.edge_count());
    println!("  POST /admin/save     手动保存");
    println!("  POST /admin/shutdown  安全关闭（自动保存后退出）");
    println!();

    axolotl_rs::server::serve("0.0.0.0:8080", db);
}
