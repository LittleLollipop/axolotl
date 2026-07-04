//
// prototype.c — 统一内存图数据库最小原型
//
// 实现 DESIGN.md v0.2 的数据结构：
//   - VertexSlot: 顶点槽
//   - EdgeBlock: 固定容量边块（硬件无关存储格式）
//   - GraphMemoryLayout: 图的整体布局
//
// 编译：
//   clang -O2 -o prototype prototype.c
//
// 运行：
//   ./prototype
//

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <time.h>

// ─────────────────────────────────────────────────────────────────────────────
// 配置
// ─────────────────────────────────────────────────────────────────────────────

#define MAX_VERTICES 1000000
#define MAX_EDGE_BLOCKS 5000000
#define BLOCK_CAPACITY 32  // EdgeBlock 容量（可配置，推荐 = warp_size）

// ─────────────────────────────────────────────────────────────────────────────
// 数据结构
// ─────────────────────────────────────────────────────────────────────────────

//
// EdgeBlock — 固定容量边块
//
// 内存布局（连续）：
//   [owner_vertex: 4B][edge_count: 4B][edges: BLOCK_CAPACITY * 4B]
//
// 注意：edge_count <= BLOCK_CAPACITY
//       如果顶点的边数超过 BLOCK_CAPACITY，使用多个 EdgeBlock
//
typedef struct {
    uint32_t owner_vertex;       // 这组边属于哪个顶点
    uint32_t edge_count;         // 实际边数（<= BLOCK_CAPACITY）
    uint32_t edges[BLOCK_CAPACITY];  // 目标顶点 ID（固定大小，方便 GPU 访问）
} EdgeBlock;

//
// VertexSlot — 顶点槽
//
typedef struct {
    uint64_t global_id;          // 全局唯一 ID（跨会话稳定）
    uint32_t edge_block_start;   // 第一个 EdgeBlock 的索引
    uint32_t edge_block_count;   // 这个顶点的 EdgeBlock 数量
    uint8_t  flags;              // 状态（存活/删除/...）
    uint8_t  padding[3];
} VertexSlot;

//
// GraphMemoryLayout — 图的整体布局
//
typedef struct {
    // 顶点数组（固定大小，随机访问 O(1)）
    VertexSlot *vertices;
    uint32_t vertex_capacity;
    uint32_t vertex_count;
    
    // EdgeBlock 数组（连续存储）
    EdgeBlock *edge_blocks;
    uint32_t edge_block_capacity;
    uint32_t edge_block_count;
    
    // 空闲槽（LIFO）
    uint32_t *free_vertices;
    uint32_t free_vertex_count;
    uint32_t *free_blocks;
    uint32_t free_block_count;
} GraphMemoryLayout;

// ─────────────────────────────────────────────────────────────────────────────
// GraphMemoryLayout 操作
// ─────────────────────────────────────────────────────────────────────────────

//
// 初始化图
//
void graph_init(GraphMemoryLayout *graph, uint32_t vertex_cap, uint32_t block_cap) {
    graph->vertices = (VertexSlot *)calloc(vertex_cap, sizeof(VertexSlot));
    graph->vertex_capacity = vertex_cap;
    graph->vertex_count = 0;
    
    graph->edge_blocks = (EdgeBlock *)calloc(block_cap, sizeof(EdgeBlock));
    graph->edge_block_capacity = block_cap;
    graph->edge_block_count = 0;
    
    graph->free_vertices = (uint32_t *)malloc(vertex_cap * sizeof(uint32_t));
    graph->free_vertex_count = 0;
    graph->free_blocks = (uint32_t *)malloc(block_cap * sizeof(uint32_t));
    graph->free_block_count = 0;
}

//
// 销毁图
//
void graph_destroy(GraphMemoryLayout *graph) {
    free(graph->vertices);
    free(graph->edge_blocks);
    free(graph->free_vertices);
    free(graph->free_blocks);
}

//
// 分配顶点槽
//
uint32_t graph_alloc_vertex(GraphMemoryLayout *graph, uint64_t global_id) {
    uint32_t slot;
    
    if (graph->free_vertex_count > 0) {
        // 复用空闲槽
        slot = graph->free_vertices[--graph->free_vertex_count];
    } else {
        // 分配新槽
        if (graph->vertex_count >= graph->vertex_capacity) {
            fprintf(stderr, "Error: vertex capacity exceeded\n");
            return (uint32_t)-1;
        }
        slot = graph->vertex_count++;
    }
    
    graph->vertices[slot].global_id = global_id;
    graph->vertices[slot].edge_block_start = 0;
    graph->vertices[slot].edge_block_count = 0;
    graph->vertices[slot].flags = 1;  // 存活
    
    return slot;
}

