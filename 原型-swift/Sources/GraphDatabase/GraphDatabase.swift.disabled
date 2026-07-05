//
//  GraphDatabase.swift
//  Query API layer for Axolotl Graph Database
//

import Foundation
import QuartzCore

// MARK: - Graph Database (Query API Layer)

/// Main query API for the graph database
public class GraphDatabase {
    
    // MARK: - Properties
    
    /// Underlying storage engine
    public let storage: PersistentGraph
    
    /// Performance statistics
    public var stats: [String: Double] = [:]
    
    // MARK: - Initialization
    
    /// Initialize with existing storage engine
    /// - Parameter storage: PersistentGraph instance
    public init(storage: PersistentGraph = PersistentGraph()) {
        self.storage = storage
    }
    
    /// Initialize with capacity
    /// - Parameter capacity: Expected number of vertices
    public init(capacity: Int) {
        self.storage = PersistentGraph(capacity: capacity)
    }
    
    // MARK: - Vertex CRUD
    
    /// Add a vertex
    /// - Parameter properties: Vertex properties
    /// - Returns: Vertex ID
    public func addVertex(properties: [String: PropertyValue] = [:]) -> VertexID {
        return storage.addVertex(properties: properties)
    }
    
    /// Delete a vertex
    /// - Parameter id: Vertex ID
    public func deleteVertex(id: VertexID) throws {
        try storage.deleteVertex(id: id)
    }
    
    /// Update a vertex
    /// - Parameters:
    ///   - id: Vertex ID
    ///   - properties: New properties (merged with existing)
    public func updateVertex(id: VertexID, properties: [String: PropertyValue]) throws {
        try storage.updateVertex(id: id, properties: properties)
    }
    
    /// Get a vertex
    /// - Parameter id: Vertex ID
    /// - Returns: Vertex or nil
    public func getVertex(id: VertexID) -> Vertex? {
        return storage.getVertex(id: id)
    }
    
    // MARK: - Edge CRUD
    
