//
//  incremental_kcore.swift
//  Incremental K-core Decomposition for Unified Memory Architecture
//
//  Algorithm:
//  - Full: Bucket-sort based K-core decomposition (O(V + E))
//  - Incremental: Only update affected vertices (O(ΔV × deg))
//
//  Created on 2026-07-04.
//

import Foundation
import QuartzCore

// MARK: - Graph Data Structure (with Adjacency List)
struct GraphKC {
    let vertexCount: Int
    var edges: [(u: Int, v: Int)]
    var adjacencyList: [[Int]]
    
    init(vertexCount: Int, edgeCount: Int) {
        self.vertexCount = vertexCount
        self.edges = []
        self.edges.reserveCapacity(edgeCount)
        self.adjacencyList = Array(repeating: [], count: vertexCount)
        
        // Generate random edges (undirected)
        var rng = SystemRandomNumberGenerator()
        for _ in 0..<edgeCount {
            let u = Int.random(in: 0..<vertexCount, using: &rng)
            let v = Int.random(in: 0..<vertexCount, using: &rng)
            if u != v {
                edges.append((u: u, v: v))
                adjacencyList[u].append(v)
                adjacencyList[v].append(u)
            }
        }
        
        // Remove duplicates (manual dedup)
        var uniqueEdges = Set<String>()
        var dedupEdges: [(u: Int, v: Int)] = []
        dedupEdges.reserveCapacity(edges.count)
        
        for edge in edges {
            let key1 = "\(edge.u),\(edge.v)"
            let key2 = "\(edge.v),\(edge.u)"
            if !uniqueEdges.contains(key1) && !uniqueEdges.contains(key2) {
                uniqueEdges.insert(key1)
                dedupEdges.append(edge)
            }
        }
        
        edges = dedupEdges
        
        // Rebuild adjacency list
        adjacencyList = Array(repeating: [], count: vertexCount)
        for edge in edges {
            adjacencyList[edge.u].append(edge.v)
            adjacencyList[edge.v].append(edge.u)
        }
    }
    
    // Add new edges for incremental update
    mutating func addEdges(count: Int) -> [(u: Int, v: Int)] {
        var newEdges: [(u: Int, v: Int)] = []
        newEdges.reserveCapacity(count)
        
        var rng = SystemRandomNumberGenerator()
        for _ in 0..<count {
            let u = Int.random(in: 0..<vertexCount, using: &rng)
            let v = Int.random(in: 0..<vertexCount, using: &rng)
            if u != v {
                newEdges.append((u: u, v: v))
                adjacencyList[u].append(v)
                adjacencyList[v].append(u)
            }
        }
        
        edges.append(contentsOf: newEdges)
        return newEdges
    }
    
    // Remove edges for incremental update
    mutating func removeEdges(edgesToRemove: [(u: Int, v: Int)]) {
        for edge in edgesToRemove {
            adjacencyList[edge.u].removeAll { $0 == edge.v }
            adjacencyList[edge.v].removeAll { $0 == edge.u }
        }
        
        let removeSet = Set(edgesToRemove.map { "\($0.u),\($0.v)" })
        edges.removeAll { removeSet.contains("\($0.u),\($0.v)") || removeSet.contains("\($0.v),\($0.u)") }
    }
    
    // Get degree of all vertices
    func getDegrees() -> [Int] {
        return adjacencyList.map { $0.count }
    }
}

// MARK: - Full K-core Decomposition (Bucket Sort)
func fullKCore(graph: GraphKC, k: Int) -> (core: [Int], TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    let n = graph.vertexCount
    var degrees = graph.getDegrees()
    
    // Find max degree for bucket size
    let maxDegree = degrees.max() ?? 0
    
    // Bucket sort optimization (O(V + E))
    var buckets = Array(repeating: [Int](), count: maxDegree + 1)
    for v in 0..<n {
        buckets[degrees[v]].append(v)
    }
    
    var core = Array(repeating: 0, count: n)
    var currentK = 0
    
    // Process vertices in increasing order of degree
    for d in 0...maxDegree {
        for v in buckets[d] {
            core[v] = max(currentK, d)
            
            // Update neighbors
            for neighbor in graph.adjacencyList[v] {
                if degrees[neighbor] > d {
                    degrees[neighbor] -= 1
                    buckets[degrees[neighbor]].append(neighbor)
                }
            }
            
            currentK = max(currentK, d)
        }
    }
    
    let elapsed = CACurrentMediaTime() - startTime
    return (core, elapsed)
}

