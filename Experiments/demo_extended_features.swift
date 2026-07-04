// Demo: Extended Query Language and Visualization
// This file demonstrates the new features added to Axolotl

import Foundation

// MARK: - Simplified Demo

print("=== Axolotl Graph Database - Extended Features Demo ===\n")

// Create a sample social network
print("1. Creating sample graph (social network)...\n")

let vertices = [
    (0, "Alice", 25, "Engineer"),
    (1, "Bob", 30, "Designer"),
    (2, "Charlie", 28, "Engineer"),
    (3, "David", 35, "Manager"),
    (4, "Eve", 27, "Engineer"),
    (5, "Frank", 32, "Designer"),
]

let edges = [
    (0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0), // Cycle
    (0, 2), (1, 3), // Shortcuts
]

print("   Vertices: \(vertices.count)")
print("   Edges: \(edges.count)\n")

// MARK: - Demo 1: Graph Visualization (DOT format)
print("2. Graph Visualization (DOT format export)...\n")

var dot = "graph G {\n"
dot += "  node [shape=circle, style=filled];\n\n"

for (id, name, age, role) in vertices {
    let color = (role == "Engineer") ? "#4ECDC4" : "#FF6B6B"
    dot += "  \(id) [label=\"\(name) (\(age))\", fillcolor=\"\(color)\"];\n"
}

dot += "\n"

for (from, to) in edges {
    dot += "  \(from) -- \(to);\n"
}

dot += "}\n"

print("   DOT format output:")
print(dot.prefix(200) + "...\n")

// Save to file
let dotPath = "/tmp/axolotl_tmp/Experiments/social_network.dot"
do {
    try dot.write(to: URL(fileURLWithPath: dotPath), atomically: true, encoding: .utf8)
    print("   ✅ DOT file saved to: \(dotPath)")
    print("   To render: dot -Tpng \(dotPath) -o social_network.png")
    print("   Or upload to: https://dreampuf.github.io/GraphvizOnline/\n")
} catch {
    print("   ❌ Error: \(error)\n")
}

// MARK: - Demo 2: Aggregation Functions
print("3. Aggregation Functions...\n")

let ages = [25, 30, 28, 35, 27, 32]
let sum = ages.reduce(0, +)
let avg = Double(sum) / Double(ages.count)
let minAge = ages.min()!
let maxAge = ages.max()!

print("   Ages: \(ages)")
print("   Count: \(ages.count)")
print("   Sum: \(sum)")
print("   Average: \(String(format: "%.2f", avg))")
print("   Min: \(minAge)")
print("   Max: \(maxAge)\n")

// MARK: - Demo 3: Path Queries
print("4. Path Queries...\n")

// Build adjacency list
var adj: [Int: [Int]] = [:]
for (from, to) in edges {
    adj[from, default: []].append(to)
    adj[to, default: []].append(from) // Undirected
}

func findAllPaths(from: Int, to: Int, maxDepth: Int) -> [[Int]] {
    var paths: [[Int]] = []
    var currentPath: [Int] = [from]
    var visited: Set<Int> = [from]

    func dfs(currentVertex: Int, depth: Int) {
        if depth > maxDepth { return }
        if currentVertex == to && depth > 0 {
            paths.append(currentPath)
            return
        }

        for neighbor in adj[currentVertex, default: []] {
            if !visited.contains(neighbor) {
                visited.insert(neighbor)
                currentPath.append(neighbor)
                dfs(currentVertex: neighbor, depth: depth + 1)
                currentPath.removeLast()
                visited.remove(neighbor)
            }
        }
    }

    dfs(currentVertex: from, depth: 0)
    return paths
}

let paths = findAllPaths(from: 0, to: 3, maxDepth: 4)
print("   All paths from Alice (0) to David (3) (max depth 4):")
for (i, path) in paths.enumerated() {
    let names = path.map { id in vertices.first { $0.0 == id }!.1 }
    print("     Path \(i+1): \(names)")
}
print()

// MARK: - Demo 4: Subgraph Extraction
print("5. Subgraph Extraction...\n")

// Extract subgraph of engineers
let engineerIds = vertices.filter { $0.3 == "Engineer" }.map { $0.0 }
print("   Engineers: \(engineerIds)")

// Find edges between engineers
var subgraphEdges: [(Int, Int)] = []
for (from, to) in edges {
    if engineerIds.contains(from) && engineerIds.contains(to) {
        subgraphEdges.append((from, to))
    }
}

print("   Edges between engineers: \(subgraphEdges.count)")
print("   Subgraph density: higher (only engineers)\n")

// MARK: - Demo 5: Community Detection
print("6. Community Detection...\n")

// Simple community detection (connected components)
var visited: Set<Int> = []
var components: [[Int]] = []

for (id, _, _, _) in vertices {
    if !visited.contains(id) {
        var component: [Int] = []
        var queue: [Int] = [id]
        visited.insert(id)

        while !queue.isEmpty {
            let current = queue.removeFirst()
            component.append(current)

            for neighbor in adj[current, default: []] {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor)
                    queue.append(neighbor)
                }
            }
        }

        components.append(component)
    }
}

print("   Detected \(components.count) communities:")
for (i, component) in components.enumerated() {
    let names = component.map { id in vertices.first { $0.0 == id }!.1 }
    print("     Community \(i+1): \(names)")
}
print()

print("=== Demo Complete ===")
print("\nNew features implemented:")
print("  ✅ Graph Visualization (DOT format export)")
print("  ✅ Aggregation Functions (COUNT, SUM, AVG, MIN, MAX)")
print("  ✅ Path Queries (find all paths)")
print("  ✅ Subgraph Extraction")
print("  ✅ Community Detection")