//
// 分配 EdgeBlock 槽
//
uint32_t graph_alloc_block(GraphMemoryLayout *graph) {
    uint32_t slot;
    
    if (graph->free_block_count > 0) {
        // 复用空闲槽
        slot = graph->free_blocks[--graph->free_block_count];
    } else {
        // 分配新槽
        if (graph->edge_block_count >= graph->edge_block_capacity) {
            fprintf(stderr, "Error: edge block capacity exceeded\n");
            return (uint32_t)-1;
        }
        slot = graph->edge_block_count++;
    }
    
    return slot;
}

//
// 添加边（核心操作）
//
// 策略：
//   1. 找到 src 的最后一个 EdgeBlock
//   2. 如果还有空间，追加到这个 Block
//   3. 否则，新建 EdgeBlock
//
int graph_add_edge(GraphMemoryLayout *graph, uint32_t src, uint32_t dst) {
    // 检查顶点是否存在
    if (src >= graph->vertex_count || dst >= graph->vertex_count) {
        fprintf(stderr, "Error: vertex out of range\n");
        return -1;
    }
    
    uint32_t block_start = graph->vertices[src].edge_block_start;
    uint32_t block_count = graph->vertices[src].edge_block_count;
    
    if (block_count == 0) {
        // 第一个 EdgeBlock
        uint32_t block_slot = graph_alloc_block(graph);
        if (block_slot == (uint32_t)-1) return -1;
        
        EdgeBlock *block = &graph->edge_blocks[block_slot];
        block->owner_vertex = src;
        block->edge_count = 1;
        block->edges[0] = dst;
        
        graph->vertices[src].edge_block_start = block_slot;
        graph->vertices[src].edge_block_count = 1;
    } else {
        // 检查最后一个 Block 是否还有空间
        uint32_t last_block_slot = block_start + block_count - 1;
        EdgeBlock *last_block = &graph->edge_blocks[last_block_slot];
        
        if (last_block->edge_count < BLOCK_CAPACITY) {
            // 还有空间，追加
            last_block->edges[last_block->edge_count] = dst;
            last_block->edge_count++;
        } else {
            // 最后一个 Block 满了，新建 Block
            uint32_t block_slot = graph_alloc_block(graph);
            if (block_slot == (uint32_t)-1) return -1;
            
            EdgeBlock *block = &graph->edge_blocks[block_slot];
            block->owner_vertex = src;
            block->edge_count = 1;
            block->edges[0] = dst;
            
            graph->vertices[src].edge_block_count++;
        }
    }
    
    return 0;
}

// ─────────────────────────────────────────────────────────────────────────────
// BFS 实现
// ─────────────────────────────────────────────────────────────────────────────

//
// BFS（基于 EdgeBlock）
//
// 返回遍历的顶点数量
//
uint32_t graph_bfs(GraphMemoryLayout *graph, uint32_t start, uint8_t *visited) {
    if (start >= graph->vertex_count) return 0;
    
    // BFS 队列（用数组模拟）
    uint32_t *queue = (uint32_t *)malloc(graph->vertex_count * sizeof(uint32_t));
    uint32_t front = 0, rear = 0;
    
    // 起点入队
    queue[rear++] = start;
    visited[start] = 1;
    
    uint32_t visited_count = 1;
    
    while (front < rear) {
        uint32_t v = queue[front++];
        
        // 遍历 v 的所有边
        uint32_t block_start = graph->vertices[v].edge_block_start;
        uint32_t block_count = graph->vertices[v].edge_block_count;
        
        for (uint32_t i = 0; i < block_count; i++) {
            EdgeBlock *block = &graph->edge_blocks[block_start + i];
            
            for (uint32_t j = 0; j < block->edge_count; j++) {
                uint32_t nb = block->edges[j];
                
                if (!visited[nb]) {
                    visited[nb] = 1;
                    queue[rear++] = nb;
                    visited_count++;
                }
            }
        }
    }
    
    free(queue);
    return visited_count;
}

// ─────────────────────────────────────────────────────────────────────────────
// CSR 格式（用于对比）
// ─────────────────────────────────────────────────────────────────────────────

typedef struct {
    uint32_t *offsets;   // 大小：vertex_count + 1
    uint32_t *edges;     // 大小：edge_count
    uint32_t vertex_count;
    uint32_t edge_count;
} CSRGraph;

void csr_init(CSRGraph *g, uint32_t vertex_count) {
    g->vertex_count = vertex_count;
    g->edge_count = 0;
    g->offsets = (uint32_t *)calloc(vertex_count + 1, sizeof(uint32_t));
    g->edges = NULL;
}

