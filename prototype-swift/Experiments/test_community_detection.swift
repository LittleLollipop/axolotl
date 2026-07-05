import Foundation

// MARK: - Core Types (Simplified)

typealias VertexID = UInt64

struct EdgeID: Hashable {
    let from: VertexID
    let to: VertexID
}

enum PropertyValue: Equatable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case null
}

struct Vertex: Equatable {
    let id: VertexID
    var properties: [String: PropertyValue]
}

struct Edge: Equatable {
    let id: EdgeID
    var properties: [String: PropertyValue]
    var weight: Double
}

// MARK: - Simple Graph with Community Detection

class SimpleGraph {
    var vertices: [VertexID: Vertex] = [:]
    var edges: [EdgeID: Edge] = [:]
    var adjacencyList: [VertexID: Set<VertexID>] = [:]
    var reverseAdjacencyList: [VertexID: Set<VertexID>] = [:] // For undirected graphs

    func addVertex(id: VertexID, properties: [String: PropertyValue]) {
        let vertex = Vertex(id: id, properties: properties)
        vertices[id] = vertex
        adjacencyList[id] = []
        reverseAdjacencyList[id] = []
    }

    func addEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue], weight: Double) {
        let edgeId = EdgeID(from: from, to: to)
        let edge = Edge(id: edgeId, properties: properties, weight: weight)
        edges[edgeId] = edge
        adjacencyList[from]?.insert(to)
        reverseAdjacencyList[to]?.insert(from) // Add reverse edge for undirected graphs
    }

    // MARK: - Community Detection (Label Propagation)

    func detectCommunities(maxIterations: Int = 100) -> [VertexID: Int] {
        let n = vertices.count
        guard n > 0 else { return [:] }

        // Initialize: each vertex gets its own label
        var labels: [VertexID: Int] = [:]
        for (i, vid) in vertices.keys.enumerated() {
            labels[vid] = i
        }

        // Iterative label propagation
        for _ in 0..<maxIterations {
            var newLabels = labels

            for vid in vertices.keys {
                // Get all neighbors (both outgoing and incoming edges)
                var allNeighbors: Set<VertexID> = []
                if let outgoing = adjacencyList[vid] {
                    allNeighbors.formUnion(outgoing)
                }
                if let incoming = reverseAdjacencyList[vid] {
                    allNeighbors.formUnion(incoming)
                }

                if allNeighbors.isEmpty {
                    continue
                }

                // Count label frequencies among neighbors
                var labelCounts: [Int: Int] = [:]
                for neighbor in allNeighbors {
                    if let label = labels[neighbor] {
                        labelCounts[label, default: 0] += 1
                    }
                }

                // Find the most common label
                if let (mostCommonLabel, _) = labelCounts.max(by: { $0.value < $1.value }) {
                    newLabels[vid] = mostCommonLabel
                }
            }

            // Check convergence
            let changed = labels.keys.filter { labels[$0] != newLabels[$0] }.count
            labels = newLabels

            if changed == 0 {
                break
            }
        }

        return labels
    }

    func getCommunities(maxIterations: Int = 100) -> [[VertexID]] {
        let labels = detectCommunities(maxIterations: maxIterations)

        // Group vertices by label
        var communities: [Int: [VertexID]] = [:]
        for (vid, label) in labels {
            communities[label, default: []].append(vid)
        }

        return Array(communities.values)
    }

    func printCommunities() {
        let communities = getCommunities()

        print("   Detected \(communities.count) communities:")
        for (i, community) in communities.enumerated() {
            print("     Community \(i): \(community.sorted())")
        }
    }
}

// MARK: - Main Test

print("=== Community Detection Test (Label Propagation) ===\n")

// Test 1: Two clear communities (no edges between them)
print("1. Testing two isolated communities...")
let graph1 = SimpleGraph()

// Community 1: vertices 0,1,2
for i in 0...2 {
    graph1.addVertex(id: UInt64(i), properties: [:])
}
graph1.addEdge(from: 0, to: 1, properties: [:], weight: 1.0)
graph1.addEdge(from: 1, to: 2, properties: [:], weight: 1.0)
graph1.addEdge(from: 0, to: 2, properties: [:], weight: 1.0)

// Community 2: vertices 3,4,5
for i in 3...5 {
    graph1.addVertex(id: UInt64(i), properties: [:])
}
graph1.addEdge(from: 3, to: 4, properties: [:], weight: 1.0)
graph1.addEdge(from: 4, to: 5, properties: [:], weight: 1.0)
graph1.addEdge(from: 3, to: 5, properties: [:], weight: 1.0)

