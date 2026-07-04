//
//  incremental_eigenvector.swift
//  Incremental Eigenvector Centrality for Unified Memory Architecture
//
//  Algorithm:
//  - Full: Power iteration to compute principal eigenvector
//  - Incremental: Only update affected vertices (similar to incremental PageRank)
//
//  Eigenvector Centrality:
//  - A vertex's centrality = sum of neighbors' centralities
//  - Computed by power iteration: x_{t+1} = A * x_t / ||A * x_t||
//
//  Created on 2026-07-04.
//

import Foundation
import QuartzCore
import Metal

// MARK: - Graph Data Structure (COO format for Metal)
struct GraphEV {
    let vertexCount: Int
    let edgeCount: Int
    let sources: [UInt32]
    let targets: [UInt32]
    let weights: [Float]  // For weighted graphs (default = 1.0)
    
    // Memberwise initializer
    init(vertexCount: Int, edgeCount: Int, sources: [UInt32], targets: [UInt32], weights: [Float]) {
        self.vertexCount = vertexCount
        self.edgeCount = edgeCount
        self.sources = sources
        self.targets = targets
        self.weights = weights
    }
    
    // Generate random graph (undirected, no self-loops)
    static func random(vertexCount: Int, edgeCount: Int, weighted: Bool = false) -> GraphEV {
        var sources: [UInt32] = []
        var targets: [UInt32] = []
        var weights: [Float] = []
        
        sources.reserveCapacity(edgeCount)
        targets.reserveCapacity(edgeCount)
        if weighted {
            weights.reserveCapacity(edgeCount)
        }
        
        // Use Set to avoid duplicates
        var edgeSet = Set<String>()
        var rng = SystemRandomNumberGenerator()
        
        while sources.count < edgeCount {
            let u = UInt32.random(in: 0..<UInt32(vertexCount), using: &rng)
            let v = UInt32.random(in: 0..<UInt32(vertexCount), using: &rng)
            
            if u != v {
                let key1 = "\(u),\(v)"
                let key2 = "\(v),\(u)"
                if !edgeSet.contains(key1) && !edgeSet.contains(key2) {
                    edgeSet.insert(key1)
                    sources.append(min(u, v))
                    targets.append(max(u, v))
                    if weighted {
                        weights.append(Float.random(in: 1.0...10.0, using: &rng))
                    }
                }
            }
        }
        
        return GraphEV(
            vertexCount: vertexCount,
            edgeCount: edgeCount,
            sources: sources,
            targets: targets,
            weights: weighted ? weights : Array(repeating: 1.0, count: edgeCount)
        )
    }
    
    // Add new edges for incremental update
    func addingEdges(_ newEdges: [(u: Int, v: Int)], weighted: Bool = false) -> GraphEV {
        var newSources = sources
        var newTargets = targets
        var newWeights = weights
        
        var rng = SystemRandomNumberGenerator()
        
        for edge in newEdges {
            newSources.append(UInt32(min(edge.u, edge.v)))
            newTargets.append(UInt32(max(edge.u, edge.v)))
            if weighted {
                newWeights.append(Float.random(in: 1.0...10.0, using: &rng))
            } else {
                newWeights.append(1.0)
            }
        }
        
        return GraphEV(
            vertexCount: vertexCount,
            edgeCount: edgeCount + newEdges.count,
            sources: newSources,
            targets: newTargets,
            weights: newWeights
        )
    }
}

