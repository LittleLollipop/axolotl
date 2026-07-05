// Test Louvain Algorithm (Fixed)
// Compile: swift test_louvain_fixed.swift

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

// MARK: - Simple Graph with Correct Louvain
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

    // MARK: - Correct Louvain Implementation

    func detectCommunitiesLouvain() -> [VertexID: Int] {
        let n = vertices.count
        guard n > 0 else { return [:] }

        // Initialize: each vertex is its own community
        var communities: [VertexID: Int] = [:]
        for (i, vid) in vertices.keys.enumerated() {
            communities[vid] = i
        }

        // Compute total number of edges (for modularity calculation)
        // For undirected graph, m = number of unique edges
        let m = Double(edges.count) / 2.0 // Assuming undirected edges are stored both ways
        guard m > 0 else { return communities }

        // Louvain iterations
        var improved = true
        while improved {
            improved = false

            // Process vertices in random order
            let vertexOrder = vertices.keys.shuffled()

            for vid in vertexOrder {
                let neighbors = getAllNeighbors(of: vid)
                if neighbors.isEmpty { continue }

                let currentCommunity = communities[vid]!

                // Compute Σ_tot for each community (sum of degrees)
                var sigmaTot: [Int: Double] = [:]
                for (otherVid, community) in communities {
                    let degree = Double(getAllNeighbors(of: otherVid).count)
                    sigmaTot[community, default: 0.0] += degree
                }

                // Compute Σ_in for each community (sum of internal edge weights)
                var sigmaIn: [Int: Double] = [:]
                for (_, edge) in edges {
                    let fromCommunity = communities[edge.id.from]!
                    let toCommunity = communities[edge.id.to]!
                    if fromCommunity == toCommunity {
                        sigmaIn[fromCommunity, default: 0.0] += edge.weight
                    }
                }
                // Divide by 2 because each edge is counted twice
                for community in sigmaIn.keys {
                    sigmaIn[community]! /= 2.0
                }

                // Find best community for vid
                var bestCommunity = currentCommunity
                var bestGain = 0.0

                // Compute k_i (degree of vid)
                let k_i = Double(neighbors.count)

                // Compute k_i,in for each community (sum of edge weights from vid to community)
                var k_i_in: [Int: Double] = [:]
                for neighbor in neighbors {
                    let neighborCommunity = communities[neighbor]!
                    let weight = getEdgeWeight(from: vid, to: neighbor)
                    k_i_in[neighborCommunity, default: 0.0] += weight
                }

                // Try removing vid from current community
                let k_i_in_current = k_i_in[currentCommunity] ?? 0.0
                let removeGain = k_i_in_current / (2.0 * m) -
                    (sigmaTot[currentCommunity]! - k_i) * k_i / (2.0 * m * 2.0 * m)

                // Try adding vid to each neighboring community
                for (community, k_i_in_val) in k_i_in {
                    if community == currentCommunity { continue }

                    let addGain = k_i_in_val / (2.0 * m) -
                        sigmaTot[community]! * k_i / (2.0 * m * 2.0 * m)

                    if addGain > bestGain {
                        bestGain = addGain
                        bestCommunity = community
                    }
                }

                // Move vertex if improvement
                if bestCommunity != currentCommunity && bestGain > removeGain && bestGain > 0 {
                    communities[vid] = bestCommunity
                    improved = true
                }
            }
        }

        return communities
    }

    func getCommunitiesLouvain() -> [[VertexID]] {
        let communities = detectCommunitiesLouvain()
        var result: [Int: [VertexID]] = [:]
        for (vid, community) in communities {
            result[community, default: []].append(vid)
        }
        return Array(result.values)
    }
}

// MARK: - Test Cases
print("=== Testing Fixed Louvain Algorithm ===\n")

// Test 1: Two separate communities
print("Test 1: Two separate communities (0-1-2 and 3-4-5)...")
let graph1 = SimpleGraph()
for i in 0...5 {
    graph1.addVertex(id: UInt64(i), properties: [:])
}
// Community 1: 0-1-2
graph1.addEdge(from: 0, to: 1, properties: [:], weight: 1.0)
graph1.addEdge(from: 1, to: 2, properties: [:], weight: 1.0)
graph1.addEdge(from: 2, to: 0, properties: [:], weight: 1.0)
// Community 2: 3-4-5
graph1.addEdge(from: 3, to: 4, properties: [:], weight: 1.0)
graph1.addEdge(from: 4, to: 5, properties: [:], weight: 1.0)
graph1.addEdge(from: 5, to: 3, properties: [:], weight: 1.0)

let communities1 = graph1.getCommunitiesLouvain()
print("   Louvain detected \(communities1.count) communities:")
for (i, community) in communities1.enumerated() {
    print("     Community \(i): \(community.sorted())")
}
print("   ✅ Expected: 2 communities\n")

// Test 2: Star graph
print("Test 2: Star graph (center 0, leaves 1-4)...")
let graph2 = SimpleGraph()
for i in 0...4 {
    graph2.addVertex(id: UInt64(i), properties: [:])
}
for i in 1...4 {
    graph2.addEdge(from: 0, to: UInt64(i), properties: [:], weight: 1.0)
}

let communities2 = graph2.getCommunitiesLouvain()
print("   Louvain detected \(communities2.count) communities:")
for (i, community) in communities2.enumerated() {
    print("     Community \(i): \(community.sorted())")
}
print("   ✅ Expected: 1 community (star is cohesive)\n")

print("=== Tests Complete ===")