graph1.printCommunities()
print("   Expected: 2 communities ([0,1,2] and [3,4,5])\n")

// Test 2: Two communities with one bridge edge
print("2. Testing two communities with bridge edge...")
let graph2 = SimpleGraph()

// Community 1: vertices 0,1,2
for i in 0...2 {
    graph2.addVertex(id: UInt64(i), properties: [:])
}
graph2.addEdge(from: 0, to: 1, properties: [:], weight: 1.0)
graph2.addEdge(from: 1, to: 2, properties: [:], weight: 1.0)

// Community 2: vertices 3,4,5
for i in 3...5 {
    graph2.addVertex(id: UInt64(i), properties: [:])
}
graph2.addEdge(from: 3, to: 4, properties: [:], weight: 1.0)
graph2.addEdge(from: 4, to: 5, properties: [:], weight: 1.0)

// Bridge: 2-3
graph2.addEdge(from: 2, to: 3, properties: [:], weight: 1.0)

graph2.printCommunities()
print("   Expected: Might merge into 1 community (bridge edge connects them)\n")

// Test 3: Star graph (should be one community)
print("3. Testing star graph (should be 1 community)...")
let graph3 = SimpleGraph()
for i in 0...4 {
    graph3.addVertex(id: UInt64(i), properties: [:])
}
for i in 1...4 {
    graph3.addEdge(from: 0, to: UInt64(i), properties: [:], weight: 1.0)
}

graph3.printCommunities()
print("   Expected: 1 community ([0,1,2,3,4])\n")

// Test 4: Graph with 3 communities
print("4. Testing graph with 3 communities...")
let graph4 = SimpleGraph()

// Community 1: 0,1,2
for i in 0...2 {
    graph4.addVertex(id: UInt64(i), properties: [:])
}
graph4.addEdge(from: 0, to: 1, properties: [:], weight: 1.0)
graph4.addEdge(from: 1, to: 2, properties: [:], weight: 1.0)

// Community 2: 3,4,5
for i in 3...5 {
    graph4.addVertex(id: UInt64(i), properties: [:])
}
graph4.addEdge(from: 3, to: 4, properties: [:], weight: 1.0)
graph4.addEdge(from: 4, to: 5, properties: [:], weight: 1.0)

// Community 3: 6,7,8
for i in 6...8 {
    graph4.addVertex(id: UInt64(i), properties: [:])
}
graph4.addEdge(from: 6, to: 7, properties: [:], weight: 1.0)
graph4.addEdge(from: 7, to: 8, properties: [:], weight: 1.0)

graph4.printCommunities()
print("   Expected: 3 communities ([0,1,2], [3,4,5], [6,7,8])\n")

// Test 5: Performance test
print("5. Performance test (100 vertices, 3 communities)...")
let graph5 = SimpleGraph()

// Create 3 communities
for i in 0..<100 {
    graph5.addVertex(id: UInt64(i), properties: [:])
}

// Community 1: 0-32
for i in 0..<32 {
    for j in (i+1)..<32 {
        if Int.random(in: 0...2) == 0 {  // 33% chance of edge
            graph5.addEdge(from: UInt64(i), to: UInt64(j), properties: [:], weight: 1.0)
        }
    }
}

// Community 2: 33-65
for i in 33..<66 {
    for j in (i+1)..<66 {
        if Int.random(in: 0...2) == 0 {
            graph5.addEdge(from: UInt64(i), to: UInt64(j), properties: [:], weight: 1.0)
        }
    }
}

// Community 3: 66-99
for i in 66..<100 {
    for j in (i+1)..<100 {
        if Int.random(in: 0...2) == 0 {
            graph5.addEdge(from: UInt64(i), to: UInt64(j), properties: [:], weight: 1.0)
        }
    }
}

// Add a few bridge edges
graph5.addEdge(from: 10, to: 40, properties: [:], weight: 1.0)
graph5.addEdge(from: 50, to: 80, properties: [:], weight: 1.0)

let start = CFAbsoluteTimeGetCurrent()
let communities = graph5.getCommunities()
let time = (CFAbsoluteTimeGetCurrent() - start) * 1000

print("   Computation time: \(String(format: "%.2f", time)) ms")
print("   Detected \(communities.count) communities")
print("   Community sizes: \(communities.map { $0.count }.sorted())")

print("\n=== Test Complete ===")