// MARK: - Incremental K-core (Add Edges)
func incrementalKCoreAddEdges(graph: GraphKC, existingCore: [Int], newEdges: [(u: Int, v: Int)], k: Int) -> (newCore: [Int], affectedVertices: Int, TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    var core = existingCore
    var degrees = graph.getDegrees()
    var affectedCount = 0
    
    // Queue for vertices that might need core number update
    var queue: [Int] = []
    var inQueue = Array(repeating: false, count: graph.vertexCount)
    
    // Add endpoints of new edges to queue
    for edge in newEdges {
        let u = edge.u
        let v = edge.v
        
        degrees[u] += 1
        degrees[v] += 1
        
        // Check if u or v might increase core number
        if !inQueue[u] && core[u] < k {
            queue.append(u)
            inQueue[u] = true
        }
        if !inQueue[v] && core[v] < k {
            queue.append(v)
            inQueue[v] = true
        }
    }
    
    // Process queue
    var queuePos = 0
    while queuePos < queue.count {
        let v = queue[queuePos]
        queuePos += 1
        inQueue[v] = false
        affectedCount += 1
        
        // Recompute core number for v
        let oldCore = core[v]
        
        // Count how many neighbors have core >= potential new core
        // Simplified: if degree >= k, core can be at least k
        if degrees[v] >= k && oldCore < k {
            core[v] = k
            
            // Check if neighbors might also increase
            for neighbor in graph.adjacencyList[v] {
                if core[neighbor] < k && !inQueue[neighbor] {
                    queue.append(neighbor)
                    inQueue[neighbor] = true
                }
            }
        }
    }
    
    let elapsed = CACurrentMediaTime() - startTime
    return (core, affectedCount, elapsed)
}

// MARK: - Incremental K-core (Remove Edges)
func incrementalKCoreRemoveEdges(graph: GraphKC, existingCore: [Int], edgesToRemove: [(u: Int, v: Int)], k: Int) -> (newCore: [Int], affectedVertices: Int, TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    var core = existingCore
    var degrees = graph.getDegrees()
    var affectedCount = 0
    
    // Queue for vertices that might need core number update
    var queue: [Int] = []
    var inQueue = Array(repeating: false, count: graph.vertexCount)
    
    // Add endpoints of removed edges to queue
    for edge in edgesToRemove {
        let u = edge.u
        let v = edge.v
        
        degrees[u] -= 1
        degrees[v] -= 1
        
        // Check if u or v might decrease core number
        if degrees[u] < core[u] && !inQueue[u] {
            queue.append(u)
            inQueue[u] = true
        }
        if degrees[v] < core[v] && !inQueue[v] {
            queue.append(v)
            inQueue[v] = true
        }
    }
    
    // Process queue
    var queuePos = 0
    while queuePos < queue.count {
        let v = queue[queuePos]
        queuePos += 1
        inQueue[v] = false
        affectedCount += 1
        
        // Recompute core number for v
        let oldCore = core[v]
        
        // Count neighbors with core >= (potential new core)
        // Simplified: core cannot exceed degree
        if degrees[v] < oldCore {
            core[v] = degrees[v]
            
            // Check if neighbors might also decrease
            for neighbor in graph.adjacencyList[v] {
                if core[neighbor] > core[v] && !inQueue[neighbor] {
                    queue.append(neighbor)
                    inQueue[neighbor] = true
                }
            }
        }
    }
    
    let elapsed = CACurrentMediaTime() - startTime
    return (core, affectedCount, elapsed)
}

