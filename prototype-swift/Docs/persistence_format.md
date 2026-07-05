# 持久化文件格式设计

## 文件结构（Version 1.0）

```
Axolotl Graph Database File (.agdb)
=====================================

[Header] (64 bytes)
├── Magic Number: "AGDB" (4 bytes)
├── Version: 1.0 (2 bytes major, 2 bytes minor)
├── Vertex Count (8 bytes, UInt64)
├── Edge Count (8 bytes, UInt64)
├── Vertex Properties Offset (8 bytes, UInt64)
├── Edge Properties Offset (8 bytes, UInt64)
├── Adjacency List Offset (8 bytes, UInt64)
├── Index Offset (8 bytes, UInt64)
├── Reserved (18 bytes, for future use)
└── Checksum (8 bytes, UInt64, CRC32 of header)

[Vertex Table] (Variable length)
├── Vertex ID 0 (UInt64)
├── Vertex Properties Offset (UInt64, pointer to properties in Vertex Properties Section)
├── Vertex Degree (UInt32, number of outgoing edges)
├── Vertex Adjacency List Offset (UInt64, pointer to adjacency list)
└── ... (repeated for each vertex)

[Edge Table] (Variable length)
├── Edge ID (from: UInt64, to: UInt64)
├── Edge Properties Offset (UInt64, pointer to properties in Edge Properties Section)
└── ... (repeated for each edge)

[Vertex Properties Section] (Variable length)
├── Vertex 0 Properties (JSON format, null-terminated string)
├── Vertex 1 Properties
└── ...

[Edge Properties Section] (Variable length)
├── Edge 0 Properties (JSON format, null-terminated string)
├── Edge 1 Properties
└── ...

[Adjacency List Section] (Variable length)
├── Vertex 0's neighbors (array of UInt64)
├── Vertex 1's neighbors
└── ...

[Index Section] (Variable length, optional)
├── Vertex ID Index (hash table for fast lookup)
└── ... (future indexes)
```

## 设计原则

1. **内存映射友好**：文件结构可以直接 mmap 到内存
2. **增量更新支持**：只修改变化的部分（Vertex Table、Edge Table、Properties）
3. **校验和**：防止文件损坏
4. **向前兼容**：Version 字段允许未来扩展

## 实现优先级

### Phase 1: Basic Persistence (Week 1-2)
- [x] Design file format (this document)
- [ ] Implement `save()` method
- [ ] Implement `load()` method
- [ ] Test with small graphs (< 10K vertices)

### Phase 2: Incremental Persistence (Week 3-4)
- [ ] Track dirty flags (which vertices/edges changed)
- [ ] Implement incremental save (only write dirty parts)
- [ ] Test with large graphs (1M+ vertices)

### Phase 3: Optimization (Week 5-6)
- [ ] Memory-mapped I/O (mmap)
- [ ] Compression (optional, for large properties)
- [ ] Concurrent read (multiple readers, single writer)

## 文件扩展名

- `.agdb` - Axolotl Graph Database file
