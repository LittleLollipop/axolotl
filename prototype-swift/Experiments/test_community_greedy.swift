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
    var reverseAdjacencyList: [VertexID: Set<VertexID>] = [:]

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
        reverseAdjacencyList[to]?.insert(from)
    }

    // MARK: - Community Detection (Greedy Modularity Optimization)

    func detectCommunitiesGreedy(maxIterations: Int = 100) -> [VertexID: Int] {
        let n = vertices.count
        guard n > 0 else { return [:] }

        // Initialize: each vertex is its own community
        var communities: [VertexID: Int] = [:]
        var communityIds = Array(vertices.keys)
        for (i, vid) in communityIds.enumerated() {
            communities[vid] = i
        }

        // Compute initial modularity
        var bestModularity = computeModularity(communities: communities)

        // Iteratively merge communities
        for _ in 0..<min(maxIterations, n) {
            var bestMerge: (Int, Int, Double)? = nil

            // Try all pairs of communities
            let uniqueCommunities = Set(communities.values)
            let communityList = Array(uniqueCommunities)

            for i in 0..<communityList.count {
                for j in (i+1)..<communityList.count {
                    let c1 = communityList[i]
                    let c2 = communityList[j]

                    // Try merging c1 and c2
                    var testCommunities = communities
                    for vid in testCommunities.keys {
                        if testCommunities[vid] == c2 {
                            testCommunities[vid] = c1
                        }
                    }

                    let modularity = computeModularity(communities: testCommunities)

                    if modularity > bestModularity {
                        bestModularity = modularity
                        bestMerge = (c1, c2, modularity)
                    }
                }
            }

            // Apply best merge
            if let (c1, c2, _) = bestMerge {
                for vid in communities.keys {
                    if communities[vid] == c2 {
                        communities[vid] = c1
                    }
                }
            } else {
                break  // No improvement
            }
        }

        // Renumber communities consecutively
        let uniqueCommunities = Set(communities.values)
        let sortedCommunities = uniqueCommunities.sorted()
        var communityMapping: [Int: Int] = [:]
        for (i, c) in sortedCommunities.enumerated() {
            communityMapping[c] = i
        }

        for vid in communities.keys {
            communities[vid] = communityMapping[communities[vid]!]!
        }

        return communities
    }

    /// Compute modularity of current community assignment
    private func computeModularity(communities: [VertexID: Int]) -> Double {
        let m = Double(edges.count * 2)  // Total number of undirected edges (count both directions)
        guard m > 0 else { return 0.0 }

        var Q = 0.0

        // For each pair of vertices
        for (vid, community) in communities {
            guard let neighbors = adjacencyList[vid] else { continue }

            for neighbor in neighbors {
                guard let neighborCommunity = communities[neighbor] else { continue }

                // Fraction of edges within community
                let ki = Double(adjacencyList[vid]?.count ?? 0)
                let kj = Double(adjacencyList[neighbor]?.count ?? 0)

                let delta = (community == neighborCommunity) ? 1.0 : 0.0
                let expected = (ki * kj) / (2.0 * m)

                Q += (delta - expected) / (2.0 * m)
            }
        }

        return Q
    }

    /// Get communities (grouped by label) using greedy optimization
    func getCommunitiesGreedy(maxIterations: Int = 100) -> [[VertexID]] {
        let communities = detectCommunitiesGreedy(maxIterations: maxIterations)

        // Group vertices by community
        var result: [Int: [VertexID]] = [:]
        for (vid, community) in communities {
            result[community, default: []].append(vid)
        }

        return Array(result.values)
    }

    func printCommunities() {
        let communities = getCommunitiesGreedy()

        print("   Detected \(communities.count) communities (modularity: \(String(format: "%.4f", computeModularity(communities: detectCommunitiesGreedy())))")
        for (i, community) in communities.enumerated() {
            print("     Community \(i): \(community.sorted())")
        }
    }
}

// MARK: - Main Test

print("=== Community Detection Test (Greedy Modularity) ===\n")

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
print("   Expected: Might be 1 or 2 communities (bridge edge)\n")

// Test 3: Star graph (should be 1 community)
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
print("5. Performance test (20 vertices, dense within communities)...")
let graph5 = SimpleGraph()

// Create 2 communities of 10 vertices each
for i in 0..<20 {
    graph5.addVertex(id: UInt64(i), properties: [:])
}

// Community 1: 0-9 (dense)
for i in 0..<10 {
    for j in (i+1)..<10 {
        if Int.random(in: 0...2) == 0 {  // 33% chance of edge
            graph5.addEdge(from: UInt64(i), to: UInt64(j), properties: [:], weight: 1.0)
        }
    }
}

// Community 2: 10-19 (dense)
for i in 10..<20 {
    for j in (i+1)..<20 {
        if Int.random(in: 0...2) == 0 {
            graph5.addEdge(from: UInt64(i), to: UInt64(j), properties: [:], weight: 1.0)
        }
    }
}

// Add a few bridge edges
graph5.addEdge(from: 5, to: 15, properties: [:], weight: 1.0)

let start = CFAbsoluteTimeGetCurrent()
let communities = graph5.getCommunitiesGreedy()
let time = (CFAbsoluteTimeGetCurrent() - start) * 1000

print("   Computation time: \(String(format: "%.2f", time)) ms")
print("   Detected \(communities.count) communities")
print("   Community sizes: \(communities.map { $0.count }.sorted())")

print("\n=== Test Complete ===")
