// benches/graph_benchmark.rs
// Axolotl-RS: 性能基准测试（对比 Swift 版本和 igraph）

use axolotl_rs::*;
use criterion::{black_box, criterionGroup, criterionMain, Criterion};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// 生成随机图数据
fn generate_random_graph(
    db: &mut GraphDB,
    vertex_count: usize,
    edge_count: usize,
) {
    // 添加顶点
    for i in 0..vertex_count {
        let mut props = HashMap::new();
        props.insert(
            "name".to_string(),
            PropertyValue::String(format!("Vertex_{}", i)),
        );
        db.add_vertex(props).unwrap();
    }

    // 添加边
    let mut rng = rand::thread_rng();
    let mut edge_set = HashSet::new();

    while edge_set.len() < edge_count {
        let from = rng.gen_range(0..vertex_count) as u64;
        let to = rng.gen_range(0..vertex_count) as u64;
        if from == to {
            continue;
        }

        let key = (from.min(to), from.max(to));
        if !edge_set.contains(&key) {
            edge_set.insert(key);
            db.add_edge(from, to, HashMap::new(), 1.0)
                .unwrap();
        }
    }
}

/// 基准测试：添加顶点
fn bench_add_vertex(c: &mut Criterion) {
    let mut db = GraphDB::new();

    c.bench_function("add_vertex_100", |b| {
        b.iter(|| {
            let mut props = HashMap::new();
            props.insert(
                "name".to_string(),
                PropertyValue::String("test".to_string()),
            );
            black_box(db.add_vertex(props).unwrap());
        })
    });
}

/// 基准测试：邻居查询
fn bench_neighbor_query(c: &mut Criterion) {
    let mut db = GraphDB::new();
    generate_random_graph(&mut db, 1000, 5000);

    let sample_vertex = 0;

    c.bench_function("neighbor_query", |b| {
        b.iter(|| {
            black_box(db.get_neighbors(sample_vertex));
        })
    });
}

/// 基准测试：最短路径
fn bench_shortest_path(c: &mut Criterion) {
    let mut db = GraphDB::new();
    generate_random_graph(&mut db, 1000, 5000);

    c.bench_function("shortest_path", |b| {
        b.iter(|| {
            black_box(db.shortest_path(0, 999));
        })
    });
}

/// 基准测试：PageRank
fn bench_pagerank(c: &mut Criterion) {
    let mut db = GraphDB::new();
    generate_random_graph(&mut db, 1000, 5000);

    c.bench_function("pagerank", |b| {
        b.iter(|| {
            black_box(db.pagerank(0.85, 100, 1e-6));
        })
    });
}

/// 基准测试：Betweenness Centrality（小图）
fn bench_betweenness(c: &mut Criterion) {
    let mut db = GraphDB::new();
    generate_random_graph(&mut db, 100, 500);

    c.bench_function("betweenness_centrality", |b| {
        b.iter(|| {
            black_box(db.betweenness_centrality());
        })
    });
}

/// 基准测试：连通分量
fn bench_connected_components(c: &mut Criterion) {
    let mut db = GraphDB::new();
    generate_random_graph(&mut db, 1000, 5000);

    c.bench_function("connected_components", |b| {
        b.iter(|| {
            black_box(db.connected_components());
        })
    });
}

/// 综合基准测试
fn bench_composite(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_operations");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    // 小型图
    group.bench_function("small_graph_build", |b| {
        b.iter(|| {
            let mut db = GraphDB::new();
            generate_random_graph(&mut db, 100, 500);
            black_box(db);
        })
    });

    // 中型图
    group.bench_function("medium_graph_build", |b| {
        b.iter(|| {
            let mut db = GraphDB::new();
            generate_random_graph(&mut db, 1000, 5000);
            black_box(db);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_add_vertex,
    bench_neighbor_query,
    bench_shortest_path,
    bench_pagerank,
    bench_betweenness,
    bench_connected_components,
    bench_composite
);

criterion_main!(benches);
