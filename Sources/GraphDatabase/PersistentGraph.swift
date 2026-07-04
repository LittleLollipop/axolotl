//
//  PersistentGraph.swift
//  Core storage engine for Axolotl Graph Database
//

import Foundation

// MARK: - Persistent Graph (Storage Engine)

/// Core storage engine for the graph database
public class PersistentGraph {
    
    // MARK: - In-Memory Storage
    
    /// Vertices stored by ID
    public var vertices: [VertexID: Vertex] = [:]
    
    /// Edges stored by ID
    public var edges: [EdgeID: Edge] = [:]
    
    /// Adjacency list for fast traversal
    public var adjacencyList: [[VertexID]] = []
    
    /// Reverse adjacency list (for fast incoming edge lookup)
    public var reverseAdjacencyList: [[VertexID]] = []
    
    /// Next vertex ID (auto-increment)
    public var nextVertexID: VertexID = 0
    
    /// Change log (for incremental persistence)
    public var changeLog: [Change] = []
    
    // MARK: - Initialization
    
    public init() {
        // Initialize with empty storage
    }
    
    public init(capacity: Int) {
        vertices.reserveCapacity(capacity)
        edges.reserveCapacity(capacity * 10)  // Assume ~10 edges per vertex
        adjacencyList.reserveCapacity(capacity)
        reverseAdjacencyList.reserveCapacity(capacity)
    }
    
    // MARK: - Vertex Operations
    
    /// Add a vertex
    /// - Parameter properties: Vertex properties
    /// - Returns: Vertex ID
    public func addVertex(properties: [String: PropertyValue] = [:]) -> VertexID {
        let id = nextVertexID
        nextVertexID += 1
        
        let vertex = Vertex(id: id, properties: properties)
        vertices[id] = vertex
        
        // Ensure adjacency lists are large enough
        while adjacencyList.count <= Int(id) {
            adjacencyList.append([])
            reverseAdjacencyList.append([])
        }
        
        // Log change
        changeLog.append(.vertexAdded(vertex))
        
        return id
    }
    
    /// Delete a vertex
    /// - Parameter id: Vertex ID
    /// - Throws: GraphError.vertexNotFound
    public func deleteVertex(id: VertexID) throws {
        guard let vertex = vertices[id] else {
            throw GraphError.vertexNotFound(id)
        }
        
        // Remove all edges connected to this vertex
        let neighbors = adjacencyList[Int(id)]
        for neighbor in neighbors {
            let edgeID: EdgeID = (min(id, neighbor), max(id, neighbor))
            if let edge = edges[edgeID] {
                try deleteEdge(from: edge.from, to: edge.to)
            }
        }
        
        // Remove vertex
        vertices.removeValue(forKey: id)
        adjacencyList[Int(id)] = []
        reverseAdjacencyList[Int(id)] = []
        
        // Log change
        changeLog.append(.vertexDeleted(id))
    }
    
    /// Update a vertex
    /// - Parameters:
    ///   - id: Vertex ID
    ///   - properties: New properties (merged with existing)
    /// - Throws: GraphError.vertexNotFound
    public func updateVertex(id: VertexID, properties: [String: PropertyValue]) throws {
        guard var vertex = vertices[id] else {
            throw GraphError.vertexNotFound(id)
        }
        
        // Merge properties
        for (key, value) in properties {
            vertex.properties[key] = value
        }
        
        vertices[id] = vertex
        
        // Log change
        changeLog.append(.vertexUpdated(id, properties))
    }
    
    /// Get a vertex
    /// - Parameter id: Vertex ID
    /// - Returns: Vertex or nil
    public func getVertex(id: VertexID) -> Vertex? {
        return vertices[id]
    }
    
    // MARK: - Edge Operations
    
