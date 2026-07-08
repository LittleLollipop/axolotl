# Axolotl 图数据库系统架构设计

> **📦 Swift 原型 — 历史参考实现**
> 
> 该 Swift 实现为项目早期原型，现已作为参考保留。当前活跃开发在 Rust 版本（`prototype-rust/`）中，v0.1.0-beta 已于 2026-07-07 发布。请以 [根目录 README](../README.md) 为权威文档。

**版本**：v0.1 (MVP - Minimum Viable Product)  
**日期**：2026-07-04  
**目标**：构建一个"可以使用的"图数据库原型

---

## 一、设计目标

### 1.1 核心目标

1. **持久化** - 图数据可以保存到磁盘，重启后恢复
2. **CRUD** - 支持顶点的增删改查，边的增删改查
3. **查询** - 支持基本的图遍历查询
4. **索引** - 支持按 ID 快速查找顶点
5. **增量更新** - 利用我们已经实现的增量算法

### 1.2 非目标（未来实现）

- ❌ 事务支持（ACID）
- ❌ 并发控制（多用户）
- ❌ 分布式存储
- ❌ 复杂查询语言（Cypher）
- ❌ 高可用/备份

### 1.3 设计原则

1. **简单优先** - 先实现最小可用功能，再逐步完善
2. **Swift 原生** - 使用 Swift 语言，不依赖外部数据库
3. **统一内存友好** - 利用 Apple M4 的统一内存架构
4. **增量算法集成** - 将我们已经实现的增量算法集成到数据库中

---

## 二、系统架构

### 2.1 整体架构（三层）

```
┌─────────────────────────────────────────────────────┐
│           Query API Layer (查询接口层)              │
│  - GraphDatabase class                            │
│  - CRUD operations                               │
│  - Traversal queries (BFS, shortestPath, etc.) │
└─────────────────────┬───────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────┐
│        Storage Engine Layer (存储引擎层)           │
│  - PersistentGraph class                         │
│  - Vertex/Edge storage                          │
│  - Index management                             │
│  - Incremental algorithm integration             │
└─────────────────────┬───────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────┐
│      Persistence Layer (持久化层)                 │
│  - Binary file format (AxolotlDB)              │
│  - Memory-mapped I/O (for unified memory)      │
│  - Incremental persistence (only write changes)  │
└─────────────────────────────────────────────────┘
```

### 2.2 核心类设计

#### **1. GraphDatabase**（查询接口层）

```swift
class GraphDatabase {
    let storage: PersistentGraph
    
    // CRUD - Vertex
    func addVertex(properties: [String: Any]) -> VertexID
    func deleteVertex(id: VertexID) throws
    func updateVertex(id: VertexID, properties: [String: Any]) throws
    func getVertex(id: VertexID) -> Vertex?
    
    // CRUD - Edge
    func addEdge(from: VertexID, to: VertexID, properties: [String: Any]) throws
    func deleteEdge(from: VertexID, to: VertexID) throws
    func updateEdge(from: VertexID, to: VertexID, properties: [String: Any]) throws
    func getEdge(from: VertexID, to: VertexID) -> Edge?
    
    // Query
    func getNeighbors(of vertex: VertexID) -> [VertexID]
    func bfs(from start: VertexID, maxDepth: Int) -> [VertexID]
    func shortestPath(from: VertexID, to: VertexID) -> [VertexID]
    
    // Persistence
    func save(to path: String) throws
    func load(from path: String) throws
}
```

#### **2. PersistentGraph**（存储引擎层）

```swift
class PersistentGraph {
    var vertices: [VertexID: Vertex]
    var edges: [EdgeID: Edge]
    var adjacencyList: [[VertexID]]
    
    // Index
    var vertexIndex: [VertexID: Int]  // vertex ID -> position in file
    
    // Incremental algorithm state
    var pageRankScores: [Float]?
    var connectedComponents: [Int]?
    
    // Persistence
    func save(to filePath: String) throws
    func load(from filePath: String) throws
    func saveIncremental(to filePath: String, changes: [Change]) throws
}
```

#### **3. Vertex 和 Edge**（数据模型）

```swift
typealias VertexID = UInt64
typealias EdgeID = (from: VertexID, to: VertexID)

struct Vertex {
    let id: VertexID
    var properties: [String: Any]  // Flexible property storage
}

struct Edge {
    let id: EdgeID
    var properties: [String: Any]
    var weight: Float  // For weighted graphs
}
```

### 2.3 持久化格式设计

#### **文件格式（AxolotlDB）**

```
AxolotlDB File Format:
┌─────────────────────────────────┐
│  Header (64 bytes)            │
│  - Magic number: "AXOLOTL"  │
│  - Version: 0.1              │
│  - Vertex count                │
│  - Edge count                 │
│  - Index offset               │
│  - Data offset                │
├─────────────────────────────────┤
│  Vertex Data (variable)       │
│  - Vertex 1: ID + properties │
│  - Vertex 2: ID + properties │
│  ...                         │
├─────────────────────────────────┤
│  Edge Data (variable)         │
│  - Edge 1: from + to + ...  │
│  - Edge 2: from + to + ...  │
│  ...                         │
├─────────────────────────────────┤
│  Index (variable)             │
│  - Vertex ID -> file offset   │
│  - (for fast lookup)          │
└─────────────────────────────────┘
```

