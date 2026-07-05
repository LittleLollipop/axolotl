# 事务支持设计文档

## 目标

为 Axolotl-RS 添加事务支持，实现 ACID 中的：
- **A**tomicity（原子性）：全部成功或全部回滚
- **C**onsistency（一致性）：事务前后图处于一致状态
- **I**solation（隔离性）：快照隔离（Snapshot Isolation）
- **D**urability（持久性）：WAL 保证崩溃后恢复

## 设计

### 核心概念

1. **WAL（Write-Ahead Log）**：所有写操作先写日志，再修改内存
2. **快照隔离**：每个事务看到的是事务开始时的图状态
3. **回滚日志**：记录旧值，用于回滚

### 事务状态

```
Active -> Committed
Active -> Rolled Back
```

### 并发控制

使用 **MVCC（多版本并发控制）** 的简化版：
- 每个顶点/边有一个版本号
- 事务开始时记录当前版本
- 提交时检查版本冲突（乐观锁）

## 实现计划

1. `src/transaction.rs` — 事务核心模块
2. 修改 `PersistentGraph` 支持事务
3. WAL 写入和回放
4. 快照隔离

## API 设计

```rust
// 基本用法
let mut tx = graph.begin_tx();
tx.add_vertex(1, props)?;
tx.add_edge(1, 2, 1.0, edge_props)?;
tx.commit()?;

// 回滚
let mut tx = graph.begin_tx();
tx.add_vertex(1, props)?;
tx.rollback()?; // 撤销所有操作

// 快照读
let tx = graph.begin_read_only_tx();
let vertex = tx.find_vertex().with_id(1).execute();
```

## 文件格式

### WAL 格式

```
[LogEntry]
  - tx_id: u64
  - op_type: u8 (0=begin, 1=add_vertex, 2=add_edge, 3=delete_vertex, 4=delete_edge, 5=commit, 6=rollback)
  - data: ...
```
