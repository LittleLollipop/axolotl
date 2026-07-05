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

// MARK: - Simple Graph with Betweenness Centrality

class SimpleGraph {
    var vertices: [VertexID: Vertex] = [:]
    var edges: [EdgeID: Edge] = [:]
    var adjacencyList: [VertexID: Set<VertexID>] = [:]
    var betweennessScores: [VertexID: Double] = [:]
    var isBetweennessDirty: Bool = true

    func addVertex(id: VertexID, properties: [String: PropertyValue]) {
        let vertex = Vertex(id: id, properties: properties)
        vertices[id] = vertex
        adjacencyList[id] = []
        isBetweennessDirty = true
    }

    func addEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue], weight: Double) {
        let edgeId = EdgeID(from: from, to: to)
        let edge = Edge(id: edgeId, properties: properties, weight: weight)
        edges[edgeId] = edge
        adjacencyList[from]?.insert(to)
        isBetweennessDirty = true
    }

    // MARK: - Betweenness Centrality

    func betweennessCentrality() -> [VertexID: Double] {
        let n = vertices.count
        guard n > 2 else { return [:] }

        // Check cache
        if !isBetweennessDirty {
            return betweennessScores
        }

        // Initialize betweenness scores
        var betweenness: [VertexID: Double] = [:]
        for vid in vertices.keys {
            betweenness[vid] = 0.0
        }

        // Compute betweenness for each vertex as source
        for s in vertices.keys {
            // BFS from s
            var stack: [VertexID] = []
            var distances: [VertexID: Int] = [:]
            var numSP: [VertexID: Double] = [:]
            var predecessors: [VertexID: [VertexID]] = [:]

            // Initialize
            for v in vertices.keys {
                distances[v] = -1
                numSP[v] = 0.0
                predecessors[v] = []
            }

            distances[s] = 0
            numSP[s] = 1.0

            var queue: [VertexID] = [s]

            // BFS
            while !queue.isEmpty {
                let v = queue.removeFirst()
                stack.append(v)

                for neighbor in adjacencyList[v] ?? [] {
                    // Path discovery
                    if distances[neighbor]! < 0 {
                        queue.append(neighbor)
                        distances[neighbor] = distances[v]! + 1
                    }

                    // Path counting
                    if distances[neighbor] == distances[v]! + 1 {
                        numSP[neighbor] = numSP[neighbor]! + numSP[v]!
                        predecessors[neighbor]?.append(v)
                    }
                }
            }

            // Accumulation (backward pass)
            var delta: [VertexID: Double] = [:]
            for v in vertices.keys {
                delta[v] = 0.0
            }

            // Process vertices in reverse BFS order
            while !stack.isEmpty {
                let w = stack.removeLast()
                if let preds = predecessors[w] {
                    for v in preds {
                        let contribution = (1.0 + delta[w]!) * (numSP[v]! / numSP[w]!)
                        delta[v] = delta[v]! + contribution
                    }
                }
                if w != s {
                    betweenness[w] = betweenness[w]! + delta[w]!
                }
            }
        }

        // Normalize (for undirected graphs)
        let normalizeFactor = Double(n - 1) * Double(n - 2) / 2.0
        if normalizeFactor > 0 {
            for vid in betweenness.keys {
                betweenness[vid] = betweenness[vid]! / normalizeFactor
            }
        }

        // Update cache
        betweennessScores = betweenness
        isBetweennessDirty = false

        return betweenness
    }

    func printBetweennessScores() {
        let scores = betweennessCentrality()
        print("   Betweenness Centrality scores:")
        for (id, score) in scores.sorted(by: { $0.key < $1.key }) {
            print("     Vertex \(id): \(String(format: "%.4f", score))")
        }
    }
}

// MARK: - Main Test

print("=== Betweenness Centrality Test ===\n")

