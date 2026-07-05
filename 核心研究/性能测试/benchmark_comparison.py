#!/usr/bin/env python3
"""
Axolotl-RS vs igraph vs NetworkX 性能对比测试
测试算法：最短路径、PageRank、Betweenness Centrality
"""

import time
import random
import subprocess
import json
import sys

# ========== 图生成器 ==========
def generate_graph(n_vertices, n_edges, directed=False):
    """生成随机图（边列表格式）"""
    edges = set()
    while len(edges) < n_edges:
        u = random.randint(0, n_vertices - 1)
        v = random.randint(0, n_vertices - 1)
        if u != v and (u, v) not in edges:
            edges.add((u, v))
            if not directed:
                edges.add((v, u))
    
    return list(edges)

# ========== NetworkX 测试 ==========
def benchmark_networkx(edges, n_vertices, algorithms):
    """使用 NetworkX 运行算法"""
    import networkx as nx
    
    G = nx.DiGraph() if hasattr(edges[0], 'directed') else nx.Graph()
    G.add_nodes_from(range(n_vertices))
    G.add_edges_from(edges)
    
    results = {}
    
    if 'shortest_path' in algorithms:
        start = time.time()
        # 测试 100 对随机顶点的最短路径
        for _ in range(100):
            s = random.randint(0, n_vertices - 1)
            t = random.randint(0, n_vertices - 1)
            try:
                path = nx.shortest_path(G, source=s, target=t)
            except:
                pass
        results['shortest_path'] = time.time() - start
    
    if 'pagerank' in algorithms:
        start = time.time()
        pr = nx.pagerank(G, max_iter=100)
        results['pagerank'] = time.time() - start
    
    if 'betweenness' in algorithms:
        start = time.time()
        bc = nx.betweenness_centrality(G, k=int(n_vertices**0.5))
        results['betweenness'] = time.time() - start
    
    return results

# ========== igraph 测试 ==========
def benchmark_igraph(edges, n_vertices, algorithms):
    """使用 igraph 运行算法"""
    import igraph as ig
    
    # 转换边格式
    edge_list = [(u, v) for u, v in edges]
    
    G = ig.Graph(n_vertices, edge_list, directed=True)
    
    results = {}
    
    if 'shortest_path' in algorithms:
        start = time.time()
        # 测试 100 对随机顶点的最短路径
        for _ in range(100):
            s = random.randint(0, n_vertices - 1)
            t = random.randint(0, n_vertices - 1)
            try:
                path = G.get_shortest_paths(s, t)[0]
            except:
                pass
        results['shortest_path'] = time.time() - start
    
    if 'pagerank' in algorithms:
        start = time.time()
        pr = G.pagerank()
        results['pagerank'] = time.time() - start
    
    if 'betweenness' in algorithms:
        start = time.time()
        bc = G.betweenness(vertices=None, directed=True, cutoff=None)
        results['betweenness'] = time.time() - start
    
    return results

# ========== Axolotl-RS 测试（通过 Rust 二进制） ==========
def benchmark_axolotl_rs(edges, n_vertices, algorithms):
    """使用 Axolotl-RS 运行算法"""
    # 将图数据写入临时文件
    graph_data = {
        'n_vertices': n_vertices,
        'edges': edges
    }
    
    with open('/tmp/benchmark_graph.json', 'w') as f:
        json.dump(graph_data, f)
    
    # 运行 Rust 基准测试
    results = {}
    
    if 'shortest_path' in algorithms:
        start = time.time()
        subprocess.run(
            ['cargo', 'run', '--release', '--example', 'benchmark', '--', '--algorithm', 'shortest_path'],
            cwd='/tmp/axolotl-rs',
            capture_output=True
        )
        results['shortest_path'] = time.time() - start
    
    if 'pagerank' in algorithms:
        start = time.time()
        subprocess.run(
            ['cargo', 'run', '--release', '--example', 'benchmark', '--', '--algorithm', 'pagerank'],
            cwd='/tmp/axolotl-rs',
            capture_output=True
        )
        results['pagerank'] = time.time() - start
    
    if 'betweenness' in algorithms:
        start = time.time()
        subprocess.run(
            ['cargo', 'run', '--release', '--example', 'benchmark', '--', '--algorithm', 'betweenness'],
            cwd='/tmp/axolotl-rs',
            capture_output=True
        )
        results['betweenness'] = time.time() - start
    
    return results