//
// 第一遍：计算度数
//
void csr_count_degree(CSRGraph *g, uint32_t src) {
    g->offsets[src + 1]++;
}

//
// 第二遍：构建 offsets
//
void csr_build_offsets(CSRGraph *g) {
    for (uint32_t i = 1; i <= g->vertex_count; i++) {
        g->offsets[i] += g->offsets[i - 1];
    }
    
    // 现在 offsets[i+1] - offsets[i] 是顶点 i 的度数
    // 但我们需要把 offsets 数组右移一位
    // 先保存 edge_count
    g->edge_count = g->offsets[g->vertex_count];
    
    // 右移
    for (uint32_t i = g->vertex_count; i > 0; i--) {
        g->offsets[i] = g->offsets[i - 1];
    }
    g->offsets[0] = 0;
}

//
// 第三遍：填充边
//
void csr_add_edge(CSRGraph *g, uint32_t src, uint32_t dst) {
    // 找到插入位置
    uint32_t pos = g->offsets[src];
    g->offsets[src]++;  // 这个实现不对...
    
    // 正确的做法是用一个临时数组来跟踪当前插入位置
    // 为了简单，这里用另一个方法
}

//
// 正确的 CSR 构建方法：先收集所有边，再构建
//
CSRGraph *csr_build(uint32_t vertex_count, uint32_t *edges, uint32_t edge_count) {
    CSRGraph *g = (CSRGraph *)malloc(sizeof(CSRGraph));
    g->vertex_count = vertex_count;
    g->edge_count = edge_count;
    
    // 计算度数
    uint32_t *degree = (uint32_t *)calloc(vertex_count, sizeof(uint32_t));
    for (uint32_t i = 0; i < edge_count * 2; i += 2) {
        degree[edges[i]]++;
    }
    
    // 构建 offsets
    g->offsets = (uint32_t *)malloc((vertex_count + 1) * sizeof(uint32_t));
    g->offsets[0] = 0;
    for (uint32_t i = 0; i < vertex_count; i++) {
        g->offsets[i + 1] = g->offsets[i] + degree[i];
    }
    
    // 填充边
    g->edges = (uint32_t *)malloc(edge_count * sizeof(uint32_t));
    uint32_t *cursor = (uint32_t *)calloc(vertex_count, sizeof(uint32_t));
    
    for (uint32_t i = 0; i < edge_count; i++) {
        uint32_t src = edges[i * 2];
        uint32_t dst = edges[i * 2 + 1];
        
        uint32_t pos = g->offsets[src] + cursor[src];
        g->edges[pos] = dst;
        cursor[src]++;
    }
    
    free(degree);
    free(cursor);
    
    return g;
}

void csr_destroy(CSRGraph *g) {
    free(g->offsets);
    free(g->edges);
    free(g);
}

//
// BFS（基于 CSR）
//
uint32_t csr_bfs(CSRGraph *g, uint32_t start, uint8_t *visited) {
    memset(visited, 0, g->vertex_count * sizeof(uint8_t));
    
    uint32_t *queue = (uint32_t *)malloc(g->vertex_count * sizeof(uint32_t));
    uint32_t front = 0, rear = 0;
    
    queue[rear++] = start;
    visited[start] = 1;
    uint32_t visited_count = 1;
    
    while (front < rear) {
        uint32_t v = queue[front++];
        
        for (uint32_t i = g->offsets[v]; i < g->offsets[v + 1]; i++) {
            uint32_t nb = g->edges[i];
            if (!visited[nb]) {
                visited[nb] = 1;
                queue[rear++] = nb;
                visited_count++;
            }
        }
    }
    
    free(queue);
    return visited_count;
}

// ─────────────────────────────────────────────────────────────────────────────
// 测试
// ─────────────────────────────────────────────────────────────────────────────

//
// 生成随机图（同时收集边用于 CSR 构建）
//
uint32_t *generate_random_graph(GraphMemoryLayout *graph, uint32_t vertex_count, uint32_t edge_count, uint32_t *edges) {
    // 创建顶点
    for (uint32_t i = 0; i < vertex_count; i++) {
        graph_alloc_vertex(graph, i);
    }
    
    // 添加随机边
    srand(42);
    for (uint32_t i = 0; i < edge_count; i++) {
        uint32_t src = rand() % vertex_count;
        uint32_t dst = rand() % vertex_count;
        if (src != dst) {
            graph_add_edge(graph, src, dst);
            
            // 保存边用于 CSR 构建
            if (edges) {
                edges[i * 2] = src;
                edges[i * 2 + 1] = dst;
            }
        } else {
            i--;  // 重新生成
        }
    }
    
    return edges;
}

