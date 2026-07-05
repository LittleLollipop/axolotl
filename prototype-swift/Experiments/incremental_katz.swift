//
//  incremental_katz.swift
//  Incremental Katz Centrality for Unified Memory Architecture
//
//  Algorithm:
//  - Full: Power iteration: x_{t+1} = α * A * x_t + β
//  - Incremental: Only update affected vertices (similar to incremental PageRank)
//
//  Katz Centrality:
//  - x_i = α * sum of neighbors' centrality + β
//  - Usually β = 1, so: x_{t+1} = α * A * x_t + 1
//
//  Created on 2026-07-04.
//

import Foundation
import QuartzCore

// MARK: - Graph Data Structure
struct GraphKatz {
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
}

// MARK: - Full Katz Centrality (CPU - Power Iteration)
func fullKatzCentrality(
    graph: GraphKatz,
    alpha: Float = 0.1,
    beta: Float = 1.0,
    maxIterations: Int = 100,
    tolerance: Float = 1e-6
) -> (centrality: [Float], iterations: Int, TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    let n = graph.vertexCount
    
    // Initialize centrality vector (all ones)
    var centrality = Array(repeating: Float(1.0), count: n)
    var newCentrality = Array(repeating: Float(0.0), count: n)
    
    // Power iteration
    var iteration = 0
    var converged = false
    
    while iteration < maxIterations && !converged {
        // Compute new centrality: x_{t+1}[i] = α * sum of neighbors' centrality + β
        for i in 0..<n {
            var sum: Float = 0.0
            for neighbor in graph.adjacencyList[i] {
                sum += centrality[neighbor]
            }
            newCentrality[i] = alpha * sum + beta
        }
        
        // Check convergence
        var maxDiff: Float = 0.0
        for i in 0..<n {
            let diff = abs(newCentrality[i] - centrality[i])
            if diff > maxDiff {
                maxDiff = diff
            }
        }
        
        // Swap arrays
        swap(&centrality, &newCentrality)
        
        iteration += 1
        
        if maxDiff < tolerance {
            converged = true
        }
    }
    
    let elapsed = CACurrentMediaTime() - startTime
    return (centrality, iteration, elapsed)
}

// MARK: - Incremental Katz Centrality (CPU - Affected Vertex Queue)
func incrementalKatzCentrality(
    graph: GraphKatz,
    existingCentrality: [Float],
    newEdges: [(u: Int, v: Int)],
    alpha: Float = 0.1,
    beta: Float = 1.0,
    maxIterations: Int = 100,
    tolerance: Float = 1e-6
) -> (centrality: [Float], affectedVertices: Int, iterations: Int, TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    let n = graph.vertexCount
    
    // Start from existing centrality
    var centrality = existingCentrality
    var newCentrality = Array(repeating: Float(0.0), count: n)
    
    // Affected vertex queue (similar to incremental PageRank)
    var affectedVertices = 0
    var queue: [Int] = []
    var inQueue = Array(repeating: false, count: n)
    
    // Initially, only endpoints of new edges are affected
    for edge in newEdges {
        if !inQueue[edge.u] {
            queue.append(edge.u)
            inQueue[edge.u] = true
        }
        if !inQueue[edge.v] {
            queue.append(edge.v)
            inQueue[edge.v] = true
        }
    }
    
    // Iterative update (only process affected vertices)
    var iteration = 0
    var converged = false
    
    while iteration < maxIterations && !converged {
        var maxDiff: Float = 0.0
        var newQueue: [Int] = []
        var newInQueue = Array(repeating: false, count: n)
        
        // Process affected vertices
        for v in queue {
            affectedVertices += 1
            
            // Recompute Katz centrality for v
            var sum: Float = 0.0
            for neighbor in graph.adjacencyList[v] {
                sum += centrality[neighbor]
            }
            let newVal = alpha * sum + beta
            
            // Check if v's centrality changed significantly
            let diff = abs(newVal - centrality[v])
            if diff > tolerance {
                newCentrality[v] = newVal
                
                // Add neighbors to queue (their centrality might change)
                for neighbor in graph.adjacencyList[v] {
                    if !newInQueue[neighbor] {
                        newQueue.append(neighbor)
                        newInQueue[neighbor] = true
                    }
                }
            } else {
                newCentrality[v] = centrality[v]
            }
            
            if diff > maxDiff {
                maxDiff = diff
            }
        }
        
        // Swap
        swap(&centrality, &newCentrality)
        
        // Update queue
        queue = newQueue
        inQueue = newInQueue
        
        iteration += 1
        
        if maxDiff < tolerance || queue.isEmpty {
            converged = true
        }
    }
    
    let elapsed = CACurrentMediaTime() - startTime
    return (centrality, affectedVertices, iteration, elapsed)
}