    /// Add an edge
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    ///   - properties: Edge properties
    ///   - weight: Edge weight
    /// - Returns: Edge ID
    public func addEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue] = [:], weight: Float = 1.0) throws -> EdgeID {
        return try storage.addEdge(from: from, to: to, properties: properties, weight: weight)
    }
    
    /// Delete an edge
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    public func deleteEdge(from: VertexID, to: VertexID) throws {
        try storage.deleteEdge(from: from, to: to)
    }
    
    /// Update an edge
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    ///   - properties: New properties (merged with existing)
    public func updateEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue]) throws {
        try storage.updateEdge(from: from, to: to, properties: properties)
    }
    
    /// Get an edge
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    /// - Returns: Edge or nil
    public func getEdge(from: VertexID, to: VertexID) -> Edge? {
        return storage.getEdge(from: from, to: to)
    }
    
    // MARK: - Basic Queries
    
    /// Get neighbors of a vertex
    /// - Parameter vertex: Vertex ID
    /// - Returns: Array of neighbor vertex IDs
    public func getNeighbors(of vertex: VertexID) -> [VertexID] {
        return storage.getNeighbors(of: vertex)
    }
    
    /// Get degree of a vertex
    /// - Parameter vertex: Vertex ID
    /// - Returns: Degree (number of neighbors)
    public func getDegree(of vertex: VertexID) -> Int {
        return storage.getDegree(of: vertex)
    }
    
    /// Check if edge exists
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    /// - Returns: True if edge exists
    public func hasEdge(from: VertexID, to: VertexID) -> Bool {
        return storage.hasEdge(from: from, to: to)
    }
    
    // MARK: - Graph Traversal Queries
    
    /// Breadth-First Search (BFS)
    /// - Parameters:
    ///   - start: Start vertex ID
    ///   - maxDepth: Maximum depth (default: unlimited)
    /// - Returns: Array of vertex IDs in BFS order
    public func bfs(from start: VertexID, maxDepth: Int = Int.max) -> [VertexID] {
        var visited = Set<VertexID>()
        var queue: [(vertex: VertexID, depth: Int)] = []
        var result: [VertexID] = []
        
        queue.append((start, 0))
        visited.insert(start)
        
        while !queue.isEmpty {
            let (current, depth) = queue.removeFirst()
            result.append(current)
            
            if depth >= maxDepth {
                continue
            }
            
            for neighbor in storage.getNeighbors(of: current) {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor)
                    queue.append((neighbor, depth + 1))
                }
            }
        }
        
        return result
    }
    
    /// Shortest path (BFS-based, unweighted)
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    /// - Returns: Array of vertex IDs representing the shortest path, or empty if no path
    public func shortestPath(from: VertexID, to: VertexID) -> [VertexID] {
        if from == to {
            return [from]
        }
        
        var visited = Set<VertexID>()
        var parent: [VertexID: VertexID] = [:]
        var queue: [VertexID] = []
        
        queue.append(from)
        visited.insert(from)
        
        while !queue.isEmpty {
            let current = queue.removeFirst()
            
            for neighbor in storage.getNeighbors(of: current) {
                if neighbor == to {
                    // Found target, reconstruct path
                    parent[neighbor] = current
                    var path: [VertexID] = [to]
                    var currentNode = current
                    while currentNode != from {
                        path.insert(currentNode, at: 0)
                        currentNode = parent[currentNode]!
                    }
                    path.insert(from, at: 0)
                    return path
                }
                
                if !visited.contains(neighbor) {
                    visited.insert(neighbor)
                    parent[neighbor] = current
                    queue.append(neighbor)
                }
            }
        }
        
        return []  // No path found
    }
    
    /// Get connected component containing a vertex
    /// - Parameter vertex: Vertex ID
    /// - Returns: Array of vertex IDs in the same connected component
    public func getConnectedComponent(containing vertex: VertexID) -> [VertexID] {
        return bfs(from: vertex)
    }
    
    // MARK: - Statistics Queries
    
    /// Get total vertex count
    public var vertexCount: Int {
        return storage.vertexCount
    }
    
    /// Get total edge count
    public var edgeCount: Int {
        return storage.edgeCount
    }
    
    /// Get graph density
    /// - Returns: Density (0.0 to 1.0)
    public func getDensity() -> Double {
        let n = Double(vertexCount)
        let e = Double(edgeCount)
        if n < 2 {
            return 0.0
        }
        return 2.0 * e / (n * (n - 1.0))
    }
    
    /// Get average degree
    public func getAverageDegree() -> Double {
        if vertexCount == 0 {
            return 0.0
        }
        return 2.0 * Double(edgeCount) / Double(vertexCount)
    }
    
    // MARK: - Batch Operations
    
    /// Add multiple vertices
    /// - Parameter count: Number of vertices to add
    /// - Returns: Array of new vertex IDs
    public func addVertices(count: Int, propertiesGenerator: ((VertexID) -> [String: PropertyValue])? = nil) -> [VertexID] {
        var ids: [VertexID] = []
        ids.reserveCapacity(count)
        
        for i in 0..<count {
            let props = propertiesGenerator?(storage.nextVertexID + UInt64(i)) ?? [:]
            let id = addVertex(properties: props)
            ids.append(id)
        }
        
        return ids
    }
    
    /// Add random edges (for testing)
    /// - Parameters:
    ///   - count: Number of edges to add
    ///   - weighted: Whether edges should have random weights
    public func addRandomEdges(count: Int, weighted: Bool = false) throws {
        let vertexCount = storage.vertexCount
        guard vertexCount > 0 else {
            throw GraphError.invalidOperation("No vertices in graph")
        }
        
        var rng = SystemRandomNumberGenerator()
        
        for _ in 0..<count {
            let from = VertexID.random(in: 0..<VertexID(vertexCount), using: &rng)
            let to = VertexID.random(in: 0..<VertexID(vertexCount), using: &rng)
            
            if from != to && !storage.hasEdge(from: from, to: to) {
                let weight = weighted ? Float.random(in: 1.0...10.0, using: &rng) : 1.0
                _ = try addEdge(from: from, to: to, weight: weight)
            }
        }
    }
    
    // MARK: - Persistence
    
    /// Save graph to file (JSON format)
    /// - Parameter path: File path
    public func save(to path: String) throws {
        try storage.saveToJSON(to: path)
    }
    
    /// Load graph from file (JSON format)
    /// - Parameter path: File path
    public func load(from path: String) throws {
        try storage.loadFromJSON(from: path)
    }
    
    // MARK: - Incremental Algorithm Integration (Placeholder)
    
    /// Compute PageRank (full recomputation)
    /// - Parameters:
    ///   - dampingFactor: Damping factor (default: 0.85)
    ///   - maxIterations: Maximum iterations (default: 100)
    ///   - tolerance: Convergence tolerance (default: 1e-6)
    /// - Returns: (scores, iterations, time)
    public func computePageRank(dampingFactor: Float = 0.85, maxIterations: Int = 100, tolerance: Float = 1e-6) -> ([Float], Int, TimeInterval) {
        let startTime = CACurrentMediaTime()
        
        let n = storage.vertexCount
        guard n > 0 else {
            return ([], 0, 0)
        }
        
        // Initialize scores
        var scores = Array(repeating: Float(1.0) / Float(n), count: n)
        var newScores = Array(repeating: Float(0.0), count: n)
        
        // Build adjacency list (for faster computation)
        let adjList = storage.adjacencyList
        
        // Power iteration
        var iteration = 0
        var converged = false
        
        while iteration < maxIterations && !converged {
            // Compute new scores
            for i in 0..<n {
                var sum: Float = 0.0
                for neighbor in adjList[i] {
                    sum += scores[neighbor] / Float(storage.getDegree(of: neighbor))
                }
                newScores[i] = (1.0 - dampingFactor) / Float(n) + dampingFactor * sum
            }
            
            // Check convergence
            var maxDiff: Float = 0.0
            for i in 0..<n {
                let diff = abs(newScores[i] - scores[i])
                if diff > maxDiff {
                    maxDiff = diff
                }
            }
            
            // Swap
            swap(&scores, &newScores)
            
            iteration += 1
            
            if maxDiff < tolerance {
                converged = true
            }
        }
        
        let elapsed = CACurrentMediaTime() - startTime
        return (scores, iteration, elapsed)
    }
    
    /// Clear all data
    public func clear() {
        storage.vertices.removeAll()
        storage.edges.removeAll()
        storage.adjacencyList.removeAll()
        storage.reverseAdjacencyList.removeAll()
        storage.nextVertexID = 0
        storage.changeLog.removeAll()
    }
}