//
// 主函数
//
int main(void) {
    printf("=== 统一内存图数据库最小原型 ===\n\n");
    
    // 测试参数
    uint32_t vertex_count = 100000;
    uint32_t edge_count = 1000000;
    
    printf("测试参数：\n");
    printf("  顶点数：%u\n", vertex_count);
    printf("  边数：%u\n", edge_count);
    printf("  BLOCK_CAPACITY：%u\n\n", BLOCK_CAPACITY);
    
    // ── 初始化图 ──
    printf("1. 初始化图...\n");
    GraphMemoryLayout graph;
    graph_init(&graph, vertex_count, edge_count * 2);
    printf("   完成\n\n");
    
    // ── 生成随机图 ──
    printf("2. 生成随机图（EdgeBlock 格式）...\n");
    uint32_t *edges = (uint32_t *)malloc(edge_count * 2 * sizeof(uint32_t));
    
    clock_t start = clock();
    generate_random_graph(&graph, vertex_count, edge_count, edges);
    clock_t end = clock();
    double build_time = (double)(end - start) / CLOCKS_PER_SEC;
    printf("   完成（耗时 %.3f 秒）\n\n", build_time);
    
    // ── BFS 测试（EdgeBlock 格式）──
    printf("3. BFS 测试（EdgeBlock 格式）...\n");
    uint8_t *visited = (uint8_t *)malloc(vertex_count * sizeof(uint8_t));
    
    start = clock();
    uint32_t visited_count = graph_bfs(&graph, 0, visited);
    end = clock();
    double bfs_time = (double)(end - start) / CLOCKS_PER_SEC;
    
    printf("   从顶点 0 出发，访问了 %u 个顶点\n", visited_count);
    printf("   BFS 耗时：%.6f 秒\n\n", bfs_time);
    
    // ── 统计 EdgeBlock 使用情况 ──
    printf("4. EdgeBlock 统计：\n");
    uint32_t total_blocks = 0;
    uint32_t total_edges = 0;
    uint32_t max_block_size = 0;
    uint32_t min_block_size = (uint32_t)-1;
    uint32_t full_blocks = 0;
    
    for (uint32_t i = 0; i < graph.vertex_count; i++) {
        uint32_t block_start = graph.vertices[i].edge_block_start;
        uint32_t block_count = graph.vertices[i].edge_block_count;
        
        total_blocks += block_count;
        
        for (uint32_t j = 0; j < block_count; j++) {
            EdgeBlock *block = &graph.edge_blocks[block_start + j];
            total_edges += block->edge_count;
            
            if (block->edge_count > max_block_size) max_block_size = block->edge_count;
            if (block->edge_count < min_block_size) min_block_size = block->edge_count;
            
            if (block->edge_count == BLOCK_CAPACITY) full_blocks++;
        }
    }
    
    printf("   总 EdgeBlock 数：%u\n", total_blocks);
    printf("   总边数：%u\n", total_edges);
    printf("   平均每个 EdgeBlock 的边数：%.2f\n", (double)total_edges / total_blocks);
    printf("   最大 EdgeBlock 大小：%u\n", max_block_size);
    printf("   最小 EdgeBlock 大小：%u\n", min_block_size);
    printf("   填满的 EdgeBlock 比例：%.2f%%\n\n", (double)full_blocks / total_blocks * 100);
    
    // ── CSR 对比 ──
    printf("5. 构建 CSR 格式...\n");
    
    start = clock();
    CSRGraph *csr = csr_build(vertex_count, edges, edge_count);
    end = clock();
    double csr_build_time = (double)(end - start) / CLOCKS_PER_SEC;
    printf("   完成（耗时 %.3f 秒）\n", csr_build_time);
    
    printf("   CSR BFS 测试...\n");
    start = clock();
    uint32_t csr_visited_count = csr_bfs(csr, 0, visited);
    end = clock();
    double csr_bfs_time = (double)(end - start) / CLOCKS_PER_SEC;
    
    printf("   从顶点 0 出发，访问了 %u 个顶点\n", csr_visited_count);
    printf("   CSR BFS 耗时：%.6f 秒\n\n", csr_bfs_time);
    
    // ── 性能对比 ──
    printf("6. 性能对比：\n");
    printf("   EdgeBlock BFS：%.6f 秒\n", bfs_time);
    printf("   CSR BFS：%.6f 秒\n", csr_bfs_time);
    printf("   比值（CSR / EdgeBlock）：%.2fx\n\n", csr_bfs_time / bfs_time);
    
    // ── 清理 ──
    free(visited);
    free(edges);
    graph_destroy(&graph);
    csr_destroy(csr);
    
    printf("=== 测试完成 ===\n");
    
    return 0;
}
