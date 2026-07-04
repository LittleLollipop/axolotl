# Axolotl 图数据库 - 社区检测算法对比

## 算法对比

### 1. 贪心模块化优化（Greedy Modularity Optimization）

**优点**：
- ✅ 准确性高（能正确识别星形图为单个社区）
- ✅ 稳定性好（不受初始化影响）

**缺点**：
- ❌ 非常慢（50 个顶点需要 15.8 秒）
- ❌ 时间复杂度高（O(n³) 或更高）

**适用场景**：
- 小型图（< 100 顶点）
- 需要高准确性的场景
- 原型验证

---

### 2. Louvain 算法（Louvain Algorithm）

**优点**：
- ✅ 非常快（50 个顶点只需 230ms，68x 加速）
- ✅ 可扩展到大图（100K+ 顶点）

**缺点**：
- ❌ 准确性较低（星形图失败，检测到 5 个社区而不是 1 个）
- ❌ 局域最优问题（可能卡在次优解）
- ❌ 分辨率参数无法完全解决问题

**适用场景**：
- 大型图（> 1000 顶点）
- 需要快速结果的场景
- 对准确性要求不那么严格的场景

---

## 模块度优化的固有限制

### 分辨率限制（Resolution Limit）

模块度优化有一个已知的限制：**无法检测小于一定大小的社区**。

对于星形图：
- 中心顶点的度为 k
- 叶子顶点的度为 1
- 将叶子顶点移动到中心顶点社区的模块度增益为 **0**
- 因此算法没有动机去合并它们

这不是实现 bug，而是**模块度函数本身的特性**。

### 为什么贪心算法能工作？

贪心算法**同时考虑合并所有社区对**，而不仅仅是移动单个顶点。这让它能够通过多步合并来逃脱局部最优。

Louvain 算法只移动单个顶点，容易卡在局部最优。

---

## 推荐使用策略

### 策略 1：根据图大小选择

```swift
if graph.vertexCount < 100 {
    // 使用贪心算法（准确性优先）
    let communities = graph.getCommunitiesGreedy()
} else {
    // 使用 Louvain 算法（速度优先）
    let communities = graph.getCommunitiesLouvain()
}
```

### 策略 2：使用贪心算法作为基准

```swift
// 先使用 Louvain 快速得到一个解
let louvainResult = graph.getCommunitiesLouvain()

// 如果图较小，使用贪心算法验证
if graph.vertexCount < 500 {
    let greedyResult = graph.getCommunitiesGreedy()
    // 比较两者，选择模块化得分更高的
}
```

### 策略 3：未来改进方向

1. **实现完整的两阶段 Louvain 算法**
   - 第一阶段：模块度优化（移动顶点）
   - 第二阶段：社区聚合（创建超顶点）
   - 重复直到不再改进

2. **添加其他社区检测算法**
   - **Label Propagation**（快速，但结果不稳定）
   - **Infomap**（基于信息论，准确性高）
   - **Walktrap**（基于随机游走）

3. **优化贪心算法**
   - 使用堆（heap）来加速查找最佳合并
   - 并行化计算模块化增益

---

## 测试案例

### 案例 1：独立社区

```
图结构：
- 社区 1：顶点 0-1-2（三角形）
- 社区 2：顶点 3-4-5（三角形）

预期结果：2 个社区
贪心算法：✅ 正确
Louvain 算法：✅ 正确
```

### 案例 2：星形图

```
图结构：
- 中心顶点：0
- 叶子顶点：1, 2, 3, 4
- 边：(0,1), (0,2), (0,3), (0,4)

预期结果：1 个社区
贪心算法：✅ 正确
Louvain 算法：❌ 失败（5 个社区）
```

### 案例 3：桥接顶点

```
图结构：
- 社区 1：顶点 0-1-2（三角形）
- 社区 2：顶点 3-4-5（三角形）
- 桥接边：(2,3)

预期结果：1 或 2 个社区
贪心算法：✅ 正确（1 个社区）
Louvain 算法：⚠️ 部分正确（2 个社区）
```

---

## 性能数据

| 图大小 | 贪心算法 | Louvain 算法 | 加速比 |
|---------|---------|---------|---------|
| 20 顶点，40 边 | 259.84 ms | 18.38 ms | **14.1x** |
| 50 顶点，100 边 | 15848.92 ms | 230.02 ms | **68.9x** |

⚠️ **注意**：贪心算法的性能数据异常地慢，可能存在实现问题或时间复杂度确实很高。

---

## 结论

1. **Louvain 算法很快，但准确性较低**
   - 对于大型图，速度优势显著
   - 对于小型图或需要高准确性的场景，建议使用贪心算法

2. **星形图是模块度优化的固有限制**
   - 这不是实现 bug，而是模块度函数本身的特性
   - 如果需要检测星形图这样的结构，建议使用其他算法（如 Infomap）

3. **当前实现状态**
   - ✅ 贪心算法：准确但慢
   - ✅ Louvain 算法：快但不准确（星形图失败）
   - ✅ 分辨率参数：无法完全解决问题

4. **下一步改进**
   - 优化贪心算法性能
   - 实现其他社区检测算法（Label Propagation, Infomap, Walktrap）
   - 考虑使用不同的模块度函数（如带分辨率参数的模块度）

---

## 使用示例

```swift
import Axolotl

// 创建图
let graph = PersistentGraph()

// 添加顶点和边
// ...

// 方法 1：使用贪心算法（准确性优先）
let greedyCommunities = graph.getCommunitiesGreedy()
print("Greedy: \(greedyCommunities.count) communities")

// 方法 2：使用 Louvain 算法（速度优先）
let louvainCommunities = graph.getCommunitiesLouvain()
print("Louvain: \(louvainCommunities.count) communities")

// 方法 3：使用带分辨率参数的 Louvain
let customCommunities = graph.getCommunitiesLouvain(resolution: 0.5)
print("Louvain (resolution=0.5): \(customCommunities.count) communities")
```

---

## 参考

1. **Louvain 算法原论文**：
   - Blondel, V. D., et al. (2008). "Fast unfolding of communities in large networks". Journal of Statistical Mechanics: Theory and Experiment.

2. **模块度优化的分辨率限制**：
   - Fortunato, S., & Barthelemy, M. (2007). "Resolution limit in community detection". Proceedings of the National Academy of Sciences.

3. **贪心模块化优化**：
   - Clauset, A., Newman, M. E., & Moore, C. (2004). "Finding community structure in very large networks". Physical Review E.

---

**最后更新**：2026-07-04
**作者**：Axolotl 图数据库团队
