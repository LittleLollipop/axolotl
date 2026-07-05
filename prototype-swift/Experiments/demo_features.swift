import Foundation

// MARK: - Main Demo

print("=== Axolotl Graph Database - Feature Demo ===\n")

// Create a sample graph
print("📊 Creating sample social network...")
let graph = PersistentGraph(filePath: "/tmp/demo_graph.json")

// Add vertices (users)
let users = [
    (1, "Alice", 30, true),
    (2, "Bob", 25, false),
    (3, "Charlie", 35, true),
    (4, "Diana", 28, true),
    (5, "Eve", 32, false)
]

for (id, name, age, active) in users {
    graph.addVertex(id: UInt64(id), properties: [
        "name": .string(name),
        "age": .int(age),
        "active": .bool(active)
    ])
}

// Add edges (relationships)
let relationships = [
    (1, 2, "knows"),
    (2, 3, "works_with"),
    (3, 4, "knows"),
    (4, 5, "knows"),
    (5, 1, "knows"),
    (1, 3, "friend")
]

for (from, to, type) in relationships {
    graph.addEdge(from: UInt64(from), to: UInt64(to), properties: ["type": .string(type)], weight: 1.0)
}

print("   Added \(users.count) users and \(relationships.count) relationships\n")

// MARK: - Demo 1: Betweenness Centrality

print("1️⃣ Betweenness Centrality")
print("   Measuring which users are 'bridges' in the network...\n")

let betweenness = graph.getBetweennessCentrality()
let sortedBetweenness = betweenness.sorted(by: { $0.value > $1.value })

print("   Top 3 most important users:")
for (id, score) in sortedBetweenness.prefix(3) {
    if let name = graph.getVertex(id: id)?.properties["name"] {
        if case .string(let nameStr) = name {
            print("     \(nameStr) (ID: \(id)): \(String(format: "%.4f", score))")
        }
    }
}
print()

// MARK: - Demo 2: Community Detection

print("2️⃣ Community Detection")
print("   Detecting user communities...\n")

let communities = graph.getCommunitiesGreedy()
print("   Found \(communities.count) communities:")
for (i, community) in communities.enumerated() {
    let names = community.compactMap { vid in
        graph.getVertex(id: vid)?.properties["name"].map { value in
            if case .string(let name) = value { return name }
            return nil
        }
    }
    print("     Community \(i): \(names.joined(separator: ", "))")
}
print()

// MARK: - Demo 3: Graph Query Language

print("3️⃣ Graph Query Language (Cypher-like)")
print("   Running sample queries...\n")

// Query 1: Find active users
print("   Query 1: Find all active users")
let activeUsers = graph.query()
    .matchVertex(alias: "v")
    .where(property: "active", equals: .bool(true))
    .execute()

print("   Result: \(activeUsers.count) users")
for vid in activeUsers {
    if let name = graph.getVertex(id: vid)?.properties["name"] {
        if case .string(let nameStr) = name {
            print("     - \(nameStr)")
        }
    }
}
print()

// Query 2: Find neighbors of Alice
print("   Query 2: Find friends of Alice (ID: 1)")
let friends = graph.findNeighbors(of: 1)
print("   Result: \(friends.count) friends")
for vid in friends {
    if let name = graph.getVertex(id: vid)?.properties["name"] {
        if case .string(let nameStr) = name {
            print("     - \(nameStr)")
        }
    }
}
print()

// Query 3: Find shortest path
print("   Query 3: Find shortest path from Alice to Eve")
if let path = graph.shortestPath(from: 1, to: 5) {
    let pathNames = path.compactMap { vid in
        graph.getVertex(id: vid)?.properties["name"].map { value in
            if case .string(let name) = value { return name }
            return nil
        }
    }
    print("   Result: \(pathNames.joined(separator: " → "))")
} else {
    print("   Result: No path found")
}
print()

// Query 4: Find users in the same community as Alice
print("   Query 4: Find users in Alice's community")
let aliceCommunity = graph.findCommunity(of: 1)
print("   Result: \(aliceCommunity.count) users in same community")
for vid in aliceCommunity {
    if let name = graph.getVertex(id: vid)?.properties["name"] {
        if case .string(let nameStr) = name {
            print("     - \(nameStr)")
        }
    }
}
print()

// MARK: - Demo 4: Persistence

print("4️⃣ Persistence (JSON & Binary)")
print("   Saving graph to disk...\n")

// Save JSON
let jsonStart = CFAbsoluteTimeGetCurrent()
try graph.save()
let jsonTime = (CFAbsoluteTimeGetCurrent() - jsonStart) * 1000
print("   JSON save time: \(String(format: "%.2f", jsonTime)) ms")

// Save Binary
let binaryStart = CFAbsoluteTimeGetCurrent()
try graph.saveBinary()
let binaryTime = (CFAbsoluteTimeGetCurrent() - binaryStart) * 1000
print("   Binary save time: \(String(format: "%.2f", binaryTime)) ms")

// File sizes
let fileManager = FileManager.default
let jsonSize = try fileManager.attributesOfItem(atPath: "/tmp/demo_graph.json")[.size] as! UInt64
let binarySize = try fileManager.attributesOfItem(atPath: "/tmp/demo_graph.bin")[.size] as! UInt64

print("   JSON file size: \(jsonSize) bytes")
print("   Binary file size: \(binarySize) bytes")
print("   Size reduction: \(String(format: "%.2f", Double(jsonSize) / Double(binarySize)))x")
print()

// MARK: - Summary

print("=== Demo Complete ===\n")
print("✅ Features Demonstrated:")
print("   1. Betweenness Centrality - Identify important 'bridge' users")
print("   2. Community Detection - Find user groups/communities")
print("   3. Graph Query Language - Cypher-like queries")
print("   4. Persistence - JSON & Binary formats")
print()
print("📊 Axolotl Graph Database - Ready for use!")
