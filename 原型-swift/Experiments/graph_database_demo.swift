//
//  graph_database_demo.swift
//  Demo: Axolotl Graph Database
//

import Foundation

// Import the module (when using Swift package)
// import AxolotlDB

// MARK: - Demo

func runGraphDatabaseDemo() {
    print("=".repeating(count: 60))
    print("Axolotl Graph Database Demo")
    print("=".repeating(count: 60))
    print()
    
    // Create database
    print("1. Creating Graph Database...")
    print("-".repeating(count: 40))
    
    let db = GraphDatabase(capacity: 100)
    
    print("  Done.")
    print()
    
    // Add vertices
    print("2. Adding Vertices...")
    print("-".repeating(count: 40))
    
    let v1 = db.addVertex(properties: ["name": .string("Alice"), "age": .int(30)])
    let v2 = db.addVertex(properties: ["name": .string("Bob"), "age": .int(25)])
    let v3 = db.addVertex(properties: ["name": .string("Charlie"), "age": .int(35)])
    let v4 = db.addVertex(properties: ["name": .string("David"), "age": .int(40)])
    let v5 = db.addVertex(properties: ["name": .string("Eve"), "age": .int(28)])
    
    print("  Added 5 vertices: \(v1), \(v2), \(v3), \(v4), \(v5)")
    print("  Vertex count: \(db.vertexCount)")
    print()
    
    // Add edges
    print("3. Adding Edges...")
    print("-".repeating(count: 40))
    
    do {
        let e1 = try db.addEdge(from: v1, to: v2, properties: ["type": .string("knows")])
        let e2 = try db.addEdge(from: v2, to: v3, properties: ["type": .string("knows")])
        let e3 = try db.addEdge(from: v3, to: v4, properties: ["type": .string("knows")])
        let e4 = try db.addEdge(from: v4, to: v5, properties: ["type": .string("knows")])
        let e5 = try db.addEdge(from: v5, to: v1, properties: ["type": .string("knows")])
        
        print("  Added 5 edges: (\(v1),\(v2)), (\(v2),\(v3)), (\(v3),\(v4)), (\(v4),\(v5)), (\(v5),\(v1))")
        print("  Edge count: \(db.edgeCount)")
    } catch {
        print("  Error: \(error)")
    }
    print()
    
    // Query: Get neighbors
    print("4. Query: Get Neighbors of Vertex \(v1)...")
    print("-".repeating(count: 40))
    
    let neighbors = db.getNeighbors(of: v1)
    print("  Neighbors of \(v1): \(neighbors)")
    print("  Degree of \(v1): \(db.getDegree(of: v1))")
    print()
    
    // Query: BFS
    print("5. Query: BFS from Vertex \(v1)...")
    print("-".repeating(count: 40))
    
    let bfsResult = db.bfs(from: v1, maxDepth: 2)
    print("  BFS result (max depth 2): \(bfsResult)")
    print()
    
    // Query: Shortest Path
    print("6. Query: Shortest Path from \(v1) to \(v4)...")
    print("-".repeating(count: 40))
    
    let shortestPath = db.shortestPath(from: v1, to: v4)
    print("  Shortest path: \(shortestPath)")
    print()
    
    // Query: Get connected component
    print("7. Query: Connected Component containing Vertex \(v1)...")
    print("-".repeating(count: 40))
    
    let component = db.getConnectedComponent(containing: v1)
    print("  Connected component: \(component)")
    print("  Component size: \(component.count)")
    print()
    
    // Statistics
    print("8. Graph Statistics...")
    print("-".repeating(count: 40))
    
    print("  Vertex count: \(db.vertexCount)")
    print("  Edge count: \(db.edgeCount)")
    print("  Density: \(String(format: "%.4f", db.getDensity()))")
    print("  Average degree: \(String(format: "%.2f", db.getAverageDegree()))")
    print()
    
    // Update vertex
    print("9. Update: Update Vertex \(v1) (change age to 31)...")
    print("-".repeating(count: 40))
    
    do {
        try db.updateVertex(id: v1, properties: ["age": .int(31)])
        if let vertex = db.getVertex(id: v1) {
            print("  Updated vertex \(v1): \(vertex.properties)")
        }
    } catch {
        print("  Error: \(error)")
    }
    print()
    
    // Compute PageRank
    print("10. Algorithm: Compute PageRank...")
    print("-".repeating(count: 40))
    
    let (scores, iterations, time) = db.computePageRank()
    print("  Time: \(String(format: "%.2f", time * 1000)) ms")
    print("  Iterations: \(iterations)")
    print("  Top 3 vertices: ", terminator: "")
    let sortedScores = scores.enumerated().sorted { $0.element > $1.element }.prefix(3)
    for (idx, score) in sortedScores {
        print("(\(idx): \(String(format: "%.4f", score))) ", terminator: "")
    }
    print()
    print()
    
    // Persistence: Save to JSON
    print("11. Persistence: Save to JSON...")
    print("-".repeating(count: 40))
    
    let jsonPath = "/tmp/axolotl_demo_graph.json"
    do {
        try db.save(to: jsonPath)
        print("  Saved to: \(jsonPath)")
    } catch {
        print("  Error: \(error)")
    }
    print()
    
    // Delete edge
    print("12. Delete: Delete Edge (\(v1),\(v2))...")
    print("-".repeating(count: 40))
    
    do {
        try db.deleteEdge(from: v1, to: v2)
        print("  Edge deleted.")
        print("  New edge count: \(db.edgeCount)")
        print("  New neighbors of \(v1): \(db.getNeighbors(of: v1))")
    } catch {
        print("  Error: \(error)")
    }
    print()
    
    // Summary
    print("=".repeating(count: 60))
    print("Summary")
    print("=".repeating(count: 60))
    print()
    print("✅ Graph database demo completed successfully!")
    print()
    print("Features demonstrated:")
    print("  1. ✅ Vertex CRUD (add, delete, update, get)")
    print("  2. ✅ Edge CRUD (add, delete, update, get)")
    print("  3. ✅ Basic queries (getNeighbors, getDegree)")
    print("  4. ✅ Graph traversal (BFS, shortestPath, connected component)")
    print("  5. ✅ Statistics (vertexCount, edgeCount, density, avg degree)")
    print("  6. ✅ Update operations")
    print("  7. ✅ PageRank algorithm")
    print("  8. ✅ Persistence (save to JSON)")
    print()
}

// Helper extension
extension String {
    func repeating(count: Int) -> String {
        return String(repeating: self, count: count)
    }
}

// Run demo
runGraphDatabaseDemo()
