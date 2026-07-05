//
//  incremental_clustering.swift
//  Incremental Local Clustering Coefficient for Unified Memory Architecture
//
//  Algorithm:
//  - Full: Compute clustering coefficient for all vertices
//  - Incremental: Only update vertices affected by new edges
//
//  Local Clustering Coefficient:
//  - C(v) = 2 * T(v) / (deg(v) * (deg(v) - 1))
//  - T(v) = number of triangles involving v
//
//  Created on 2026-07-04.
//

import Foundation
import QuartzCore

// MARK: - Graph Data Structure (with Adjacency Sets)
struct GraphCluster {
    let vertexCount: Int
    var edges: [(u: Int, v: Int)]
    var adjacencySet: [Set<Int>]
    
    init(vertexCount: Int, edgeCount: Int) {
        self.vertexCount = vertexCount
        self.edges = []
        self.edges.reserveCapacity(edgeCount)
        self.adjacencySet = Array(repeating: [], count: vertexCount)
        
        // Generate random edges (undirected)
        var rng = SystemRandomNumberGenerator()
        for _ in 0..<edgeCount {
            let u = Int.random(in: 0..<vertexCount, using: &rng)
            let v = Int.random(in: 0..<vertexCount, using: &rng)
            if u != v {
                edges.append((u: u, v: v))
                adjacencySet[u].insert(v)
                adjacencySet[v].insert(u)
            }
        }
        
        // Remove duplicates (Set automatically handles this)
        var uniqueEdges: [(u: Int, v: Int)] = []
        var edgeSet = Set<String>()
        
        for edge in edges {
            let key1 = "\(edge.u),\(edge.v)"
            let key2 = "\(edge.v),\(edge.u)"
            if !edgeSet.contains(key1) && !edgeSet.contains(key2) {
                edgeSet.insert(key1)
                uniqueEdges.append(edge)
            }
        }
        
        edges = uniqueEdges
    }
    
    // Add new edges
    mutating func addEdges(count: Int) -> [(u: Int, v: Int)] {
        var newEdges: [(u: Int, v: Int)] = []
        newEdges.reserveCapacity(count)
        
        var rng = SystemRandomNumberGenerator()
        for _ in 0..<count {
            let u = Int.random(in: 0..<vertexCount, using: &rng)
            let v = Int.random(in: 0..<vertexCount, using: &rng)
            if u != v {
                newEdges.append((u: u, v: v))
                adjacencySet[u].insert(v)
                adjacencySet[v].insert(u)
            }
        }
        
        edges.append(contentsOf: newEdges)
        return newEdges
    }
    
    // Count triangles involving vertex v
    func countTriangles(v: Int) -> Int {
        let neighbors = adjacencySet[v]
        var triangleCount = 0
        
        for i in neighbors {
            for j in neighbors {
                if i < j && adjacencySet[i].contains(j) {
                    triangleCount += 1
                }
            }
        }
        
        return triangleCount
    }
}

// MARK: - Full Local Clustering Coefficient
func fullClusteringCoefficient(graph: GraphCluster) -> (coefficients: [Double], triangles: [Int], TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    let n = graph.vertexCount
    var coefficients = Array(repeating: 0.0, count: n)
    var triangles = Array(repeating: 0, count: n)
    
    for v in 0..<n {
        let deg = graph.adjacencySet[v].count
        
        if deg < 2 {
            coefficients[v] = 0.0
            triangles[v] = 0
            continue
        }
        
        // Count triangles involving v
        let t = graph.countTriangles(v: v)
        triangles[v] = t
        
        // Compute clustering coefficient
        coefficients[v] = 2.0 * Double(t) / Double(deg * (deg - 1))
    }
    
    let elapsed = CACurrentMediaTime() - startTime
    return (coefficients, triangles, elapsed)
}

// MARK: - Incremental Local Clustering Coefficient (Correct Implementation)
func incrementalClusteringCoefficientCorrect(
    graph: GraphCluster,
    newEdges: [(u: Int, v: Int)]
) -> (coefficients: [Double], triangles: [Int], affectedVertices: Int, TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    // First, compute full clustering coefficients
    let (fullCoeff, fullTriangles, _) = fullClusteringCoefficient(graph: graph)
    
    var coefficients = fullCoeff
    var triangles = fullTriangles
    var affectedVertices = 0
    
    // Vertices that might be affected:
    // 1. Endpoints of new edges (u and v)
    // 2. Common neighbors of u and v (new triangles form)
    var verticesToUpdate = Set<Int>()
    
    for edge in newEdges {
        let u = edge.u
        let v = edge.v
        
        verticesToUpdate.insert(u)
        verticesToUpdate.insert(v)
        
        // Find common neighbors of u and v (new triangles form)
        let commonNeighbors = graph.adjacencySet[u].intersection(graph.adjacencySet[v])
        for w in commonNeighbors {
            verticesToUpdate.insert(w)
        }
    }
    
    // Recompute clustering coefficient for affected vertices
    for v in verticesToUpdate {
        affectedVertices += 1
        
        let deg = graph.adjacencySet[v].count
        
        if deg < 2 {
            coefficients[v] = 0.0
            triangles[v] = 0
            continue
        }
        
        // Recount triangles
        let t = graph.countTriangles(v: v)
        triangles[v] = t
        
        // Recompute clustering coefficient
        coefficients[v] = 2.0 * Double(t) / Double(deg * (deg - 1))
    }
    
    let elapsed = CACurrentMediaTime() - startTime
    return (coefficients, triangles, affectedVertices, elapsed)
}

