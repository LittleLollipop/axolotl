// examples/social_network.rs
// Axolotl-RS: 社交网络图示例

use axolotl_rs::*;

fn main() {
    println!("🦀 Axolotl-RS: 高性能图数据库演示");
    println!("=====================================\n");
    
    // 创建图数据库
    let mut graph = GraphDB::new();
    
    // 添加顶点（人物）
    let vertices = vec![
        (0, "Alice", "Engineer"),
        (1, "Bob", "Designer"),
        (2, "Charlie", "Manager"),
        (3, "Diana", "Engineer"),
        (4, "Eve", "Designer"),
    ];
    
    for (id, name, job) in vertices {
        let mut properties = std::collections::HashMap::new();
        properties.insert("name".to_string(), PropertyValue::String(name.to_string()));
        properties.insert("job".to_string(), PropertyValue::String(job.to_string()));
        graph.add_vertex_with_id(id, properties).unwrap();
    }
    
    // 添加边（朋友关系）
    let edges = vec![
        (0, 1, 0.8), // Alice -> Bob (亲密度 0.8)
        (0, 2, 0.6), // Alice -> Charlie
        (1, 2, 0.9), // Bob -> Charlie
        (1, 3, 0.7), // Bob -> Diana
        (2, 3, 0.8), // Charlie -> Diana
        (2, 4, 0.6), // Charlie -> Eve
        (3, 4, 0.9), // Diana -> Eve
    ];
    
    for (from, to, weight) in edges {
        let properties = std::collections::HashMap::new();
        graph.add_edge(from, to, properties, weight).unwrap();
    }
    
    println!("✅ 图创建成功");
    println!("   顶点数: {}", graph.vertex_count());
    println!("   边数: {}", graph.edge_count());
    
    // 测试最短路径
    println!("\n📊 最短路径 (Alice -> Eve):");
    let path = graph.shortest_path(0, 4);
    println!("   路径: {:?}", path);
    
    // 测试 PageRank
    println!("\n📊 PageRank (Top 3):");
    let pagerank = graph.pagerank(0.85, 100, 1e-6);
    let mut pr_sorted: Vec<_> = pagerank.iter().collect();
    pr_sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (id, score) in pr_sorted.iter().take(3) {
        let name = graph.get_vertex(**id)
            .and_then(|v| v.properties.get("name"))
            .map(|p| match p {
                PropertyValue::String(s) => s.clone(),
                _ => "Unknown".to_string(),
            })
            .unwrap_or_else(|| "Unknown".to_string());
        println!("   {} (ID: {}): {:.4}", name, id, score);
    }
    
    // 测试 Betweenness Centrality
    println!("\n📊 Betweenness Centrality (Top 3):");
    let betweenness = graph.betweenness_centrality();
    let mut bc_sorted: Vec<_> = betweenness.iter().collect();
    bc_sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (id, score) in bc_sorted.iter().take(3) {
        let name = graph.get_vertex(**id)
            .and_then(|v| v.properties.get("name"))
            .map(|p| match p {
                PropertyValue::String(s) => s.clone(),
                _ => "Unknown".to_string(),
            })
            .unwrap_or_else(|| "Unknown".to_string());
        println!("   {} (ID: {}): {:.4}", name, id, score);
    }
    
    // 测试连通分量
    println!("\n📊 连通分量:");
    let components = graph.connected_components();
    println!("   检测到 {} 个连通分量", components.len());
    for (i, component) in components.iter().enumerate() {
        println!("   分量 {}: {:?}", i + 1, component);
    }
    
    // 测试社区检测
    println!("\n📊 社区检测 (贪心算法):");
    let communities = graph.detect_communities_greedy();
    println!("   检测到 {} 个社区", communities.len());
    
    println!("\n✅ 演示完成！");
}
