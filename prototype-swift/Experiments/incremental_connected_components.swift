//
//  incremental_connected_components.swift
//  Incremental Connected Components using Union-Find
//
//  Algorithm:
//  - Full: Build Union-Find from scratch by processing all edges
//  - Incremental: Only process newly added edges on existing Union-Find
//
//  Why Union-Find?
//  - O(α(V)) amortized per operation (α is inverse Ackermann, ≈ constant)
//  - Naturally supports incremental updates (adding edges)
//  - Path compression + union by rank = extremely fast
//
//  Performance:
//  - Full: O(E * α(V)) ≈ O(E)
//  - Incremental: O(ΔE * α(V)) ≈ O(ΔE)

import Foundation
import QuartzCore  // For CACurrentMediaTime()

// MARK: - Union-Find (Disjoint Set Union)
class UnionFind {
    var parent: [Int]
    var rank: [Int]
    let vertexCount: Int
    
    init(vertexCount: Int) {
        self.vertexCount = vertexCount
        self.parent = Array(0..<vertexCount)
        self.rank = Array(repeating: 0, count: vertexCount)
    }
    
    // Find with path compression
    func find(_ x: Int) -> Int {
        if parent[x] != x {
            parent[x] = find(parent[x])  // Path compression
        }
        return parent[x]
    }
    
    // Union by rank
    func union(_ x: Int, _ y: Int) -> Bool {
        let rootX = find(x)
        let rootY = find(y)
        
        if rootX == rootY {
            return false  // Already in same component
        }
        
        // Union by rank
        if rank[rootX] < rank[rootY] {
            parent[rootX] = rootY
        } else if rank[rootX] > rank[rootY] {
            parent[rootY] = rootX
        } else {
            parent[rootY] = rootX
            rank[rootX] += 1
        }
        
        return true  // Successfully merged
    }
    
    // Get all components
    func getComponents() -> [[Int]] {
        var components: [Int: [Int]] = [:]
        for v in 0..<vertexCount {
            let root = find(v)
            components[root, default: []].append(v)
        }
        return Array(components.values)
    }
    
    // Count number of components
    func countComponents() -> Int {
        var roots = Set<Int>()
        for v in 0..<vertexCount {
            roots.insert(find(v))
        }
        return roots.count
    }
    
    // Copy Union-Find (for incremental updates)
    func copy() -> UnionFind {
        let copy = UnionFind(vertexCount: vertexCount)
        copy.parent = parent
        copy.rank = rank
        return copy
    }
}

// MARK: - Graph Data Structure
struct Graph {
    let vertexCount: Int
    var edges: [(u: Int, v: Int)]  // Undirected edges
    
    init(vertexCount: Int, edgeCount: Int) {
        self.vertexCount = vertexCount
        self.edges = []
        self.edges.reserveCapacity(edgeCount)
        
        // Generate random edges (undirected) with deduplication
        var rng = SystemRandomNumberGenerator()
        var edgeSet = Set<String>()  // For deduplication
        
        for _ in 0..<edgeCount {
            let u = Int.random(in: 0..<vertexCount, using: &rng)
            let v = Int.random(in: 0..<vertexCount, using: &rng)
            if u != v {
                let normalizedEdge = (u: min(u, v), v: max(u, v))
                let key = "\(normalizedEdge.u),\(normalizedEdge.v)"
                if !edgeSet.contains(key) {
                    edgeSet.insert(key)
                    edges.append(normalizedEdge)
                }
            }
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
                let edge = (u: min(u, v), v: max(u, v))
                edges.append(edge)
                newEdges.append(edge)
            }
        }
        
        return newEdges
    }
}

// MARK: - Full Connected Components (Build Union-Find from Scratch)
func fullConnectedComponents(graph: Graph) -> (UnionFind, TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    let uf = UnionFind(vertexCount: graph.vertexCount)
    
    // Process all edges
    for edge in graph.edges {
        _ = uf.union(edge.u, edge.v)  // Return value not needed for full build
    }
    
    let elapsed = CACurrentMediaTime() - startTime
    return (uf, elapsed)
}

// MARK: - Incremental Connected Components (Only Process New Edges)
func incrementalConnectedComponents(graph: Graph, existingUF: UnionFind, newEdges: [(u: Int, v: Int)]) -> (UnionFind, Int, TimeInterval) {
    let startTime = CACurrentMediaTime()
    
    // Copy existing Union-Find (to avoid modifying original)
    let uf = existingUF.copy()
    
    // Only process new edges
    var mergedCount = 0
    for edge in newEdges {
        if uf.union(edge.u, edge.v) {
            mergedCount += 1
        }
    }
    
    let elapsed = CACurrentMediaTime() - startTime
    return (uf, mergedCount, elapsed)
}