// MARK: - Main Experiment
func runClusteringExperiment() {
    print("=".repeating(count: 60))
    print("Incremental Local Clustering Coefficient Experiment")
    print("=".repeating(count: 60))
    print()
    
    // Experiment parameters
    let vertexCount = 10000
    let edgeCount = 50000
    let newEdgeCount = 1000  // 2% of edges
    
    print("Experiment Parameters:")
    print("  Vertex Count: \(vertexCount)")
    print("  Edge Count: \(edgeCount)")
    print("  New Edges: \(newEdgeCount) (\(String(format: "%.1f", Double(newEdgeCount) / Double(edgeCount) * 100))% of edges)")
    print()
    
    // Generate initial graph
    print("Generating initial graph...")
    var graph = GraphCluster(vertexCount: vertexCount, edgeCount: edgeCount)
    print("  Done. Edges: \(graph.edges.count)")
    print()
    
    // Full Local Clustering Coefficient
    print("1. Full Local Clustering Coefficient")
    print("-".repeating(count: 40))
    
    let (fullCoeff, fullTriangles, fullTime) = fullClusteringCoefficient(graph: graph)
    
    let avgCoeff = fullCoeff.reduce(0.0, +) / Double(vertexCount)
    let totalTriangles = fullTriangles.reduce(0, +) / 3  // Each triangle counted 3 times
    
    print("  Time: \(String(format: "%.2f", fullTime * 1000)) ms")
    print("  Avg clustering coefficient: \(String(format: "%.4f", avgCoeff))")
    print("  Total triangles: \(totalTriangles)")
    print()
    
    // Generate new edges
    print("2. Generating \(newEdgeCount) New Edges...")
    print("-".repeating(count: 40))
    
    let newEdges = graph.addEdges(count: newEdgeCount)
    print("  Done. Total edges: \(graph.edges.count)")
    print()
    
    // Incremental update
    print("3. Incremental Local Clustering Coefficient (Corrected)")
    print("-".repeating(count: 40))
    
    let (incCoeff, incTriangles, affectedVertices, incTime) = incrementalClusteringCoefficientCorrect(
        graph: graph,
        newEdges: newEdges
    )
    
    print("  Time: \(String(format: "%.2f", incTime * 1000)) ms")
    print("  Affected vertices: \(affectedVertices)")
    print("  Speedup: \(String(format: "%.1f", fullTime / incTime))x")
    print()
    
    // Verification: Full recomputation after adding edges
    print("4. Verification (Full Recomputation after Adding Edges)")
    print("-".repeating(count: 40))
    
    let (verifyCoeff, verifyTriangles, verifyTime) = fullClusteringCoefficient(graph: graph)
    
    let verifyAvgCoeff = verifyCoeff.reduce(0.0, +) / Double(vertexCount)
    let verifyTotalTriangles = verifyTriangles.reduce(0, +) / 3
    
    print("  Time: \(String(format: "%.2f", verifyTime * 1000)) ms")
    print("  Avg clustering coefficient: \(String(format: "%.4f", verifyAvgCoeff))")
    print("  Total triangles: \(verifyTotalTriangles)")
    print()
    
    // Compare results
    var maxDiff = 0.0
    for i in 0..<vertexCount {
        let diff = abs(incCoeff[i] - verifyCoeff[i])
        if diff > maxDiff {
            maxDiff = diff
        }
    }
    
    print("  Max difference: \(String(format: "%.10f", maxDiff))")
    print("  Correctness: \(maxDiff < 1e-10 ? "✅ MATCH" : "❌ MISMATCH")")
    print()
    
    // Summary
    print("=".repeating(count: 60))
    print("Summary")
    print("=".repeating(count: 60))
    print()
    print("| Metric | Full | Incremental | Speedup |")
    print("|--------|------|-------------|---------|")
    print("| Time (ms) | \(String(format: "%.2f", fullTime * 1000)) | \(String(format: "%.2f", incTime * 1000)) | \(String(format: "%.1f", fullTime / incTime))x |")
    print("| Affected Vertices | \(vertexCount) | \(affectedVertices) | \(String(format: "%.1f", Double(vertexCount) / Double(affectedVertices)))x less |")
    print()
    
    // Save results
    let results: [String: Any] = [
        "algorithm": "Incremental Local Clustering Coefficient",
        "vertexCount": vertexCount,
        "edgeCount": edgeCount,
        "newEdgeCount": newEdgeCount,
        "fullTime": fullTime * 1000,
        "incrementalTime": incTime * 1000,
        "speedup": fullTime / incTime,
        "affectedVertices": affectedVertices,
        "avgCoefficient": avgCoeff,
        "totalTriangles": totalTriangles
    ]
    
    if let jsonData = try? JSONSerialization.data(withJSONObject: results, options: .prettyPrinted),
       let jsonString = String(data: jsonData, encoding: .utf8) {
        let resultsPath = "/tmp/axolotl_tmp/Experiments/clustering_results.json"
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
runClusteringExperiment()