// MARK: - Full Eigenvector Centrality (CPU - Power Iteration)
func fullEigenvectorCentrality(graph: GraphEV, maxIterations: Int = 100, tolerance: Float = 1e-6) -> (centrality: [Float], iterations: Int, TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    let n = graph.vertexCount
    let m = graph.edgeCount
    
    // Initialize centrality vector (all ones)
    var centrality = Array(repeating: Float(1.0), count: n)
    var newCentrality = Array(repeating: Float(0.0), count: n)
    
    // Build adjacency list for faster computation
    var adjList = Array(repeating: [(neighbor: Int, weight: Float)](), count: n)
    for i in 0..<m {
        let u = Int(graph.sources[i])
        let v = Int(graph.targets[i])
        let w = graph.weights[i]
        adjList[u].append((v, w))
        adjList[v].append((u, w))  // Undirected
    }
    
    // Power iteration
    var iteration = 0
    var converged = false
    
    while iteration < maxIterations && !converged {
        // Compute new centrality: x_{t+1}[i] = sum of neighbors' centrality * weight
        for i in 0..<n {
            newCentrality[i] = 0.0
            for (neighbor, weight) in adjList[i] {
                newCentrality[i] += centrality[neighbor] * weight
            }
        }
        
        // Normalize (divide by L2 norm)
        var norm: Float = 0.0
        var maxDiff: Float = 0.0
        
        for i in 0..<n {
            norm += newCentrality[i] * newCentrality[i]
        }
        norm = sqrt(norm)
        
        for i in 0..<n {
            newCentrality[i] /= norm
            
            // Check convergence
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

// MARK: - Incremental Eigenvector Centrality (CPU - Affected Vertex Queue)
func incrementalEigenvectorCentrality(
    graph: GraphEV,
    existingCentrality: [Float],
    newEdges: [(u: Int, v: Int)],
    maxIterations: Int = 100,
    tolerance: Float = 1e-6
) -> (centrality: [Float], affectedVertices: Int, iterations: Int, TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    let n = graph.vertexCount
    let m = graph.edgeCount
    
    // Start from existing centrality
    var centrality = existingCentrality
    var newCentrality = Array(repeating: Float(0.0), count: n)
    
    // Build adjacency list (including new edges)
    var adjList = Array(repeating: [(neighbor: Int, weight: Float)](), count: n)
    for i in 0..<m {
        let u = Int(graph.sources[i])
        let v = Int(graph.targets[i])
        let w = graph.weights[i]
        adjList[u].append((v, w))
        adjList[v].append((u, w))
    }
    
    // Add new edges to adjacency list
    for edge in newEdges {
        adjList[edge.u].append((edge.v, 1.0))
        adjList[edge.v].append((edge.u, 1.0))
    }
    
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
            
            // Recompute centrality for v
            var newVal: Float = 0.0
            for (neighbor, weight) in adjList[v] {
                newVal += centrality[neighbor] * weight
            }
            
            // Check if v's centrality changed significantly
            let diff = abs(newVal - centrality[v])
            if diff > tolerance {
                newCentrality[v] = newVal
                
                // Add neighbors to queue (their centrality might change)
                for (neighbor, _) in adjList[v] {
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
        
        // Normalize
        var norm: Float = 0.0
        for v in newQueue {
            norm += newCentrality[v] * newCentrality[v]
        }
        for v in queue {
            if !newQueue.contains(v) {
                norm += newCentrality[v] * newCentrality[v]
            }
        }
        norm = sqrt(norm)
        
        for v in queue {
            newCentrality[v] /= norm
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

// MARK: - GPU Kernel (Optional - for comparison)
let eigenvectorKernel = """
#include <metal_stdlib>
using namespace metal;

// Full Eigenvector Centrality (one iteration)
kernel void eigenvector_iteration(
    device const uint32_t* sources [[buffer(0)]],
    device const uint32_t* targets [[buffer(1)]],
    device const float* weights [[buffer(2)]],
    device const float* old_centrality [[buffer(3)]],
    device float* new_centrality [[buffer(4)]],
    constant uint32_t& edge_count [[buffer(5)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= edge_count) return;
    
    uint32_t u = sources[gid];
    uint32_t v = targets[gid];
    float w = weights[gid];
    
    // Atomic add: new_centrality[u] += old_centrality[v] * w
    // atomic_fetch_add is not available in all Metal versions
    // Simplified: This kernel is just for demonstration
}
"""

// MARK: - Main Experiment
func runEigenvectorExperiment() {
    print("=".repeating(count: 60))
    print("Incremental Eigenvector Centrality Experiment")
    print("=".repeating(count: 60))
    print()
    
    // Experiment parameters
    let vertexCount = 10000
    let edgeCount = 50000
    let newEdgeCount = 1000  // 2% of edges
    let maxIterations = 100
    let tolerance: Float = 1e-6
    
    print("Experiment Parameters:")
    print("  Vertex Count: \(vertexCount)")
    print("  Edge Count: \(edgeCount)")
    print("  New Edges: \(newEdgeCount) (\(String(format: "%.1f", Double(newEdgeCount) / Double(edgeCount) * 100))% of edges)")
    print("  Max Iterations: \(maxIterations)")
    print("  Tolerance: \(tolerance)")
    print()
    
    // Generate initial graph
    print("Generating initial graph...")
    let graph = GraphEV.random(vertexCount: vertexCount, edgeCount: edgeCount, weighted: false)
    print("  Done. Edges: \(graph.edgeCount)")
    print()
    
    // Full Eigenvector Centrality
    print("1. Full Eigenvector Centrality (Power Iteration)")
    print("-".repeating(count: 40))
    
    let (fullCentrality, fullIterations, fullTime) = fullEigenvectorCentrality(
        graph: graph,
        maxIterations: maxIterations,
        tolerance: tolerance
    )
    
    print("  Time: \(String(format: "%.2f", fullTime * 1000)) ms")
    print("  Iterations: \(fullIterations)")
    print("  Top 5 vertices: ", terminator: "")
    let sortedIndices = fullCentrality.enumerated().sorted { $0.element > $1.element }.prefix(5)
    for (idx, cent) in sortedIndices {
        print("(\(idx): \(String(format: "%.4f", cent))) ", terminator: "")
    }
    print()
    print()
    
    // Generate new edges
    print("2. Generating \(newEdgeCount) New Edges...")
    print("-".repeating(count: 40))
    
    var rng = SystemRandomNumberGenerator()
    var newEdges: [(u: Int, v: Int)] = []
    newEdges.reserveCapacity(newEdgeCount)
    
    var edgeSet = Set<String>()
    for edge in graph.sources.enumerated() {
        edgeSet.insert("\(edge.element),\(graph.targets[edge.offset])")
    }
    
    while newEdges.count < newEdgeCount {
        let u = Int.random(in: 0..<vertexCount, using: &rng)
        let v = Int.random(in: 0..<vertexCount, using: &rng)
        
        if u != v {
            let key1 = "\(min(u, v)),\(max(u, v))"
            if !edgeSet.contains(key1) {
                edgeSet.insert(key1)
                newEdges.append((u: u, v: v))
            }
        }
    }
    
    print("  Done.")
    print()
    
    // Incremental update
    print("3. Incremental Eigenvector Centrality")
    print("-".repeating(count: 40))
    
    let (incCentrality, affectedVertices, incIterations, incTime) = incrementalEigenvectorCentrality(
        graph: graph,
        existingCentrality: fullCentrality,
        newEdges: newEdges,
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
    
    let graphWithNewEdges = graph.addingEdges(newEdges)
    let (verifyCentrality, verifyIterations, verifyTime) = fullEigenvectorCentrality(
        graph: graphWithNewEdges,
        maxIterations: maxIterations,
        tolerance: tolerance
    )
    
    print("  Time: \(String(format: "%.2f", verifyTime * 1000)) ms")
    print("  Iterations: \(verifyIterations)")
    print("  Top 5 vertices: ", terminator: "")
    let verifySorted = verifyCentrality.enumerated().sorted { $0.element > $1.element }.prefix(5)
    for (idx, cent) in verifySorted {
        print("(\(idx): \(String(format: "%.4f", cent))) ", terminator: "")
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
        "algorithm": "Incremental Eigenvector Centrality",
        "vertexCount": vertexCount,
        "edgeCount": edgeCount,
        "newEdgeCount": newEdgeCount,
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
        let resultsPath = "/tmp/axolotl_tmp/Experiments/eigenvector_results.json"
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
runEigenvectorExperiment()
