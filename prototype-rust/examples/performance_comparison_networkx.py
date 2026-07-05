#!/usr/bin/env python3
"""
NetworkX 性能测试
与 Axolotl (Rust + GPU) 对比
"""

import time
import random
import networkx as nx
import sys

def load_edgelist(filename):
    """加载边列表文件"""
    edges = []
    max_vertex = 0
    
    with open(filename, 'r') as f:
        for line in f:
            parts = line.strip().split()
            if len(parts) >= 2:
                u = int(parts[0])
                v = int(parts[1])
                edges.append((u, v))
                max_vertex = max(max_vertex, u, v)
    
    vertex_count = max_vertex + 1
    return edges, vertex_count

def benchmark_pagerank(G, iterations=20):
    """测试 PageRank 性能"""
    print("\n--- PageRank (NetworkX) ---")
    
    # NetworkX 的 pagerank 不支持单次迭代，只能测试完整计算
    start = time.time()
    pr = nx.pagerank(G, max_iter=100)
    elapsed = time.time() - start
    
    # 计算 PR 值之和
    pr_sum = sum(pr.values())
    
    print(f"  迭代次数: 100 (NetworkX 默认)")
    print(f"  总时间: {elapsed:.4f}s")
    print(f"  PR 值之和: {pr_sum:.10f}")
    print(f"  正确性: {'✓' if abs(pr_sum - 1.0) < 1e-6 else '✗'}")

def benchmark_bfs(G, iterations=10):
    """测试 BFS 性能"""
    print("\n--- BFS (NetworkX) ---")
    
    source = 0
    start = time.time()
    for _ in range(iterations):
        # 从 source 开始 BFS
        lengths = nx.single_source_shortest_path_length(G, source)
        _ = dict(lengths)
    elapsed = time.time() - start
    
    print(f"  源点: {source}")
    print(f"  迭代次数: {iterations}")
    print(f"  总时间: {elapsed:.4f}s")
    print(f"  平均每次: {elapsed * 1000 / iterations:.4f}ms")

def main():
    print("=== NetworkX 性能测试 ===\n")
    
    # 测试数据集
    datasets = [
        ("dataset_1k_10k.edgelist", 1000, 10000),
        ("dataset_10k_100k.edgelist", 10000, 100000),
    ]
    
    for filename, expected_vertices, expected_edges in datasets:
        print(f"\n{'='*80}")
        print(f"数据集: {filename}")
        print(f"预期: {expected_vertices} 顶点, {expected_edges} 边")
        print(f"{'='*80}")
        
        # 加载图数据
        edges, vertex_count = load_edgelist(f"examples/{filename}")
        print(f"实际加载: {vertex_count} 顶点, {len(edges)} 边")
        
        # 构建 NetworkX 图
        G = nx.DiGraph()
        G.add_nodes_from(range(vertex_count))
        G.add_edges_from(edges)
        
        # 测试 PageRank
        benchmark_pagerank(G)
        
        # 测试 BFS
        benchmark_bfs(G)

if __name__ == '__main__':
    # 检查依赖
    try:
        import networkx
        print(f"✓ NetworkX 已安装 (版本: {networkx.__version__})")
    except ImportError:
        print("✗ NetworkX 未安装，请运行: pip install networkx")
        sys.exit(1)
    
    main()
