//
//  test_graph_database.swift
//  Standalone test for Axolotl Graph Database
//

import Foundation

// MARK: - Core Types (Inline for Testing)

typealias VertexID = UInt64

/// Edge ID (struct for Hashable conformance)
struct EdgeID: Hashable {
    let from: VertexID
    let to: VertexID
    
    init(from: VertexID, to: VertexID) {
        self.from = min(from, to)  // Normalize: always (smaller, larger)
        self.to = max(from, to)
    }
}

enum PropertyValue: Codable, Equatable, Hashable {
    case int(Int)
    case double(Double)
    case string(String)
    case bool(Bool)
    case float(Float)
    case data(Data)
    
    enum CodingKeys: String, CodingKey {
        case type, value
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .int(let value):
            try container.encode("int", forKey: .type)
            try container.encode(value, forKey: .value)
        case .double(let value):
            try container.encode("double", forKey: .type)
            try container.encode(value, forKey: .value)
        case .string(let value):
            try container.encode("string", forKey: .type)
            try container.encode(value, forKey: .value)
        case .bool(let value):
            try container.encode("bool", forKey: .type)
            try container.encode(value, forKey: .value)
        case .float(let value):
            try container.encode("float", forKey: .type)
            try container.encode(value, forKey: .value)
        case .data(let value):
            try container.encode("data", forKey: .type)
            try container.encode(value, forKey: .value)
        }
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "int":
            self = .int(try container.decode(Int.self, forKey: .value))
        case "double":
            self = .double(try container.decode(Double.self, forKey: .value))
        case "string":
            self = .string(try container.decode(String.self, forKey: .value))
        case "bool":
            self = .bool(try container.decode(Bool.self, forKey: .value))
        case "float":
            self = .float(try container.decode(Float.self, forKey: .value))
        case "data":
            self = .data(try container.decode(Data.self, forKey: .value))
        default:
            throw DecodingError.dataCorrupted(DecodingError.Context(codingPath: [], debugDescription: "Unknown type: \(type)"))
        }
    }
}

struct Vertex: Codable, Equatable, Hashable {
    let id: VertexID
    var properties: [String: PropertyValue]
    
    init(id: VertexID, properties: [String: PropertyValue] = [:]) {
        self.id = id
        self.properties = properties
    }
}

struct Edge: Codable, Equatable, Hashable {
    let from: VertexID
    let to: VertexID
    var properties: [String: PropertyValue]
    var weight: Float
    
    init(from: VertexID, to: VertexID, properties: [String: PropertyValue] = [:], weight: Float = 1.0) {
        self.from = from
        self.to = to
        self.properties = properties
        self.weight = weight
    }
    
    var id: EdgeID {
        return (from, to)
    }
}

enum GraphError: Error, LocalizedError {
    case vertexNotFound(VertexID)
    case edgeNotFound(VertexID, VertexID)
    case vertexAlreadyExists(VertexID)
    case edgeAlreadyExists(VertexID, VertexID)
    case invalidOperation(String)
    
    var errorDescription: String? {
        switch self {
        case .vertexNotFound(let id):
            return "Vertex \(id) not found"
        case .edgeNotFound(let from, let to):
            return "Edge (\(from), \(to)) not found"
        case .vertexAlreadyExists(let id):
            return "Vertex \(id) already exists"
        case .edgeAlreadyExists(let from, let to):
            return "Edge (\(from), \(to)) already exists"
        case .invalidOperation(let msg):
            return "Invalid operation: \(msg)"
        }
    }
}

// MARK: - Persistent Graph (Storage Engine)

class PersistentGraph {
    var vertices: [VertexID: Vertex] = [:]
    var edges: [EdgeID: Edge] = [:]
    var adjacencyList: [[VertexID]] = []
    var reverseAdjacencyList: [[VertexID]] = []
    var nextVertexID: VertexID = 0
    
    func addVertex(properties: [String: PropertyValue] = [:]) -> VertexID {
        let id = nextVertexID
        nextVertexID += 1
        
        let vertex = Vertex(id: id, properties: properties)
        vertices[id] = vertex
        
        while adjacencyList.count <= Int(id) {
            adjacencyList.append([])
            reverseAdjacencyList.append([])
        }
        
        return id
    }
    
    func deleteVertex(id: VertexID) throws {
        guard vertices[id] != nil else {
            throw GraphError.vertexNotFound(id)
        }
        
        let neighbors = adjacencyList[Int(id)]
        for neighbor in neighbors {
            try deleteEdge(from: id, to: neighbor)
        }
        
        vertices.removeValue(forKey: id)
        adjacencyList[Int(id)] = []
        reverseAdjacencyList[Int(id)] = []
    }
    
    func updateVertex(id: VertexID, properties: [String: PropertyValue]) throws {
        guard var vertex = vertices[id] else {
            throw GraphError.vertexNotFound(id)
        }
        
        for (key, value) in properties {
            vertex.properties[key] = value
        }
        
        vertices[id] = vertex
    }
    