// Test 1: Simple path graph (0-1-2-3)
print("1. Testing simple path graph (0-1-2-3)...")
let graph1 = SimpleGraph()
graph1.addVertex(id: 0, properties: [:])
graph1.addVertex(id: 1, properties: [:])
graph1.addVertex(id: 2, properties: [:])
graph1.addVertex(id: 3, properties: [:])

graph1.addEdge(from: 0, to: 1, properties: [:], weight: 1.0)
graph1.addEdge(from: 1, to: 2, properties: [:], weight: 1.0)
graph1.addEdge(from: 2, to: 3, properties: [:], weight: 1.0)

graph1.printBetweennessScores()
print("   Expected: Vertex 1 and 2 should have higher scores (bridge vertices)\n")

// Test 2: Star graph (center 0, leaves 1,2,3,4)
print("2. Testing star graph (center 0)...")
let graph2 = SimpleGraph()
for i in 0...4 {
    graph2.addVertex(id: UInt64(i), properties: [:])
}
for i in 1...4 {
    graph2.addEdge(from: 0, to: UInt64(i), properties: [:], weight: 1.0)
    graph2.addEdge(from: UInt64(i), to: 0, properties: [:], weight: 1.0)  // Add reverse edge for undirected graph
}

graph2.printBetweennessScores()
print("   Expected: Vertex 0 should have the highest score (center)\n")

// Test 3: Complete graph (all pairs connected)
print("3. Testing complete graph (K4)...")
let graph3 = SimpleGraph()
for i in 0...3 {
    graph3.addVertex(id: UInt64(i), properties: [:])
}
for i in 0...3 {
    for j in 0...3 {
        if i != j {
            graph3.addEdge(from: UInt64(i), to: UInt64(j), properties: [:], weight: 1.0)
        }
    }
}

graph3.printBetweennessScores()
print("   Expected: All vertices should have similar scores (complete graph)\n")

// Test 4: Graph with a clear "bridge" vertex
print("4. Testing graph with bridge vertex...")
let graph4 = SimpleGraph()
for i in 0...5 {
    graph4.addVertex(id: UInt64(i), properties: [:])
}

// Component 1: 0-1-2
graph4.addEdge(from: 0, to: 1, properties: [:], weight: 1.0)
graph4.addEdge(from: 1, to: 2, properties: [:], weight: 1.0)

// Component 2: 3-4-5
graph4.addEdge(from: 3, to: 4, properties: [:], weight: 1.0)
graph4.addEdge(from: 4, to: 5, properties: [:], weight: 1.0)

// Bridge: 2-3
graph4.addEdge(from: 2, to: 3, properties: [:], weight: 1.0)

graph4.printBetweennessScores()
print("   Expected: Vertex 2 and 3 should have higher scores (bridge)\n")

// Test 5: Performance test
print("5. Performance test (100 vertices, 200 edges)...")
let graph5 = SimpleGraph()
for i in 0..<100 {
    graph5.addVertex(id: UInt64(i), properties: [:])
}
for _ in 0..<200 {
    let from = UInt64(Int.random(in: 0..<100))
    let to = UInt64(Int.random(in: 0..<100))
    if from != to {
        graph5.addEdge(from: from, to: to, properties: [:], weight: 1.0)
        graph5.addEdge(from: to, to: from, properties: [:], weight: 1.0)  // Undirected
    }
}

let start = CFAbsoluteTimeGetCurrent()
let scores = graph5.betweennessCentrality()
let time = (CFAbsoluteTimeGetCurrent() - start) * 1000

print("   Computation time: \(String(format: "%.2f", time)) ms")
print("   Top 5 vertices:")
let sortedScores = scores.sorted(by: { $0.value > $1.value })
let topCount = min(5, sortedScores.count)
for i in 0..<topCount {
    let (id, score) = sortedScores[i]
    print("     Vertex \(id): \(String(format: "%.4f", score))")
}

print("\n=== Test Complete ===")