// MARK: - Main Experiment
func runKatzExperiment() {
    print("=".repeating(count: 60))
    print("Incremental Katz Centrality Experiment")
    print("=".repeating(count: 60))
    print()
    
    // Experiment parameters
    let vertexCount = 10000
    let edgeCount = 50000
    let newEdgeCount = 1000  // 2% of edges
    let alpha: Float = 0.1  // Attenuation factor
    let beta: Float = 1.0  // Constant term
    let maxIterations = 100
    let tolerance: Float = 1e-6
    
    print("Experiment Parameters:")
    print("  Vertex Count: \(vertexCount)")
    print("  Edge Count: \(edgeCount)")
    print("  New Edges: \(newEdgeCount) (\(String(format: "%.1f", Double(newEdgeCount) / Double(edgeCount) * 100))% of edges)")
    print("  Alpha (attenuation): \(alpha)")
    print("  Beta (constant): \(beta)")
    print("  Max Iterations: \(maxIterations)")
    print("  Tolerance: \(tolerance)")
    print()
    
    // Generate initial graph
    print("Generating initial graph...")
    var graph = GraphKatz(vertexCount: vertexCount, edgeCount: edgeCount)
    print("  Done. Edges: \(graph.edges.count)")
    print()
    
    // Full Katz Centrality
    print("1. Full Katz Centrality (Power Iteration)")
    print("-".repeating(count: 40))
    
    let (fullCentrality, fullIterations, fullTime) = fullKatzCentrality(
        graph: graph,
        alpha: alpha,
        beta: beta,
        maxIterations: maxIterations,
        tolerance: tolerance
    )
    
    print("  Time: \(String(format: "%.2f", fullTime * 1000)) ms")
    print("  Iterations: \(fullIterations)")
    print("  Top 5 vertices: ", terminator: "")
    let sortedIndices = fullCentrality.enumerated().sorted { $0.element > $1.element }.prefix(5)
    for (idx, cent) in sortedIndices {
        print("(\(idx): \(String(format: "%.2f", cent))) ", terminator: "")
    }
    print()
    print()
    
    // Generate new edges
    print("2. Generating \(newEdgeCount) New Edges...")
    print("-".repeating(count: 40))
    
    let newEdges = graph.addEdges(count: newEdgeCount)
    print("  Done. Total edges: \(graph.edges.count)")
    print()
    
    // Incremental update
    print("3. Incremental Katz Centrality")
    print("-".repeating(count: 40))
    
    let (incCentrality, affectedVertices, incIterations, incTime) = incrementalKatzCentrality(
        graph: graph,
        existingCentrality: fullCentrality,
        newEdges: newEdges,
        alpha: alpha,
        beta: beta,
        maxIterations: maxIterations,
        tolerance: tolerance
    )
    
    print("  Time: \(String(format: "%.2f", incTime * 1000)) ms")
    print("  Iterations: \(incIterations)")
    print("  Affected vertices: \(affectedVertices)")
    print("  Speedup: \(String(format: "%.1f", fullTime / incTime))x")
    print()
    
    // Verification: Full recomputation after adding edges
    print("4. Verification (Full Recomputation after Adding Edges)")
    print("-".repeating(count: 40))
    
    let (verifyCentrality, verifyIterations, verifyTime) = fullKatzCentrality(
        graph: graph,
        alpha: alpha,
        beta: beta,
        maxIterations: maxIterations,
        tolerance: tolerance
    )
    
    print("  Time: \(String(format: "%.2f", verifyTime * 1000)) ms")
    print("  Iterations: \(verifyIterations)")
    print("  Top 5 vertices: ", terminator: "")
    let verifySorted = verifyCentrality.enumerated().sorted { $0.element > $1.element }.prefix(5)
    for (idx, cent) in verifySorted {
        print("(\(idx): \(String(format: "%.2f", cent))) ", terminator: "")
    }
    print()
    print()
    
    // Compare results (compute correlation)
    var correlation: Float = 0.0
    var sumX: Float = 0.0, sumY: Float = 0.0
    var sumXY: Float = 0.0, sumX2: Float = 0.0, sumY2: Float = 0.0
    
    for i in 0..<vertexCount {
        let x = incCentrality[i]
        let y = verifyCentrality[i]
        sumX += x
        sumY += y
        sumXY += x * y
        sumX2 += x * x
        sumY2 += y * y
    }
    
    let n = Float(vertexCount)
    correlation = (n * sumXY - sumX * sumY) / sqrt((n * sumX2 - sumX * sumX) * (n * sumY2 - sumY * sumY))
    
    print("  Correlation with verification: \(String(format: "%.6f", correlation))")
    print("  Correctness: \(correlation > 0.99 ? "✅ MATCH" : "⚠️ APPROXIMATE")")
    print()
    
    // Summary
    print("=".repeating(count: 60))
    print("Summary")
    print("=".repeating(count: 60))
    print()
    print("| Metric | Full | Incremental | Speedup |")
    print("|--------|------|-------------|---------|")
    print("| Time (ms) | \(String(format: "%.2f", fullTime * 1000)) | \(String(format: "%.2f", incTime * 1000)) | \(String(format: "%.1f", fullTime / incTime))x |")
    print("| Iterations | \(fullIterations) | \(incIterations) | - |")
    print("| Affected Vertices | \(vertexCount) | \(affectedVertices) | \(String(format: "%.1f", Float(vertexCount) / Float(affectedVertices)))x less |")
    print()
    
    // Save results
    let results: [String: Any] = [
        "algorithm": "Incremental Katz Centrality",
        "vertexCount": vertexCount,
        "edgeCount": edgeCount,
        "newEdgeCount": newEdgeCount,
        "alpha": alpha,
        "beta": beta,
        "fullTime": fullTime * 1000,
        "incrementalTime": incTime * 1000,
        "speedup": fullTime / incTime,
        "fullIterations": fullIterations,
        "incrementalIterations": incIterations,
        "affectedVertices": affectedVertices,
        "correlation": correlation
    ]
    
    if let jsonData = try? JSONSerialization.data(withJSONObject: results, options: .prettyPrinted),
       let jsonString = String(data: jsonData, encoding: .utf8) {
        let resultsPath = "/tmp/axolotl_tmp/Experiments/katz_results.json"
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
runKatzExperiment()
