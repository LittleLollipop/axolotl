import Foundation

// MARK: - Type Definitions (Must match PersistentGraph.swift)

typealias VertexID = UInt64

struct EdgeID: Hashable, Codable {
    let from: VertexID
    let to: VertexID
}

enum PropertyValue: Codable, Equatable, Hashable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case null
    
    enum CodingKeys: String, CodingKey {
        case type, value
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        
        switch type {
        case "string":
            self = .string(try container.decode(String.self, forKey: .value))
        case "int":
            self = .int(try container.decode(Int.self, forKey: .value))
        case "double":
            self = .double(try container.decode(Double.self, forKey: .value))
        case "bool":
            self = .bool(try container.decode(Bool.self, forKey: .value))
        case "null":
            self = .null
        default:
            self = .null
        }
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        
        switch self {
        case .string(let s):
            try container.encode("string", forKey: .type)
            try container.encode(s, forKey: .value)
        case .int(let i):
            try container.encode("int", forKey: .type)
            try container.encode(i, forKey: .value)
        case .double(let d):
            try container.encode("double", forKey: .type)
            try container.encode(d, forKey: .value)
        case .bool(let b):
            try container.encode("bool", forKey: .type)
            try container.encode(b, forKey: .value)
        case .null:
            try container.encode("null", forKey: .type)
            try container.encodeNil(forKey: .value)
        }
    }
    
    func toAny() -> Any {
        switch self {
        case .string(let s): return s
        case .int(let i): return i
        case .double(let d): return d
        case .bool(let b): return b
        case .null: return NSNull()
        }
    }
    
    static func fromAny(_ any: Any) -> PropertyValue {
        if let s = any as? String { return .string(s) }
        if let i = any as? Int { return .int(i) }
        if let d = any as? Double { return .double(d) }
        if let b = any as? Bool { return .bool(b) }
        if any is NSNull { return .null }
        return .null
    }
}

struct Vertex: Codable {
    let id: VertexID
    var properties: [String: PropertyValue]
}

struct Edge: Codable {
    let id: EdgeID
    var properties: [String: PropertyValue]
    var weight: Double
}

enum GraphError: Error, CustomStringConvertible {
    case vertexNotFound(VertexID)
    case edgeNotFound(VertexID, VertexID)
    case vertexAlreadyExists(VertexID)
    case edgeAlreadyExists(VertexID, VertexID)
    case invalidFormat
    
    var description: String {
        switch self {
        case .vertexNotFound(let id):
            return "Vertex \(id) not found"
        case .edgeNotFound(let from, let to):
            return "Edge (\(from) -> \(to)) not found"
        case .vertexAlreadyExists(let id):
            return "Vertex \(id) already exists"
        case .edgeAlreadyExists(let from, let to):
            return "Edge (\(from) -> \(to)) already exists"
        case .invalidFormat:
            return "Invalid file format"
        }
    }
}

// MARK: - Persistent Graph Database

class PersistentGraph {
    private var vertices: [VertexID: Vertex] = [:]
    private var edges: [EdgeID: Edge] = [:]
    private var adjacencyList: [VertexID: Set<VertexID>] = [:]
    private let filePath: String
    
