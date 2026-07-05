#!/usr/bin/env python3
"""
生成更大的图数据集（用于性能测试）
"""

import random
import sys

def generate_graph(n_vertices, n_edges, filename):
    """生成随机图（边列表格式）"""
    print(f"生成图: {n_vertices} 顶点, {n_edges} 边")
    print(f"输出文件: {filename}")
    
    edges = set()
    batch_size = 1000000  # 批量处理，避免内存占用过大
    
    with open(filename, 'w') as f:
        count = 0
        while count < n_edges:
            batch = min(batch_size, n_edges - count)
            
            for _ in range(batch):
                u = random.randint(0, n_vertices - 1)
                v = random.randint(0, n_vertices - 1)
                if u != v:
                    f.write(f"{u} {v}\n")
                    count += 1
            
            print(f"  进度: {count}/{n_edges} ({count*100//n_edges}%)")
    
    print(f"完成！生成 {count} 条边")

if __name__ == '__main__':
    print("=== 生成图数据集 ===\n")
    
    # 生成 100K 顶点, 1M 边的图
    generate_graph(
        n_vertices=100000,
        n_edges=1000000,
        filename='dataset_100k_1m.edgelist'
    )
    
    print("\n=== 生成完成 ===")
    print("文件: dataset_100k_1m.edgelist")