// MARK: - Main Experiment
func runConnectedComponentsExperiment() {
    print("=== Incremental Connected Components Experiment ===\n")
    
    // Experiment parameters
    let vertexCount = 100_000
    let initialEdgeCount = 500_000  // Sparse graph
    let newEdgeCount = 5_000  // 1% of initial edges
    
    print("Graph: \(vertexCount) vertices, \(initialEdgeCount) initial edges")
    print("Incremental update: \(newEdgeCount) new edges\n")
    
    // Generate graph
    print("Generating graph...")
    var graph = Graph(vertexCount: vertexCount, edgeCount: initialEdgeCount)
    print("Generated \(graph.edges.count) unique edges\n")
    
    // Method 1: Full recomputation
    print("Method 1: Full Recomputation")
    print("---")
    let (fullUF, fullTime) = fullConnectedComponents(graph: graph)
    let fullComponents = fullUF.countComponents()
    print("Time: \(String(format: "%.4f", fullTime * 1000)) ms")
    print("Number of components: \(fullComponents)")
    
    // Get average component size
    let avgSize = graph.vertexCount / fullComponents
    print("Average component size: \(avgSize)")
    print()
    
    // Method 2: Incremental update
    print("Method 2: Incremental Update")
    print("---")
    
    // Add new edges
    let newEdges = graph.addEdges(count: newEdgeCount)
    print("Added \(newEdges.count) new edges")
    
    // Incremental update (starting from full UF)
    let (incUF, mergedCount, incTime) = incrementalConnectedComponents(
        graph: graph,
        existingUF: fullUF,
        newEdges: newEdges
    )
    let incComponents = incUF.countComponents()
    print("Time: \(String(format: "%.4f", incTime * 1000)) ms")
    print("Number of components: \(incComponents)")
    print("Number of merged components: \(mergedCount)")
    print()
    
    // Speedup
    let speedup = fullTime / incTime
    print("Speedup: \(String(format: "%.2f", speedup))x")
    print()
    
    // Verification: Compare results
    print("Verification:")
    print("---")
    
    // For incremental, we need to compare with "full recomputation after adding edges"
    let (verifyUF, _) = fullConnectedComponents(graph: graph)
    let verifyComponents = verifyUF.countComponents()
    
    // Compare component assignments
    var mismatches = 0
    for v in 0..<vertexCount {
        if incUF.find(v) != verifyUF.find(v) {
            mismatches += 1
        }
    }
    
    if mismatches == 0 {
        print("✅ Results MATCH")
        print("   (Incremental result matches full recomputation after adding edges)")
    } else {
        print("❌ Results MISMATCH: \(mismatches) vertices have different components")
    }
    print()
    
    // Detailed analysis
    print("Detailed Analysis:")
    print("---")
    print("Initial components: \(fullComponents)")
    print("After adding edges (full recomputation): \(verifyComponents)")
    print("After adding edges (incremental): \(incComponents)")
    print("Components reduced: \(fullComponents - verifyComponents)")
    print()
    
    // Performance breakdown
    print("Performance Breakdown:")
    print("---")
    print("Full recomputation time: \(String(format: "%.4f", fullTime * 1000)) ms")
    print("  - Processes \(graph.edges.count - newEdges.count) edges")
    print("Incremental update time: \(String(format: "%.4f", incTime * 1000)) ms")
    print("  - Processes \(newEdges.count) edges")
    print("Theoretical speedup: \(String(format: "%.2f", Double(graph.edges.count - newEdges.count) / Double(newEdges.count)))x")
    print("Actual speedup: \(String(format: "%.2f", speedup))x")
    print()
    
    // Union-Find operation cost analysis
    print("Union-Find Operation Cost:")
    print("---")
    print("find() operation: O(α(V)) ≈ O(1) amortized")
    print("union() operation: O(α(V)) ≈ O(1) amortized")
    print("Building from scratch: O(E * α(V)) ≈ O(E)")
    print("Incremental update: O(ΔE * α(V)) ≈ O(ΔE)")
    print()
    
    // Why incremental is faster
    print("Why Incremental is Faster:")
    print("---")
    print("1. Union-Find operations are extremely fast (≈ 30-50 ns/operation)")
    print("2. Incremental only processes ΔE edges (vs E edges in full)")
    print("3. No need to rebuild the entire data structure")
    print("4. Path compression makes subsequent finds even faster")
    print()
    
    // Summary
    print("Summary:")
    print("---")
    print("Incremental Connected Components achieves \(String(format: "%.0f", speedup))x speedup")
    print("by only processing \(newEdgeCount) new edges (vs \(graph.edges.count) total edges)")
    print()
    print("This demonstrates the power of incremental algorithms:")
    print("When the graph changes slightly, we can update the result efficiently")
    print("without recomputing everything from scratch.")
    print()
    
    // Save results
    let results = """
    {
      "algorithm": "Incremental Connected Components",
      "graph": {
        "vertices": \(vertexCount),
        "initial_edges": \(graph.edges.count - newEdges.count),
        "new_edges": \(newEdges.count),
        "total_edges": \(graph.edges.count)
      },
      "performance": {
        "full_recomputation_ms": \(String(format: "%.4f", fullTime * 1000)),
        "incremental_ms": \(String(format: "%.4f", incTime * 1000)),
        "speedup": \(String(format: "%.2f", speedup))
      },
      "correctness": {
        "matches": \(mismatches == 0),
        "mismatches": \(mismatches)
      },
      "components": {
        "initial": \(fullComponents),
        "after_add_full": \(verifyComponents),
        "after_add_incremental": \(incComponents)
      }
    }
    """
    
    let resultsPath = "/tmp/axolotl_tmp/Experiments/connected_components_results.json"
    try! results.write(toFile: resultsPath, atomically: true, encoding: .utf8)
    print("Results saved to: \(resultsPath)")
}

// Run experiment
runConnectedComponentsExperiment()