# ========== 主测试函数 ==========
def run_benchmark(n_vertices, n_edges, algorithms, runs=3):
    """运行完整基准测试"""
    print(f"\n{'='*80}")
    print(f"基准测试: {n_vertices} 顶点, {n_edges} 边")
    print(f"算法: {', '.join(algorithms)}")
    print(f"{'='*80}\n")
    
    # 生成图（所有测试使用同一个图）
    random.seed(42)
    edges = generate_graph(n_vertices, n_edges)
    
    # 运行测试
    results = {
        'networkx': benchmark_networkx(edges, n_vertices, algorithms),
        'igraph': benchmark_igraph(edges, n_vertices, algorithms),
        'axolotl_rs': benchmark_axolotl_rs(edges, n_vertices, algorithms)
    }
    
    # 打印结果
    print(f"{'算法':<20} {'NetworkX':<15} {'igraph':<15} {'Axolotl-RS':<15}")
    print("-" * 80)
    
    for algo in algorithms:
        nx_time = results['networkx'].get(algo, 'N/A')
        ig_time = results['igraph'].get(algo, 'N/A')
        ax_time = results['axolotl_rs'].get(algo, 'N/A')
        
        if isinstance(nx_time, float):
            nx_str = f"{nx_time:.4f}s"
        else:
            nx_str = str(nx_time)
        
        if isinstance(ig_time, float):
            ig_str = f"{ig_time:.4f}s"
        else:
            ig_str = str(ig_time)
        
        if isinstance(ax_time, float):
            ax_str = f"{ax_time:.4f}s"
        else:
            ax_str = str(ax_time)
        
        print(f"{algo:<20} {nx_str:<15} {ig_str:<15} {ax_str:<15}")
    
    # 计算加速比
    print(f"\n{'加速比 (vs NetworkX)':<20} {'igraph':<15} {'Axolotl-RS':<15}")
    print("-" * 80)
    
    for algo in algorithms:
        nx_time = results['networkx'].get(algo)
        ig_time = results['igraph'].get(algo)
        ax_time = results['axolotl_rs'].get(algo)
        
        if nx_time and isinstance(nx_time, float):
            if ig_time and isinstance(ig_time, float):
                ig_speedup = nx_time / ig_time
                ig_str = f"{ig_speedup:.1f}x"
            else:
                ig_str = 'N/A'
            
            if ax_time and isinstance(ax_time, float):
                ax_speedup = nx_time / ax_time
                ax_str = f"{ax_speedup:.1f}x"
            else:
                ax_str = 'N/A'
            
            print(f"{algo:<20} {ig_str:<15} {ax_str:<15}")
    
    return results

# ========== 主程序 ==========
if __name__ == '__main__':
    print("Axolotl-RS vs igraph vs NetworkX 性能对比测试")
    print("=" * 80)
    
    # 检查依赖
    try:
        import networkx
        print("✓ NetworkX 已安装")
    except ImportError:
        print("✗ NetworkX 未安装，请运行: pip install networkx")
        sys.exit(1)
    
    try:
        import igraph
        print("✓ igraph 已安装")
    except ImportError:
        print("✗ igraph 未安装，请运行: pip install python-igraph")
        sys.exit(1)
    
    # 运行基准测试
    test_cases = [
        (100, 200, ['shortest_path', 'pagerank', 'betweenness']),
        (500, 1000, ['shortest_path', 'pagerank', 'betweenness']),
        (1000, 5000, ['pagerank', 'betweenness']),  # 大型图跳过最短路径（太慢）
    ]
    
    all_results = []
    
    for n_vertices, n_edges, algorithms in test_cases:
        results = run_benchmark(n_vertices, n_edges, algorithms)
        all_results.append({
            'n_vertices': n_vertices,
            'n_edges': n_edges,
            'algorithms': algorithms,
            'results': results
        })
    
    # 保存结果
    with open('/tmp/benchmark_results.json', 'w') as f:
        json.dump(all_results, f, indent=2)
    
    print("\n" + "=" * 80)
    print("基准测试完成！结果已保存到 /tmp/benchmark_results.json")
    print("=" * 80)