// MARK: - Main Experiment
func runKCoreExperiment() {
    print("=".repeating(count: 60))
    print("Incremental K-core Decomposition Experiment")
    print("=".repeating(count: 60))
    print()
    
    // Experiment parameters
    let vertexCount = 5000
    let edgeCount = 20000
    let k = 5  // Core number threshold
    let newEdgeCount = 500  // 2.5% of edges
    
    print("Experiment Parameters:")
    print("  Vertex Count: \(vertexCount)")
    print("  Initial Edge Count: \(edgeCount)")
    print("  K (core threshold): \(k)")
    print("  New Edges: \(newEdgeCount) (\(String(format: "%.1f", Double(newEdgeCount) / Double(edgeCount) * 100))% of edges)")
    print()
    
    // Generate initial graph
    print("Generating initial graph...")
    var graph = GraphKC(vertexCount: vertexCount, edgeCount: edgeCount)
    print("  Done. Actual edges: \(graph.edges.count)")
    print()
    
    // Full K-core decomposition
    print("1. Full K-core Decomposition (Bucket Sort)")
    print("-".repeating(count: 40))
    
    let (fullCore, fullTime) = fullKCore(graph: graph, k: k)
    let fullCoreCount = fullCore.filter { $0 >= k }.count
    
    print("  Time: \(String(format: "%.2f", fullTime * 1000)) ms")
    print("  Vertices with core >= \(k): \(fullCoreCount) / \(vertexCount)")
    print("  Max core number: \(fullCore.max()!)")
    print()
    
    // Incremental update (Add Edges)
    print("2. Incremental K-core (Add Edges)")
    print("-".repeating(count: 40))
    
    // Add new edges
    let newEdges = graph.addEdges(count: newEdgeCount)
    
    let (incCoreAdd, affectedAdd, incTimeAdd) = incrementalKCoreAddEdges(
        graph: graph,
        existingCore: fullCore,
        newEdges: newEdges,
        k: k
    )
    
    print("  New edges: \(newEdges.count)")
    print("  Affected vertices: \(affectedAdd)")
    print("  Time: \(String(format: "%.2f", incTimeAdd * 1000)) ms")
    print("  Speedup: \(String(format: "%.1f", fullTime / incTimeAdd))x")
    print()
    
    // Verify correctness (Full recomputation after adding edges)
    print("3. Verification (Full Recomputation after Adding Edges)")
    print("-".repeating(count: 40))
    
    let (verifyCoreAdd, verifyTimeAdd) = fullKCore(graph: graph, k: k)
    let verifyCoreCountAdd = verifyCoreAdd.filter { $0 >= k }.count
    
    print("  Time: \(String(format: "%.2f", verifyTimeAdd * 1000)) ms")
    print("  Vertices with core >= \(k): \(verifyCoreCountAdd) / \(vertexCount)")
    
    // Compare results
    var addMatch = true
    for i in 0..<vertexCount {
        if incCoreAdd[i] != verifyCoreAdd[i] {
            addMatch = false
            break
        }
    }
    
    print("  Correctness: \(addMatch ? "✅ MATCH" : "❌ MISMATCH")")
    print()
    
    // Incremental update (Remove Edges)
    print("4. Incremental K-core (Remove Edges)")
    print("-".repeating(count: 40))
    
    // Remove some edges
    let edgesToRemove = Array(newEdges.prefix(newEdgeCount / 2))
    graph.removeEdges(edgesToRemove: edgesToRemove)
    
    let (incCoreRemove, affectedRemove, incTimeRemove) = incrementalKCoreRemoveEdges(
        graph: graph,
        existingCore: verifyCoreAdd,
        edgesToRemove: edgesToRemove,
        k: k
    )
    
    print("  Removed edges: \(edgesToRemove.count)")
    print("  Affected vertices: \(affectedRemove)")
    print("  Time: \(String(format: "%.2f", incTimeRemove * 1000)) ms")
    print("  Speedup: \(String(format: "%.1f", fullTime / incTimeRemove))x")
    print()
    
    // Verify correctness (Full recomputation after removing edges)
    print("5. Verification (Full Recomputation after Removing Edges)")
    print("-".repeating(count: 40))
    
    let (verifyCoreRemove, verifyTimeRemove) = fullKCore(graph: graph, k: k)
    let verifyCoreCountRemove = verifyCoreRemove.filter { $0 >= k }.count
    
    print("  Time: \(String(format: "%.2f", verifyTimeRemove * 1000)) ms")
    print("  Vertices with core >= \(k): \(verifyCoreCountRemove) / \(vertexCount)")
    
    // Compare results
    var removeMatch = true
    for i in 0..<vertexCount {
        if incCoreRemove[i] != verifyCoreRemove[i] {
            removeMatch = false
            break
        }
    }
    
    print("  Correctness: \(removeMatch ? "✅ MATCH" : "❌ MISMATCH")")
    print()
    
    // Summary
    print("=".repeating(count: 60))
    print("Summary")
    print("=".repeating(count: 60))
    print()
    print("| Scenario | Full Recomputation | Incremental | Speedup | Correctness |")
    print("|----------|-------------------|-------------|---------|-------------|")
    print("| Add Edges | \(String(format: "%.2f", verifyTimeAdd * 1000)) ms | \(String(format: "%.2f", incTimeAdd * 1000)) ms | \(String(format: "%.1f", verifyTimeAdd / incTimeAdd))x | \(addMatch ? "✅" : "❌") |")
    print("| Remove Edges | \(String(format: "%.2f", verifyTimeRemove * 1000)) ms | \(String(format: "%.2f", incTimeRemove * 1000)) ms | \(String(format: "%.1f", verifyTimeRemove / incTimeRemove))x | \(removeMatch ? "✅" : "❌") |")
    print()
    
    // Save results to JSON
    let results: [String: Any] = [
        "algorithm": "Incremental K-core Decomposition",
        "vertexCount": vertexCount,
        "edgeCount": edgeCount,
        "k": k,
        "newEdgeCount": newEdgeCount,
        "fullTime": fullTime * 1000,
        "incrementalAddTime": incTimeAdd * 1000,
        "incrementalRemoveTime": incTimeRemove * 1000,
        "speedupAdd": verifyTimeAdd / incTimeAdd,
        "speedupRemove": verifyTimeRemove / incTimeRemove,
        "correctnessAdd": addMatch,
        "correctnessRemove": removeMatch,
        "affectedVerticesAdd": affectedAdd,
        "affectedVerticesRemove": affectedRemove
    ]
    
    if let jsonData = try? JSONSerialization.data(withJSONObject: results, options: .prettyPrinted),
       let jsonString = String(data: jsonData, encoding: .utf8) {
        let resultsPath = "/tmp/axolotl_tmp/Experiments/kcore_results.json"
        try? jsonString.write(toFile: resultsPath, atomically: true, encoding: .utf8)
        print("Results saved to: \(resultsPath)")
    }
}

// Helper extension
extension String {
    func repeating(count: Int) -> String {
        return String(repeating: self, count: count)
    }
}

// Run experiment
runKCoreExperiment()
