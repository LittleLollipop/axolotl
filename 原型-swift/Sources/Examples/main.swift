// Examples/main.swift
// 图数据库演示程序（修正版 v2）

import Foundation
import GraphDatabase

print("🦄 Axolotl 图数据库演示")
print(String(repeating: "=", count: 50))

// ============================================
// 1. 创建一个社交网络图
// ============================================
print("\n📊 创建一个社交网络图...")

let graph = PersistentGraph()

// 添加顶点（人物）并存储姓名到 ID 的映射
var nameToId: [String: VertexID] = [:]
let vertices = [
    ("Alice", "Data Scientist", 28),
    ("Bob", "Software Engineer", 32),
    ("Charlie", "Product Manager", 30),
    ("Diana", "Data Scientist", 27),
    ("Eve", "Software Engineer", 29),
    ("Frank", "DevOps Engineer", 35),
    ("Grace", "Data Scientist", 31),
    ("Henry", "Software Engineer", 26),
    ("Ivy", "Product Manager", 33),
    ("Jack", "DevOps Engineer", 34),
]

for (name, job, age) in vertices {
    let vid = try! graph.addVertex(properties: [
        "name": .string(name),
        "job": .string(job),
        "age": .int(age)
    ])
    nameToId[name] = vid
}

// 添加边（朋友关系）
let edges = [
    ("Alice", "Bob", 0.9),
    ("Alice", "Charlie", 0.8),
    ("Alice", "Diana", 0.95),
    ("Bob", "Charlie", 0.7),
    ("Bob", "Eve", 0.85),
    ("Charlie", "Ivy", 0.9),
    ("Diana", "Grace", 0.9),
    ("Diana", "Alice", 0.95),
    ("Eve", "Bob", 0.85),
    ("Eve", "Henry", 0.8),
    ("Frank", "Jack", 0.9),
    ("Frank", "Bob", 0.6),
    ("Grace", "Alice", 0.9),
    ("Grace", "Diana", 0.9),
    ("Henry", "Eve", 0.8),
    ("Henry", "Jack", 0.7),
    ("Ivy", "Charlie", 0.9),
    ("Ivy", "Alice", 0.75),
    ("Jack", "Frank", 0.9),
    ("Jack", "Henry", 0.7),
]

for (fromName, toName, weight) in edges {
    if let fromId = nameToId[fromName], let toId = nameToId[toName] {
        _ = try! graph.addEdge(from: fromId, to: toId, properties: [:], weight: Double(weight))
    }
}

print("✅ 图创建完成！")
print("   顶点数: \(graph.vertexCount)")
print("   边数: \(graph.edgeCount)")

// ============================================
// 2. 基本查询
// ============================================
print("\n🔍 基本查询...")

// 查找 Alice 的朋友
if let aliceId = nameToId["Alice"] {
    let aliceNeighbors = graph.getNeighbors(of: aliceId)
    let neighborNames = aliceNeighbors.compactMap { vid -> String? in
        if let vertex = graph.getVertex(id: vid),
           case .string(let name) = vertex.properties["name"] {
            return name
        }
        return nil
    }
    print("   Alice 的朋友: \(neighborNames)")
}

// 计算最短路径
if let aliceId = nameToId["Alice"], let jackId = nameToId["Jack"] {
    let path = graph.shortestPath(from: aliceId, to: jackId)
    if !path.isEmpty {
        let pathNames = path.compactMap { vid -> String? in
            if let vertex = graph.getVertex(id: vid),
               case .string(let name) = vertex.properties["name"] {
                return name
            }
            return nil
        }
        print("   Alice 到 Jack 的最短路径: \(pathNames)")
    }
}

// ============================================
// 3. 图算法
// ============================================
print("\n🧮 运行图算法...")