#### **持久化策略**

1. **全量保存**（save）
   - 将整个图写入文件
   - 使用二进制格式（紧凑、快速）

2. **增量保存**（saveIncremental）
   - 只写入变化的顶点/边
   - 需要记录变更日志（WAL - Write Ahead Log）

3. **加载**（load）
   - 从文件读取整个图
   - 重建索引

---

## 三、实现计划

### 3.1 阶段 1：最小可用原型（MVP）- 预计 6-8 周

#### **Week 1-2：持久化层**

- [ ] 设计二进制文件格式
- [ ] 实现 `save()` 和 `load()` 功能
- [ ] 测试：保存/加载 10K 顶点、50K 边的图

#### **Week 3：CRUD 操作**

- [ ] 实现 `addVertex()`, `deleteVertex()`, `updateVertex()`, `getVertex()`
- [ ] 实现 `addEdge()`, `deleteEdge()`, `updateEdge()`, `getEdge()`
- [ ] 测试：增删改查操作

#### **Week 4：查询接口**

- [ ] 实现 `getNeighbors()`
- [ ] 实现 `bfs()`
- [ ] 实现 `shortestPath()`
- [ ] 测试：基本查询功能

#### **Week 5：索引**

- [ ] 实现顶点 ID 索引（哈希表）
- [ ] 测试：快速查找顶点

#### **Week 6-8：集成增量算法**

- [ ] 集成增量 PageRank
- [ ] 集成增量 BFS
- [ ] 集成增量 SSSP
- [ ] 测试：增量更新性能

### 3.2 阶段 2：基本可用的数据库 - 预计 4-6 周

#### **Week 9-10：错误处理**

- [ ] 定义错误类型
- [ ] 添加错误处理
- [ ] 添加日志记录

#### **Week 11-12：事务支持（简化版）**

- [ ] 实现原子性（Atomicity）
- [ ] 实现持久性（Durability）
- [ ] 测试：事务回滚

#### **Week 13-14：并发控制（简化版）**

- [ ] 实现读写锁
- [ ] 测试：多用户并发访问

### 3.3 阶段 3：完整可用的系统 - 预计 2-4 周

#### **Week 15-16：备份/恢复**

- [ ] 实现备份功能
- [ ] 实现恢复功能
- [ ] 测试：数据恢复

#### **Week 17-18：监控/日志**

- [ ] 添加性能监控
- [ ] 添加操作日志
- [ ] 测试：监控功能

---

## 四、技术决策

### 4.1 为什么选择 Swift？

1. **统一内存架构友好** - Swift 可以直接访问 Metal GPU
2. **性能** - Swift 的性能接近 C++
3. **安全性** - Swift 的内存安全特性减少 bug
4. **我们已经在用** - 所有算法原型都是 Swift

### 4.2 为什么选择二进制文件格式？

1. **性能** - 二进制 I/O 比 JSON/XML 快
2. **紧凑** - 二进制格式占用空间小
3. **简单** - 不需要依赖外部数据库（如 SQLite）

### 4.3 为什么不支持分布式？

1. **MVP 目标** - 先实现单机版本
2. **复杂性** - 分布式系统复杂度很高
3. **统一内存架构** - Apple M4 的统一内存已经很大（128GB），可以存储大规模图

---

## 五、风险评估

### 5.1 技术风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| **Swift 性能不够** | 高 | 使用 Instruments 分析瓶颈，优化关键路径 |
| **二进制格式设计错误** | 中 | 版本化文件格式，支持向后兼容 |
| **内存占用过大** | 中 | 使用内存映射（mmap），按需加载 |

### 5.2 时间风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| **工作量被低估** | 高 | 优先实现核心功能，非核心功能延后 |
| **技术难题** | 中 | 寻求帮助（Stack Overflow、Apple Developer Forums） |

---

## 六、成功标准

### 6.1 MVP 成功标准

- ✅ 可以保存/加载包含 100K 顶点、1M 边的图（< 1 秒）
- ✅ 可以执行基本的 CRUD 操作
- ✅ 可以执行基本的图遍历查询（BFS、最短路径）
- ✅ 增量算法可以正确更新（PageRank、BFS、SSSP）

### 6.2 完整系统成功标准

- ✅ 支持事务（ACID 的 A 和 D）
- ✅ 支持并发访问（读写锁）
- ✅ 支持备份/恢复

---

## 七、下一步

1. **实现持久化层**（Week 1-2）
2. **实现 CRUD 操作**（Week 3）
3. **实现查询接口**（Week 4）
4. **实现索引**（Week 5）
5. **集成增量算法**（Week 6-8）

---

**附录：参考资源**

- [1] Neo4j 文件格式：https://neo4j.com/docs/
- [2] Redis 持久化机制：https://redis.io/docs/
- [3] SQLite 文件格式：https://sqlite.org/fileformat2.html
