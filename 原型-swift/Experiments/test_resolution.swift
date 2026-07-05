// Test resolution parameter for Louvain algorithm
// This file tests whether lowering the resolution parameter helps with star graphs

import Foundation

// MARK: - Core Types
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
struct Vertex {
    let id: VertexID
    var properties: [String: PropertyValue]
}
struct Edge {
    let id: EdgeID
    var properties: [String: PropertyValue]
    var weight: Double
}

// MARK: - Simple Graph with Resolution Parameter
class ResolutionGraph {
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

    func getAllNeighbors(of vertex: VertexID) -> Set<VertexID> {
        var neighbors: Set<VertexID> = []
        if let outgoing = adjacencyList[vertex] {
            neighbors.formUnion(outgoing)
        }
        if let incoming = reverseAdjacencyList[vertex] {
            neighbors.formUnion(incoming)
        }
        return neighbors
    }

    func getEdgeWeight(from: VertexID, to: VertexID) -> Double {
        if let edge = edges[EdgeID(from: from, to: to)] {
            return edge.weight
        }
        return 1.0
    }

    // MARK: - Louvain with Resolution Parameter

    func detectCommunitiesLouvain(resolution: Double = 1.0) -> [VertexID: Int] {
        let n = vertices.count
        guard n > 0 else { return [:] }

        // Initialize: each vertex is its own community
        var communities: [VertexID: Int] = [:]
        for (i, vid) in vertices.keys.enumerated() {
            communities[vid] = i
        }

        let m = Double(edges.count) / 2.0
        guard m > 0 else { return communities }

        // Multiple passes
        for _ in 0..<100 {
            var improved = false

            for vid in vertices.keys {
                let neighbors = getAllNeighbors(of: vid)
                if neighbors.isEmpty { continue }

                let currentCommunity = communities[vid]!
                let k_i = Double(neighbors.count)

                // Compute sigma_tot for each community
                var sigmaTot: [Int: Double] = [:]
                for (otherVid, community) in communities {
                    let degree = Double(getAllNeighbors(of: otherVid).count)
                    sigmaTot[community, default: 0.0] += degree
                }

                // Compute k_i,in for each community
                var k_i_in: [Int: Double] = [:]
                for neighbor in neighbors {
                    let neighborCommunity = communities[neighbor]!
                    let weight = getEdgeWeight(from: vid, to: neighbor)
                    k_i_in[neighborCommunity, default: 0.0] += weight
                }

                // Find best community
                var bestCommunity = currentCommunity
                var bestGain = 0.0

                for (community, k_i_in_val) in k_i_in {
                    if community == currentCommunity { continue }

                    let gain = k_i_in_val / (2.0 * m) -
                        sigmaTot[community]! * k_i / (4.0 * m * m * resolution)

                    if gain > bestGain {
                        bestGain = gain
                        bestCommunity = community
                    }
                }

                // Also consider staying in current community
                let k_i_in_current = k_i_in[currentCommunity] ?? 0.0
                let currentGain = k_i_in_current / (2.0 * m) -
                    (sigmaTot[currentCommunity]! - k_i) * k_i / (4.0 * m * m * resolution)

                if bestGain > currentGain && bestGain > 0 {
                    communities[vid] = bestCommunity
                    improved = true
                }
            }

            // Check convergence
            let uniqueCommunities = Set(communities.values)
            if uniqueCommunities.count == vertices.count {
                break
            }
        }

        return communities
    }

    func getCommunitiesLouvain(resolution: Double = 1.0) -> [[VertexID]] {
        let communities = detectCommunitiesLouvain(resolution: resolution)
        var result: [Int: [VertexID]] = [:]
        for (vid, community) in communities {
            result[community, default: []].append(vid)
        }
        return Array(result.values)
    }
}

// MARK: - Test Cases
print("=== Testing Resolution Parameter for Louvain ===\n")

// Test 1: Star graph with different resolution values
print("Test 1: Star graph (center 0, leaves 1-4) with different resolution values...\n")

let graph = ResolutionGraph()
for i in 0...4 {
    graph.addVertex(id: UInt64(i), properties: [:])
}
for i in 1...4 {
    graph.addEdge(from: 0, to: UInt64(i), properties: [:], weight: 1.0)
}

print("   Resolution = 1.0 (default):")
let communities1 = graph.getCommunitiesLouvain(resolution: 1.0)
print("     Detected \(communities1.count) communities:")
for (i, community) in communities1.enumerated() {
    print("       Community \(i): \(community.sorted())")
}
print("     ✅ Expected: 1 community (but may get stuck in local optimum)\n")

print("   Resolution = 0.5 (prefer larger communities):")
let communities2 = graph.getCommunitiesLouvain(resolution: 0.5)
print("     Detected \(communities2.count) communities:")
for (i, community) in communities2.enumerated() {
    print("       Community \(i): \(community.sorted())")
}
print("     ✅ Expected: 1 community (more likely with lower resolution)\n")

print("   Resolution = 0.1 (strongly prefer larger communities):")
let communities3 = graph.getCommunitiesLouvain(resolution: 0.1)
print("     Detected \(communities3.count) communities:")
for (i, community) in communities3.enumerated() {
    print("       Community \(i): \(community.sorted())")
}
print("     ✅ Expected: 1 community (should work with very low resolution)\n")

// Test 2: Two separate communities with different resolution values
print("Test 2: Two separate communities (0-1-2 and 3-4-5) with different resolution values...\n")

let graph2 = ResolutionGraph()
for i in 0...5 {
    graph2.addVertex(id: UInt64(i), properties: [:])
}
// Community 1: 0-1-2
graph2.addEdge(from: 0, to: 1, properties: [:], weight: 1.0)
graph2.addEdge(from: 1, to: 2, properties: [:], weight: 1.0)
graph2.addEdge(from: 2, to: 0, properties: [:], weight: 1.0)
// Community 2: 3-4-5
graph2.addEdge(from: 3, to: 4, properties: [:], weight: 1.0)
graph2.addEdge(from: 4, to: 5, properties: [:], weight: 1.0)
graph2.addEdge(from: 5, to: 3, properties: [:], weight: 1.0)

print("   Resolution = 1.0 (default):")
let communities4 = graph2.getCommunitiesLouvain(resolution: 1.0)
print("     Detected \(communities4.count) communities:")
for (i, community) in communities4.enumerated() {
    print("       Community \(i): \(community.sorted())")
}
print("     ✅ Expected: 2 communities\n")

print("   Resolution = 2.0 (prefer smaller communities):")
let communities5 = graph2.getCommunitiesLouvain(resolution: 2.0)
print("     Detected \(communities5.count) communities:")
for (i, community) in communities5.enumerated() {
    print("       Community \(i): \(community.sorted())")
}
print("     ✅ Expected: 2 or more communities (higher resolution may split communities)\n")

print("=== Test Complete ===")
print("\nSummary:")
print("- Resolution < 1.0: Prefer larger communities (may help with star graphs)")
print("- Resolution = 1.0: Standard modularity (default)")
print("- Resolution > 1.0: Prefer smaller communities (may split communities)")
print("\n⚠️ Note: Resolution parameter may not always fix local optimum issues.")
print("   For star graphs, consider using Greedy Modularity Optimization instead.")