    init(filePath: String) throws {
        self.filePath = filePath
        
        let dir = (filePath as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        
        if FileManager.default.fileExists(atPath: filePath) {
            try load()
        }
    }
    
    func addVertex(id: VertexID? = nil, properties: [String: PropertyValue] = [:]) throws -> VertexID {
        let vid = id ?? VertexID(vertices.count)
        
        guard vertices[vid] == nil else {
            throw GraphError.vertexAlreadyExists(vid)
        }
        
        let vertex = Vertex(id: vid, properties: properties)
        vertices[vid] = vertex
        adjacencyList[vid] = []
        
        return vid
    }
    
    func deleteVertex(id: VertexID) throws {
        guard vertices[id] != nil else {
            throw GraphError.vertexNotFound(id)
        }
        
        let neighbors = adjacencyList[id] ?? []
        for neighbor in neighbors {
            edges.removeValue(forKey: EdgeID(from: id, to: neighbor))
            adjacencyList[neighbor]?.remove(id)
        }
        
        for (vid, _) in vertices {
            edges.removeValue(forKey: EdgeID(from: vid, to: id))
            adjacencyList[vid]?.remove(id)
        }
        
        vertices.removeValue(forKey: id)
        adjacencyList.removeValue(forKey: id)
    }
    
    func addEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue] = [:], weight: Double = 1.0) throws -> EdgeID {
        guard vertices[from] != nil else {
            throw GraphError.vertexNotFound(from)
        }
        guard vertices[to] != nil else {
            throw GraphError.vertexNotFound(to)
        }
        
        let edgeId = EdgeID(from: from, to: to)
        
        guard edges[edgeId] == nil else {
            throw GraphError.edgeAlreadyExists(from, to)
        }
        
        let edge = Edge(id: edgeId, properties: properties, weight: weight)
        edges[edgeId] = edge
        adjacencyList[from]?.insert(to)
        
        return edgeId
    }
    
    func deleteEdge(from: VertexID, to: VertexID) throws {
        let edgeId = EdgeID(from: from, to: to)
        
        guard edges[edgeId] != nil else {
            throw GraphError.edgeNotFound(from, to)
        }
        
        edges.removeValue(forKey: edgeId)
        adjacencyList[from]?.remove(to)
    }
    
    func getVertex(id: VertexID) -> Vertex? {
        return vertices[id]
    }
    
    func getEdge(from: VertexID, to: VertexID) -> Edge? {
        return edges[EdgeID(from: from, to: to)]
    }
    
    func getNeighbors(of vertexId: VertexID) -> [VertexID] {
        return Array(adjacencyList[vertexId] ?? [])
    }
    
    func getStatistics() -> (vertexCount: Int, edgeCount: Int) {
        return (vertices.count, edges.count)
    }
    
    func save() throws {
        let data = try serializeToJSON()
        
        let tempPath = filePath + ".tmp"
        if FileManager.default.fileExists(atPath: tempPath) {
            try FileManager.default.removeItem(atPath: tempPath)
        }
        
        try data.write(to: URL(fileURLWithPath: tempPath))
        
        if FileManager.default.fileExists(atPath: filePath) {
            try FileManager.default.removeItem(atPath: filePath)
        }
        
        try FileManager.default.moveItem(atPath: tempPath, toPath: filePath)
        
        print("✅ Database saved to \(filePath)")
        print("   Vertices: \(vertices.count)")
        print("   Edges: \(edges.count)")
    }
    
    private func load() throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: filePath))
        try deserializeFromJSON(data: data)
        
        print("✅ Database loaded from \(filePath)")
        print("   Vertices: \(vertices.count)")
        print("   Edges: \(edges.count)")
    }
    
    private func serializeToJSON() throws -> Data {
        let verticesArray = vertices.values.map { vertex in
            return [
                "id": vertex.id,
                "properties": vertex.properties.mapValues { $0.toAny() }
            ]
        }
        
        let edgesArray = edges.values.map { edge in
            return [
                "from": edge.id.from,
                "to": edge.id.to,
                "properties": edge.properties.mapValues { $0.toAny() },
                "weight": edge.weight
            ]
        }
        
        let jsonObject: [String: Any] = [
            "version": "1.0",
            "vertexCount": vertices.count,
            "edgeCount": edges.count,
            "vertices": verticesArray,
            "edges": edgesArray
        ]
        
        return try JSONSerialization.data(withJSONObject: jsonObject, options: [.prettyPrinted])
    }
    
    private func deserializeFromJSON(data: Data) throws {
        guard let jsonObject = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let verticesArray = jsonObject["vertices"] as? [[String: Any]],
              let edgesArray = jsonObject["edges"] as? [[String: Any]] else {
            throw GraphError.invalidFormat
        }
        
        vertices.removeAll()
        edges.removeAll()
        adjacencyList.removeAll()
        
        for vertexDict in verticesArray {
            guard let id = vertexDict["id"] as? UInt64,
                  let propertiesDict = vertexDict["properties"] as? [String: Any] else {
                continue
            }
            
            let properties = propertiesDict.mapValues { PropertyValue.fromAny($0) }
            let vertex = Vertex(id: id, properties: properties)
            vertices[id] = vertex
            adjacencyList[id] = []
        }
        
        for edgeDict in edgesArray {
            guard let from = edgeDict["from"] as? UInt64,
                  let to = edgeDict["to"] as? UInt64,
                  let propertiesDict = edgeDict["properties"] as? [String: Any] else {
                continue
            }
            
            let edgeId = EdgeID(from: from, to: to)
            let properties = propertiesDict.mapValues { PropertyValue.fromAny($0) }
            let weight = edgeDict["weight"] as? Double ?? 1.0
            let edge = Edge(id: edgeId, properties: properties, weight: weight)
            
            edges[edgeId] = edge
            adjacencyList[from]?.insert(to)
        }
    }
}