    func getVertex(id: VertexID) -> Vertex? {
        return vertices[id]
    }
    
    func addEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue] = [:], weight: Float = 1.0) throws -> EdgeID {
        guard vertices[from] != nil else {
            throw GraphError.vertexNotFound(from)
        }
        guard vertices[to] != nil else {
            throw GraphError.vertexNotFound(to)
        }
        
        let edgeID: EdgeID = (min(from, to), max(from, to))
        if edges[edgeID] != nil {
            throw GraphError.edgeAlreadyExists(from, to)
        }
        
        let edge = Edge(from: from, to: to, properties: properties, weight: weight)
        edges[edgeID] = edge
        
        adjacencyList[Int(from)].append(to)
        adjacencyList[Int(to)].append(from)
        
        return edgeID
    }
    
    func deleteEdge(from: VertexID, to: VertexID) throws {
        let edgeID: EdgeID = (min(from, to), max(from, to))
        
        guard edges[edgeID] != nil else {
            throw GraphError.edgeNotFound(from, to)
        }
        
        edges.removeValue(forKey: edgeID)
        
        adjacencyList[Int(from)].removeAll { $0 == to }
        adjacencyList[Int(to)].removeAll { $0 == from }
    }
    
    func getNeighbors(of vertex: VertexID) -> [VertexID] {
        guard Int(vertex) < adjacencyList.count else {
            return []
        }
        return adjacencyList[Int(vertex)]
    }
    
    func getDegree(of vertex: VertexID) -> Int {
        return getNeighbors(of: vertex).count
    }
    
    var vertexCount: Int {
        return vertices.count
    }
    
    var edgeCount: Int {
        return edges.count
    }
}

// MARK: - Graph Database (Query API)

class GraphDatabase {
    let storage: PersistentGraph
    
    init() {
        self.storage = PersistentGraph()
    }
    
    func addVertex(properties: [String: PropertyValue] = [:]) -> VertexID {
        return storage.addVertex(properties: properties)
    }
    
    func deleteVertex(id: VertexID) throws {
        try storage.deleteVertex(id: id)
    }
    
    func addEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue] = [:], weight: Float = 1.0) throws -> EdgeID {
        return try storage.addEdge(from: from, to: to, properties: properties, weight: weight)
    }
    
    func getNeighbors(of vertex: VertexID) -> [VertexID] {
        return storage.getNeighbors(of: vertex)
    }
    
    func getDegree(of vertex: VertexID) -> Int {
        return storage.getDegree(of: vertex)
    }
    
    func bfs(from start: VertexID, maxDepth: Int = Int.max) -> [VertexID] {
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
    
    func shortestPath(from: VertexID, to: VertexID) -> [VertexID] {
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
        
        return []
    }
    
    var vertexCount: Int {
        return storage.vertexCount
    }
    
    var edgeCount: Int {
        return storage.edgeCount
    }
}

// MARK: - Test

func testGraphDatabase() {
    print("=" * 60)
    print("Axolotl Graph Database - Standalone Test")
    print("=" * 60)
    print()
    
    // Create database
    print("1. Creating Graph Database...")
    let db = GraphDatabase()
    print("  Done.")
    print()
    
    // Add vertices
    print("2. Adding Vertices...")
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
    do {
        _ = try db.addEdge(from: v1, to: v2)
        _ = try db.addEdge(from: v2, to: v3)
        _ = try db.addEdge(from: v3, to: v4)
        _ = try db.addEdge(from: v4, to: v5)
        _ = try db.addEdge(from: v5, to: v1)
        
        print("  Added 5 edges")
        print("  Edge count: \(db.edgeCount)")
    } catch {
        print("  Error: \(error)")
    }
    print()
    
    // Query: Get neighbors
    print("4. Query: Get Neighbors of Vertex \(v1)...")
    let neighbors = db.getNeighbors(of: v1)
    print("  Neighbors of \(v1): \(neighbors)")
    print("  Degree of \(v1): \(db.getDegree(of: v1))")
    print()
    
    // Query: BFS
    print("5. Query: BFS from Vertex \(v1)...")
    let bfsResult = db.bfs(from: v1, maxDepth: 2)
    print("  BFS result (max depth 2): \(bfsResult)")
    print()
    
    // Query: Shortest Path
    print("6. Query: Shortest Path from \(v1) to \(v4)...")
    let shortestPath = db.shortestPath(from: v1, to: v4)
    print("  Shortest path: \(shortestPath)")
    print()
    
    // Statistics
    print("7. Statistics...")
    print("  Vertex count: \(db.vertexCount)")
    print("  Edge count: \(db.edgeCount)")
    print()
    
    print("=" * 60)
    print("Test Completed Successfully!")
    print("=" * 60)
}

// Helper
extension String {
    static func * (lhs: String, rhs: Int) -> String {
        return String(repeating: lhs, count: rhs)
    }
}

// Run test
testGraphDatabase()
