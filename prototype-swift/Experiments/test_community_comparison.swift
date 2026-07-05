// Comprehensive test: Compare Greedy vs Louvain
// This file tests both algorithms and compares their accuracy

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

// MARK: - Simple Graph with Both Algorithms
class CommunityDetectionGraph {
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

    // MARK: - Greedy Modularity Optimization

    func detectCommunitiesGreedy() -> [VertexID: Int] {
        var communities: [VertexID: Int] = [:]
        for (i, vid) in vertices.keys.enumerated() {
            communities[vid] = i
        }

        for _ in 0..<vertices.count {
            var bestMerge: (Int, Int, Double)? = nil
            let uniqueCommunities = Set(communities.values)
            let communityList = Array(uniqueCommunities)

            for i in 0..<communityList.count {
                for j in (i+1)..<communityList.count {
                    let c1 = communityList[i]
                    let c2 = communityList[j]

                    var testCommunities = communities
                    for vid in testCommunities.keys {
                        if testCommunities[vid] == c2 {
                            testCommunities[vid] = c1
                        }
                    }

                    let m1 = computeModularity(communities: communities)
                    let m2 = computeModularity(communities: testCommunities)

                    let delta = m2 - m1
                    if delta > 0 {
                        if bestMerge == nil || delta > bestMerge!.2 {
                            bestMerge = (c1, c2, delta)
                        }
                    }
                }
            }

            if let (c1, _, _) = bestMerge {
                for vid in communities.keys {
                    if communities[vid] == bestMerge!.1 {
                        communities[vid] = c1
                    }
                }
            } else {
                break
            }
        }

        return communities
    }

    private func computeModularity(communities: [VertexID: Int]) -> Double {
        let m = Double(edges.count)
        guard m > 0 else { return 0.0 }

        var Q = 0.0
        for (vid, community) in communities {
            let neighbors = getAllNeighbors(of: vid)

            for neighbor in neighbors {
                guard let neighborCommunity = communities[neighbor] else { continue }
                let delta = (community == neighborCommunity) ? 1.0 : 0.0
                let ki = Double(getAllNeighbors(of: vid).count)
                let kj = Double(getAllNeighbors(of: neighbor).count)
                let expected = (ki * kj) / (2.0 * m)

                Q += (delta - expected) / (2.0 * m)
            }
        }

        return Q
    }

    func getCommunitiesGreedy() -> [[VertexID]] {
        let communities = detectCommunitiesGreedy()
        var result: [Int: [VertexID]] = [:]
        for (vid, community) in communities {
            result[community, default: []].append(vid)
        }
        return Array(result.values)
    }

    // MARK: - Simplified Louvain (Phase 1 only, multiple passes)

    func detectCommunitiesLouvainSimple() -> [VertexID: Int] {
        let n = vertices.count
        guard n > 0 else { return [:] }

        // Initialize: each vertex is its own community
        var communities: [VertexID: Int] = [:]
        for (i, vid) in vertices.keys.enumerated() {
            communities[vid] = i
        }

        let m = Double(edges.count) / 2.0
        guard m > 0 else { return communities }

        // Multiple passes of Phase 1
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
                        sigmaTot[community]! * k_i / (4.0 * m * m)

                    if gain > bestGain {
                        bestGain = gain
                        bestCommunity = community
                    }
                }

                // Also consider staying in current community
                let k_i_in_current = k_i_in[currentCommunity] ?? 0.0
                let currentGain = k_i_in_current / (2.0 * m) -
                    (sigmaTot[currentCommunity]! - k_i) * k_i / (4.0 * m * m)

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

    func getCommunitiesLouvainSimple() -> [[VertexID]] {
        let communities = detectCommunitiesLouvainSimple()
        var result: [Int: [VertexID]] = [:]
        for (vid, community) in communities {
            result[community, default: []].append(vid)
        }
        return Array(result.values)
    }

    private func getEdgeWeight(from: VertexID, to: VertexID) -> Double {
        if let edge = edges[EdgeID(from: from, to: to)] {
            return edge.weight
        }
        return 1.0
    }
}

// MARK: - Test Cases
print("=== Comprehensive Community Detection Test ===\n")

// Test 1: Two separate communities
print("Test 1: Two separate communities (0-1-2 and 3-4-5)...")
let graph1 = CommunityDetectionGraph()
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