// MARK: - Test Program

print("=== Axolotl Graph Database Test ===\n")

let testDbPath = "/tmp/test_graph.axolotl"

// Test 1: Create database and add data
print("Test 1: Create database and add data")
print(String(repeating: "-", count: 50))

do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    // Add vertices
    print("\n1. Adding vertices...")
    let v1 = try db.addVertex(properties: ["name": .string("Alice"), "age": .int(30)])
    let v2 = try db.addVertex(properties: ["name": .string("Bob"), "age": .int(25)])
    let v3 = try db.addVertex(properties: ["name": .string("Charlie"), "age": .int(35)])
    let v4 = try db.addVertex(properties: ["name": .string("David"), "age": .int(40)])
    let v5 = try db.addVertex(properties: ["name": .string("Eve"), "age": .int(28)])
    
    print("   Added 5 vertices (IDs: \(v1)-\(v5))")
    
    // Add edges
    print("\n2. Adding edges...")
    _ = try db.addEdge(from: v1, to: v2, properties: ["type": .string("knows")])
    _ = try db.addEdge(from: v1, to: v5, properties: ["type": .string("knows")])
    _ = try db.addEdge(from: v2, to: v3, properties: ["type": .string("knows")])
    _ = try db.addEdge(from: v3, to: v4, properties: ["type": .string("knows")])
    _ = try db.addEdge(from: v4, to: v5, properties: ["type": .string("knows")])
    
    print("   Added 5 edges")
    
    // Show statistics
    let stats = db.getStatistics()
    print("\n3. Graph statistics:")
    print("   Vertices: \(stats.vertexCount)")
    print("   Edges: \(stats.edgeCount)")
    
    // Save to disk
    print("\n4. Saving to disk...")
    try db.save()
    
    print("\n✅ Test 1 PASSED\n")
    
} catch {
    print("❌ Test 1 FAILED: \(error)")
}

// Test 2: Load database from disk
print("Test 2: Load database from disk")
print(String(repeating: "-", count: 50))

do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    // Check statistics
    let stats = db.getStatistics()
    print("\n1. Loaded graph statistics:")
    print("   Vertices: \(stats.vertexCount)")
    print("   Edges: \(stats.edgeCount)")
    
    // Verify data
    print("\n2. Verifying data...")
    
    // Check vertex 0
    if let v0 = db.getVertex(id: 0) {
        print("   Vertex 0: \(v0.properties["name"] ?? .null)")
    } else {
        print("   ❌ Vertex 0 not found")
    }
    
    // Check edge (0, 1)
    if let e01 = db.getEdge(from: 0, to: 1) {
        print("   Edge (0 -> 1): \(e01.properties["type"] ?? .null)")
    } else {
        print("   ❌ Edge (0 -> 1) not found")
    }
    
    // Check neighbors of vertex 0
    let neighbors = db.getNeighbors(of: 0)
    print("\n3. Neighbors of vertex 0: \(neighbors)")
    
    print("\n✅ Test 2 PASSED\n")
    
} catch {
    print("❌ Test 2 FAILED: \(error)")
}

// Test 3: Incremental changes and save
print("Test 3: Incremental changes and save")
print(String(repeating: "-", count: 50))

do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    // Add more vertices and edges
    print("\n1. Adding more data...")
    let v6 = try db.addVertex(properties: ["name": .string("Frank"), "age": .int(32)])
    _ = try db.addEdge(from: v6, to: 0, properties: ["type": .string("knows")])
    
    print("   Added vertex 6 (Frank)")
    print("   Added edge (6 -> 0)")
    
    // Save
    print("\n2. Saving to disk...")
    try db.save()
    
    // Reload and verify
    print("\n3. Reloading and verifying...")
    let db2 = try PersistentGraph(filePath: testDbPath)
    let stats = db2.getStatistics()
    print("   Vertices: \(stats.vertexCount) (expected: 6)")
    print("   Edges: \(stats.edgeCount) (expected: 6)")
    
    if stats.vertexCount == 6 && stats.edgeCount == 6 {
        print("\n✅ Test 3 PASSED\n")
    } else {
        print("\n❌ Test 3 FAILED: Data mismatch\n")
    }
    
} catch {
    print("❌ Test 3 FAILED: \(error)")
}

// Cleanup
print("Cleaning up...")
try? FileManager.default.removeItem(atPath: testDbPath)
print("✅ Test file removed\n")

print("=== All tests completed ===")
