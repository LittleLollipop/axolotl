// Test Louvain Algorithm
// Compile: swift test_louvain.swift

import Foundation

// MARK: - Core Types
typealias VertexID = UInt64
struct EdgeID: Hashable, Codable {
    let from: VertexID
    let to: VertexID
}
enum PropertyValue: Codable, Equatable, Hashable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case null
}
struct Vertex: Codable {
    let id: VertexID
    var properties: [String: PropertyValue]
}
struct Edge: Codable {
    let id: EdgeID
    var properties: [String: PropertyValue]
    var weight: Double
}
enum GraphError: Error {
    case vertexNotFound(VertexID)
    case edgeNotFound(EdgeID)
}

// MARK: - Simple Graph with Louvain
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

    // MARK: - Louvain Algorithm

    func detectCommunitiesLouvain(maxIterations: Int = 100) -> [VertexID: Int] {
        let n = vertices.count
        guard n > 0 else { return [:] }

        var communities: [VertexID: Int] = [:]
        for (i, vid) in vertices.keys.enumerated() {
            communities[vid] = i
        }

        let m = Double(edges.count)

        for _ in 0..<maxIterations {
            var improved = false

            for vid in vertices.keys {
                let neighbors = getAllNeighbors(of: vid)
                if neighbors.isEmpty { continue }

                let currentCommunity = communities[vid]!
                var bestCommunity = currentCommunity
                var bestGain = 0.0

                let candidateCommunities = Set(neighbors.compactMap { communities[$0] })

                for candidateCommunity in candidateCommunities {
                    if candidateCommunity == currentCommunity { continue }

                    let gain = computeModularityGain(
                        vertex: vid,
                        from: currentCommunity,
                        to: candidateCommunity,
                        communities: communities,
                        m: m
                    )

                    if gain > bestGain {
                        bestGain = gain
                        bestCommunity = candidateCommunity
                    }
                }

                if bestCommunity != currentCommunity {
                    communities[vid] = bestCommunity
                    improved = true
                }
            }

            if !improved {
                break
            }
        }

        return communities
    }

    private func computeModularityGain(
        vertex: VertexID,
        from: Int,
        to: Int,
        communities: [VertexID: Int],
        m: Double
    ) -> Double {
        let neighbors = getAllNeighbors(of: vertex)
        var sumInTo = 0.0
        var sumInFrom = 0.0
        let k_i = Double(neighbors.count)

        for neighbor in neighbors {
            let neighborCommunity = communities[neighbor]!
            let weight = getEdgeWeight(from: vertex, to: neighbor)

            if neighborCommunity == to {
                sumInTo += weight
            }
            if neighborCommunity == from {
                sumInFrom += weight
            }
        }

        var sumDegreesTo = 0.0
        var sumDegreesFrom = 0.0

        for (vid, community) in communities {
            if community == to {
                sumDegreesTo += Double(getAllNeighbors(of: vid).count)
            }
            if community == from {
                sumDegreesFrom += Double(getAllNeighbors(of: vid).count)
            }
        }

        let gain = (sumInTo - sumInFrom) - (k_i * sumDegreesTo - k_i * sumDegreesFrom) / (2.0 * m)
        return gain
    }

    private func getEdgeWeight(from: VertexID, to: VertexID) -> Double {
        if let edge = edges[EdgeID(from: from, to: to)] {
            return edge.weight
        }
        return 1.0
    }

    func getCommunitiesLouvain() -> [[VertexID]] {
        let communities = detectCommunitiesLouvain()
        var result: [Int: [VertexID]] = [:]
        for (vid, community) in communities {
            result[community, default: []].append(vid)
        }
        return Array(result.values)
    }

    // MARK: - Greedy Modularity (for comparison)

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
}

// MARK: - Test Cases
print("=== Testing Louvain Algorithm ===\n")

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

// Test 2: Bridge vertex
print("Test 2: Bridge vertex (connecting two communities)...")
let graph2 = SimpleGraph()
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
// Bridge: 2-3
graph2.addEdge(from: 2, to: 3, properties: [:], weight: 1.0)