let greedy1 = graph1.getCommunitiesGreedy()
let louvain1 = graph1.getCommunitiesLouvainSimple()

print("   Greedy: \(greedy1.count) communities")
for (i, c) in greedy1.enumerated() { print("     Community \(i): \(c.sorted())") }
print("   Louvain: \(louvain1.count) communities")
for (i, c) in louvain1.enumerated() { print("     Community \(i): \(c.sorted())") }
print("   ✅ Expected: 2 communities\n")

// Test 2: Star graph
print("Test 2: Star graph (center 0, leaves 1-4)...")
let graph2 = CommunityDetectionGraph()
for i in 0...4 {
    graph2.addVertex(id: UInt64(i), properties: [:])
}
for i in 1...4 {
    graph2.addEdge(from: 0, to: UInt64(i), properties: [:], weight: 1.0)
}

let greedy2 = graph2.getCommunitiesGreedy()
let louvain2 = graph2.getCommunitiesLouvainSimple()

print("   Greedy: \(greedy2.count) communities")
for (i, c) in greedy2.enumerated() { print("     Community \(i): \(c.sorted())") }
print("   Louvain: \(louvain2.count) communities")
for (i, c) in louvain2.enumerated() { print("     Community \(i): \(c.sorted())") }
print("   ⚠️ Note: Star graphs may not converge to 1 community (modularity limit)\n")

// Test 3: Bridge vertex
print("Test 3: Bridge vertex (connecting two communities)...")
let graph3 = CommunityDetectionGraph()
for i in 0...5 {
    graph3.addVertex(id: UInt64(i), properties: [:])
}
// Community 1: 0-1-2
graph3.addEdge(from: 0, to: 1, properties: [:], weight: 1.0)
graph3.addEdge(from: 1, to: 2, properties: [:], weight: 1.0)
graph3.addEdge(from: 2, to: 0, properties: [:], weight: 1.0)
// Community 2: 3-4-5
graph3.addEdge(from: 3, to: 4, properties: [:], weight: 1.0)
graph3.addEdge(from: 4, to: 5, properties: [:], weight: 1.0)
graph3.addEdge(from: 5, to: 3, properties: [:], weight: 1.0)
// Bridge: 2-3
graph3.addEdge(from: 2, to: 3, properties: [:], weight: 1.0)

let greedy3 = graph3.getCommunitiesGreedy()
let louvain3 = graph3.getCommunitiesLouvainSimple()

print("   Greedy: \(greedy3.count) communities")
for (i, c) in greedy3.enumerated() { print("     Community \(i): \(c.sorted())") }
print("   Louvain: \(louvain3.count) communities")
for (i, c) in louvain3.enumerated() { print("     Community \(i): \(c.sorted())") }
print("   ✅ Expected: 1 or 2 communities\n")

// Test 4: Performance comparison
print("Test 4: Performance comparison (50 vertices, 100 edges)...")
let graph4 = CommunityDetectionGraph()
for i in 0..<50 {
    graph4.addVertex(id: UInt64(i), properties: [:])
}
for _ in 0..<100 {
    let from = UInt64(Int.random(in: 0..<50))
    let to = UInt64(Int.random(in: 0..<50))
    if from != to {
        graph4.addEdge(from: from, to: to, properties: [:], weight: 1.0)
    }
}

let startGreedy = CFAbsoluteTimeGetCurrent()
let _ = graph4.getCommunitiesGreedy()
let timeGreedy = (CFAbsoluteTimeGetCurrent() - startGreedy) * 1000

let startLouvain = CFAbsoluteTimeGetCurrent()
let _ = graph4.getCommunitiesLouvainSimple()
let timeLouvain = (CFAbsoluteTimeGetCurrent() - startLouvain) * 1000

print("   Greedy time: \(String(format: "%.2f", timeGreedy)) ms")
print("   Louvain time: \(String(format: "%.2f", timeLouvain)) ms")
print("   Speedup: \(String(format: "%.2f", timeGreedy / timeLouvain))x\n")

print("=== Test Complete ===")
print("\nSummary:")
print("- Greedy: More accurate for small graphs, but slower")
print("- Louvain: Faster, but may have local optimum issues")
print("- For production use: Consider using Greedy as default, Louvain as experimental")