    /// Add an edge
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    ///   - properties: Edge properties
    ///   - weight: Edge weight
    /// - Throws: GraphError.vertexNotFound, GraphError.edgeAlreadyExists
    public func addEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue] = [:], weight: Float = 1.0) throws -> EdgeID {
        
        // Check if vertices exist
        guard vertices[from] != nil else {
            throw GraphError.vertexNotFound(from)
        }
        guard vertices[to] != nil else {
            throw GraphError.vertexNotFound(to)
        }
        
        // Check if edge already exists
        let edgeID: EdgeID = (min(from, to), max(from, to))
        if edges[edgeID] != nil {
            throw GraphError.edgeAlreadyExists(from, to)
        }
        
        // Add edge
        let edge = Edge(from: from, to: to, properties: properties, weight: weight)
        edges[edgeID] = edge
        
        // Update adjacency lists
        adjacencyList[Int(from)].append(to)
        adjacencyList[Int(to)].append(from)
        reverseAdjacencyList[Int(from)].append(to)
        reverseAdjacencyList[Int(to)].append(from)
        
        // Log change
        changeLog.append(.edgeAdded(edge))
        
        return edgeID
    }
    
    /// Delete an edge
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    /// - Throws: GraphError.edgeNotFound
    public func deleteEdge(from: VertexID, to: VertexID) throws {
        let edgeID: EdgeID = (min(from, to), max(from, to))
        
        guard edges[edgeID] != nil else {
            throw GraphError.edgeNotFound(from, to)
        }
        
        // Remove edge
        edges.removeValue(forKey: edgeID)
        
        // Update adjacency lists
        adjacencyList[Int(from)].removeAll { $0 == to }
        adjacencyList[Int(to)].removeAll { $0 == from }
        reverseAdjacencyList[Int(from)].removeAll { $0 == to }
        reverseAdjacencyList[Int(to)].removeAll { $0 == from }
        
        // Log change
        changeLog.append(.edgeDeleted(edgeID))
    }
    
    /// Update an edge
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    ///   - properties: New properties (merged with existing)
    /// - Throws: GraphError.edgeNotFound
    public func updateEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue]) throws {
        let edgeID: EdgeID = (min(from, to), max(from, to))
        
        guard var edge = edges[edgeID] else {
            throw GraphError.edgeNotFound(from, to)
        }
        
        // Merge properties
        for (key, value) in properties {
            edge.properties[key] = value
        }
        
        edges[edgeID] = edge
        
        // Log change
        changeLog.append(.edgeUpdated(edgeID, properties))
    }
    
    /// Get an edge
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    /// - Returns: Edge or nil
    public func getEdge(from: VertexID, to: VertexID) -> Edge? {
        let edgeID: EdgeID = (min(from, to), max(from, to))
        return edges[edgeID]
    }
    
    // MARK: - Query Operations
    
    /// Get neighbors of a vertex
    /// - Parameter vertex: Vertex ID
    /// - Returns: Array of neighbor vertex IDs
    public func getNeighbors(of vertex: VertexID) -> [VertexID] {
        guard Int(vertex) < adjacencyList.count else {
            return []
        }
        return adjacencyList[Int(vertex)]
    }
    
    /// Get incoming neighbors (vertices that have edges to this vertex)
    /// - Parameter vertex: Vertex ID
    /// - Returns: Array of incoming neighbor vertex IDs
    public func getIncomingNeighbors(of vertex: VertexID) -> [VertexID] {
        guard Int(vertex) < reverseAdjacencyList.count else {
            return []
        }
        return reverseAdjacencyList[Int(vertex)]
    }
    
    /// Get degree of a vertex
    /// - Parameter vertex: Vertex ID
    /// - Returns: Degree (number of neighbors)
    public func getDegree(of vertex: VertexID) -> Int {
        return getNeighbors(of: vertex).count
    }
    
    /// Check if edge exists
    /// - Parameters:
    ///   - from: Source vertex ID
    ///   - to: Target vertex ID
    /// - Returns: True if edge exists
    public func hasEdge(from: VertexID, to: VertexID) -> Bool {
        let edgeID: EdgeID = (min(from, to), max(from, to))
        return edges[edgeID] != nil
    }
    
    // MARK: - Persistence (JSON Format - Simple & Debuggable)
    
    /// Save graph to JSON file
    /// - Parameter path: File path
    /// - Throws: Error
    public func saveToJSON(to path: String) throws {
        let data = try JSONEncoder().encode(vertices.map { $0.value })
        try data.write(to: URL(fileURLWithPath: path))
    }
    
    /// Load graph from JSON file
    /// - Parameter path: File path
    /// - Throws: Error
    public func loadFromJSON(from path: String) throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        let loadedVertices = try JSONDecoder().decode([Vertex].self, from: data)
        
        // Rebuild in-memory storage
        vertices.removeAll()
        edges.removeAll()
        adjacencyList.removeAll()
        reverseAdjacencyList.removeAll()
        
        for vertex in loadedVertices {
            vertices[vertex.id] = vertex
            if Int(vertex.id) >= adjacencyList.count {
                adjacencyList.append([])
                reverseAdjacencyList.append([])
            }
        }
        
        // TODO: Also load edges (need to extend JSON format)
    }
    
    // MARK: - Statistics
    
    /// Get total vertex count
    public var vertexCount: Int {
        return vertices.count
    }
    
    /// Get total edge count
    public var edgeCount: Int {
        return edges.count
    }
    
    /// Clear change log (call after saving)
    public func clearChangeLog() {
        changeLog.removeAll()
    }
}