// PageRank
print("   📈 计算 PageRank...")
let pageRank = graph.getPageRank()
let topPageRank = pageRank.sorted { $0.value > $1.value }.prefix(3)
print("   Top 3 PageRank:")
for (vid, score) in topPageRank {
    var name = "Unknown"
    if let vertex = graph.getVertex(id: vid),
       case .string(let n) = vertex.properties["name"] {
        name = n
    }
    print("      \(name) (\(vid)): \(String(format: "%.4f", score))")
}

// Betweenness Centrality
print("\n   🌉 计算 Betweenness Centrality...")
let betweenness = graph.betweennessCentrality()
let topBetweenness = betweenness.sorted { $0.value > $1.value }.prefix(3)
print("   Top 3 Betweenness Centrality:")
for (vid, score) in topBetweenness {
    var name = "Unknown"
    if let vertex = graph.getVertex(id: vid),
       case .string(let n) = vertex.properties["name"] {
        name = n
    }
    print("      \(name) (\(vid)): \(String(format: "%.4f", score))")
}

// Connected Components
print("\n   🔗 查找连通分量...")
let components = graph.computeWeaklyConnectedComponents()
print("   连通分量数量: \(components.count)")
for (i, component) in components.enumerated() {
    let names = component.map { vid -> String in
        if let vertex = graph.getVertex(id: vid),
           case .string(let name) = vertex.properties["name"] {
            return name
        }
        return "Unknown"
    }
    print("   分量 \(i+1): \(names)")
}

// ============================================
// 4. 社区检测
// ============================================
print("\n🏘️  社区检测...")

// 使用贪心算法
print("   🔍 使用贪心算法检测社区...")
let startTime1 = CFAbsoluteTimeGetCurrent()
let greedyCommunitiesDict = graph.detectCommunitiesGreedy()
let endTime1 = CFAbsoluteTimeGetCurrent()
print("   检测到 \(Set(greedyCommunitiesDict.values).count) 个社区 (耗时: \(String(format: "%.3f", endTime1 - startTime1))s)")

// 按社区ID分组
var communities: [Int: [VertexID]] = [:]
for (vid, communityId) in greedyCommunitiesDict {
    communities[communityId, default: []].append(vid)
}

// 转换为 [Set<VertexID>]
let communitySets = communities.values.map { Set($0) }

for (i, community) in communitySets.enumerated() {
    let names = community.map { vid -> String in
        if let vertex = graph.getVertex(id: vid),
           case .string(let name) = vertex.properties["name"] {
            return name
        }
        return "Unknown"
    }
    let jobs = community.compactMap { vid -> String? in
        if let vertex = graph.getVertex(id: vid),
           case .string(let job) = vertex.properties["job"] {
            return job
        }
        return nil
    }
    print("   社区 \(i+1): \(names) [职业: \(Set(jobs))]")
}

// ============================================
// 5. 可视化导出
// ============================================
print("\n🎨 导出可视化...")

// 导出 DOT 文件（按社区着色）
let dotPath = "/tmp/axolotl_tmp/social_network.dot"
do {
    try graph.saveToDOT(filePath: dotPath, directed: false, coloredByCommunity: true)
    print("   ✅ DOT 文件已导出: \(dotPath)")
} catch {
    print("   ❌ 导出失败: \(error)")
}

// ============================================
// 6. 保存和加载
// ============================================
print("\n💾 测试持久化...")

// 保存为 JSON
let jsonSavePath = "/tmp/axolotl_tmp/social_network.json"
do {
    try graph.save()
    print("   ✅ JSON 保存成功: \(jsonSavePath)")
} catch {
    print("   ❌ JSON 保存失败: \(error)")
}

// ============================================
// 完成
// ============================================
print("\n" + String(repeating: "=", count: 50))
print("✨ 演示完成!")
print("\n💡 提示:")
print("   - DOT 文件可以用 Graphviz 渲染: dot -Tpng social_network.dot -o social_network.png")
print("   - JSON 文件可以用其他工具可视化")
print("   - 图数据库功能演示完成")