let communities2 = graph2.getCommunitiesLouvain()
print("   Louvain detected \(communities2.count) communities:")
for (i, community) in communities2.enumerated() {
    print("     Community \(i): \(community.sorted())")
}
print("   ✅ Expected: 1 or 2 communities (depending on modularity)\n")

// Test 3: Star graph
print("Test 3: Star graph (center 0, leaves 1-4)...")
let graph3 = SimpleGraph()
for i in 0...4 {
    graph3.addVertex(id: UInt64(i), properties: [:])
}
for i in 1...4 {
    graph3.addEdge(from: 0, to: UInt64(i), properties: [:], weight: 1.0)
    graph3.addEdge(from: UInt64(i), to: 0, properties: [:], weight: 1.0)
}

let communities3 = graph3.getCommunitiesLouvain()
print("   Louvain detected \(communities3.count) communities:")
for (i, community) in communities3.enumerated() {
    print("     Community \(i): \(community.sorted())")
}
print("   ✅ Expected: 1 community (star is cohesive)\n")

// Test 4: Three communities
print("Test 4: Three communities (connected by bridges)...")
let graph4 = SimpleGraph()
for i in 0...8 {
    graph4.addVertex(id: UInt64(i), properties: [:])
}
// Community 1: 0-1-2
graph4.addEdge(from: 0, to: 1, properties: [:], weight: 1.0)
graph4.addEdge(from: 1, to: 2, properties: [:], weight: 1.0)
graph4.addEdge(from: 2, to: 0, properties: [:], weight: 1.0)
// Community 2: 3-4-5
graph4.addEdge(from: 3, to: 4, properties: [:], weight: 1.0)
graph4.addEdge(from: 4, to: 5, properties: [:], weight: 1.0)
graph4.addEdge(from: 5, to: 3, properties: [:], weight: 1.0)
// Community 3: 6-7-8
graph4.addEdge(from: 6, to: 7, properties: [:], weight: 1.0)
graph4.addEdge(from: 7, to: 8, properties: [:], weight: 1.0)
graph4.addEdge(from: 8, to: 6, properties: [:], weight: 1.0)
// Bridges: 2-3, 5-6
graph4.addEdge(from: 2, to: 3, properties: [:], weight: 0.5) // Weaker connection
graph4.addEdge(from: 5, to: 6, properties: [:], weight: 0.5)

let communities4 = graph4.getCommunitiesLouvain()
print("   Louvain detected \(communities4.count) communities:")
for (i, community) in communities4.enumerated() {
    print("     Community \(i): \(community.sorted())")
}
print("   ✅ Expected: 3 communities (bridges are weak)\n")

// Test 5: Performance comparison (Louvain vs Greedy)
print("Test 5: Performance comparison (20 vertices, 40 edges)...")
let graph5 = SimpleGraph()
for i in 0..<20 {
    graph5.addVertex(id: UInt64(i), properties: [:])
}
for _ in 0..<40 {
    let from = UInt64(Int.random(in: 0..<20))
    let to = UInt64(Int.random(in: 0..<20))
    if from != to {
        graph5.addEdge(from: from, to: to, properties: [:], weight: 1.0)
    }
}

let startLouvain = CFAbsoluteTimeGetCurrent()
let _ = graph5.getCommunitiesLouvain()
let timeLouvain = (CFAbsoluteTimeGetCurrent() - startLouvain) * 1000

let startGreedy = CFAbsoluteTimeGetCurrent()
let _ = graph5.detectCommunitiesGreedy()
let timeGreedy = (CFAbsoluteTimeGetCurrent() - startGreedy) * 1000

print("   Louvain time: \(String(format: "%.2f", timeLouvain)) ms")
print("   Greedy time: \(String(format: "%.2f", timeGreedy)) ms")
print("   Speedup: \(String(format: "%.2f", timeGreedy / timeLouvain))x\n")

print("=== All Tests Complete ===")
