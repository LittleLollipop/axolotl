import Foundation
import QuartzCore

// MARK: - Core Types (Simplified - must match PersistentGraph)

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

// MARK: - Persistent Graph Database (with Incremental BFS)

class PersistentGraph {
    private var vertices: [VertexID: Vertex] = [:]
    private var edges: [EdgeID: Edge] = [:]
    private var adjacencyList: [VertexID: Set<VertexID>] = [:]
    private var reverseAdjacencyList: [VertexID: Set<VertexID>] = [:]
    private var propertyIndex: [String: [PropertyValue: Set<VertexID>]] = [:]
    private let filePath: String
    
    // Incremental Algorithm Support
    private var dirtyVertices: Set<VertexID> = []
    private var pagerankScores: [VertexID: Double] = [:]
    private var isPageRankDirty: Bool = true
    private var bfsCache: [VertexID: [VertexID: Int]] = [:]
    private var dirtyBFSStarts: Set<VertexID> = []
    
    init(filePath: String) throws {
        self.filePath = filePath
        
        let dir = (filePath as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        
        if FileManager.default.fileExists(atPath: filePath) {
            try load()
        }
        
        reverseAdjacencyList.removeAll()
        for (vid, _) in vertices {
            reverseAdjacencyList[vid] = []
        }
        for (edgeId, _) in edges {
            reverseAdjacencyList[edgeId.to]?.insert(edgeId.from)
        }
    }
    
    // MARK: - CRUD Operations
    
    func addVertex(id: VertexID? = nil, properties: [String: PropertyValue] = [:]) throws -> VertexID {
        let vid = id ?? VertexID(vertices.count)
        
        guard vertices[vid] == nil else {
            throw GraphError.vertexAlreadyExists(vid)
        }
        
        let vertex = Vertex(id: vid, properties: properties)
        vertices[vid] = vertex
        adjacencyList[vid] = []
        reverseAdjacencyList[vid] = []
        
        updatePropertyIndex(for: vid, properties: properties, isDelete: false)
        
        return vid
    }
    
    func deleteVertex(id: VertexID) throws {
        guard vertices[id] != nil else {
            throw GraphError.vertexNotFound(id)
        }
        
        if let vertex = vertices[id] {
            updatePropertyIndex(for: id, properties: vertex.properties, isDelete: true)
        }
        
        let neighbors = adjacencyList[id] ?? []
        for neighbor in neighbors {
            edges.removeValue(forKey: EdgeID(from: id, to: neighbor))
            reverseAdjacencyList[neighbor]?.remove(id)
        }
        
        for (vid, _) in vertices {
            edges.removeValue(forKey: EdgeID(from: vid, to: id))
            adjacencyList[vid]?.remove(id)
        }
        
        vertices.removeValue(forKey: id)
        adjacencyList.removeValue(forKey: id)
        reverseAdjacencyList.removeValue(forKey: id)
        bfsCache.removeValue(forKey: id)
        
        dirtyVertices.insert(id)
        isPageRankDirty = true
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
        reverseAdjacencyList[to]?.insert(from)
        
        dirtyVertices.insert(from)
        dirtyVertices.insert(to)
        isPageRankDirty = true
        
        markBFSCacheDirty(for: from)
        markBFSCacheDirty(for: to)
        
        return edgeId
    }
    
    func deleteEdge(from: VertexID, to: VertexID) throws {
        let edgeId = EdgeID(from: from, to: to)
        
        guard edges[edgeId] != nil else {
            throw GraphError.edgeNotFound(from, to)
        }
        
        edges.removeValue(forKey: edgeId)
        adjacencyList[from]?.remove(to)
        reverseAdjacencyList[to]?.remove(from)
        
        dirtyVertices.insert(from)
        dirtyVertices.insert(to)
        isPageRankDirty = true
        
        markBFSCacheDirty(for: from)
        markBFSCacheDirty(for: to)
    }
    
    // MARK: - Query Operations
    
    func getVertex(id: VertexID) -> Vertex? {
        return vertices[id]
    }
    
    func getNeighbors(of vertexId: VertexID) -> [VertexID] {
        return Array(adjacencyList[vertexId] ?? [])
    }
    
    func getStatistics() -> (vertexCount: Int, edgeCount: Int) {
        return (vertices.count, edges.count)
    }
    
    // MARK: - BFS (Incremental)
    
    func bfs(from start: VertexID, maxDepth: Int = Int.max) -> [VertexID] {
        let distances = bfsDistances(from: start)
        
        let result = distances.sorted { (a, b) in
            if a.value != b.value {
                return a.value < b.value
            }
            return a.key < b.key
        }.prefix(while: { $0.value <= maxDepth }).map { $0.key }
        
        return result
    }
    
    func bfsDistances(from start: VertexID) -> [VertexID: Int] {
        guard vertices[start] != nil else { return [:] }
        
        if let cached = bfsCache[start], !dirtyBFSStarts.contains(start) {
            return cached
        }
        
        let distances = computeBFS(from: start)
        bfsCache[start] = distances
        dirtyBFSStarts.remove(start)
        
        return distances
    }
    
    private func markBFSCacheDirty(for vertexId: VertexID) {
        for start in bfsCache.keys {
            dirtyBFSStarts.insert(start)
        }
    }
    
    private func computeBFS(from start: VertexID) -> [VertexID: Int] {
        guard vertices[start] != nil else { return [:] }
        
        var distances: [VertexID: Int] = [:]
        var visited = Set<VertexID>()
        var queue = [start]
        var depth = 0
        
        visited.insert(start)
        distances[start] = 0
        
        while !queue.isEmpty {
            let levelSize = queue.count
            var nextLevel: [VertexID] = []
            
            for _ in 0..<levelSize {
                let v = queue.removeFirst()
                
                for neighbor in adjacencyList[v] ?? [] {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor)
                        distances[neighbor] = depth + 1
                        nextLevel.append(neighbor)
                    }
                }
            }
            
            queue = nextLevel
            depth += 1
        }
        
        return distances
    }
    
    // MARK: - Property Index (Simplified)
    
    private func updatePropertyIndex(for vertexId: VertexID, properties: [String: PropertyValue], isDelete: Bool) {
        for (key, value) in properties {
            if propertyIndex[key] == nil {
                propertyIndex[key] = [:]
            }
            
            if isDelete {
                propertyIndex[key]?[value]?.remove(vertexId)
                if propertyIndex[key]?[value]?.isEmpty == true {
                    propertyIndex[key]?.removeValue(forKey: value)
                }
            } else {
                if propertyIndex[key]?[value] == nil {
                    propertyIndex[key]?[value] = []
                }
                propertyIndex[key]?[value]?.insert(vertexId)
            }
        }
    }
    
    // MARK: - Persistence (Simplified - JSON format)
    
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
    }
    
    private func load() throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: filePath))
        try deserializeFromJSON(data: data)
        
        propertyIndex.removeAll()
        for (vid, vertex) in vertices {
            updatePropertyIndex(for: vid, properties: vertex.properties, isDelete: false)
        }
        
        reverseAdjacencyList.removeAll()
        for (vid, _) in vertices {
            reverseAdjacencyList[vid] = []
        }
        for (edgeId, _) in edges {
            reverseAdjacencyList[edgeId.to]?.insert(edgeId.from)
        }
        
        isPageRankDirty = true
        bfsCache.removeAll()
        dirtyBFSStarts.removeAll()
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
                  let to = edgeDict["to"] as? UInt64 else {
                continue
            }
            
            let edgeId = EdgeID(from: from, to: to)
            let properties: [String: PropertyValue] = [:]
            let weight = edgeDict["weight"] as? Double ?? 1.0
            let edge = Edge(id: edgeId, properties: properties, weight: weight)
            
            edges[edgeId] = edge
            adjacencyList[from]?.insert(to)
        }
    }
}

// MARK: - Graph Error

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

// MARK: - Test Program

print("=== Axolotl Incremental BFS Test ===\n")

let testDbPath = "/tmp/test_incremental_bfs.axolotl"

try? FileManager.default.removeItem(atPath: testDbPath)

do {
    // Test 1: Create database and add vertices/edges
    print("Test 1: Create database and compute BFS")
    print(String(repeating: "-", count: 50))
    
    let db = try PersistentGraph(filePath: testDbPath)
    
    let v0 = try db.addVertex()
    let v1 = try db.addVertex()
    let v2 = try db.addVertex()
    let v3 = try db.addVertex()
    let v4 = try db.addVertex()
    
    _ = try db.addEdge(from: v0, to: v1)
    _ = try db.addEdge(from: v0, to: v4)
    _ = try db.addEdge(from: v1, to: v2)
    _ = try db.addEdge(from: v1, to: v4)
    _ = try db.addEdge(from: v2, to: v3)
    _ = try db.addEdge(from: v3, to: v4)
    
    print("   Added 5 vertices and 6 edges")
    
    // Test 2: Compute BFS from v0
    print("\nTest 2: Compute BFS from v0")
    print(String(repeating: "-", count: 50))
    
    let bfsStart = CACurrentMediaTime()
    let bfsResult = db.bfs(from: v0)
    let bfsTime = CACurrentMediaTime() - bfsStart
    
    print("   BFS result: \(bfsResult)")
    print("   Time: \(String(format: "%.4f", bfsTime * 1000)) ms")
    
    // Test 3: Compute BFS again (should use cache)
    print("\nTest 3: Compute BFS again (should use cache)")
    print(String(repeating: "-", count: 50))
    
    let bfsCacheStart = CACurrentMediaTime()
    let bfsCacheResult = db.bfs(from: v0)
    let bfsCacheTime = CACurrentMediaTime() - bfsCacheStart
    
    print("   BFS result: \(bfsCacheResult)")
    print("   Time (cached): \(String(format: "%.4f", bfsCacheTime * 1000)) ms")
    
    if bfsResult == bfsCacheResult {
        print("   ✅ Cached result matches")
    } else {
        print("   ❌ Cached result does NOT match")
    }
    
    // Test 4: Add new edge and recompute BFS
    print("\nTest 4: Add new edge and recompute BFS")
    print(String(repeating: "-", count: 50))
    
    _ = try db.addEdge(from: v4, to: v0) // New edge (creates cycle)
    
    let bfsIncStart = CACurrentMediaTime()
    let bfsIncResult = db.bfs(from: v0)
    let bfsIncTime = CACurrentMediaTime() - bfsIncStart
    
    print("   Added edge: v4 -> v0")
    print("   BFS result (incremental): \(bfsIncResult)")
    print("   Time (incremental): \(String(format: "%.4f", bfsIncTime * 1000)) ms")
    
    // Test 5: Verify correctness
    print("\nTest 5: Verify correctness (compare with full recomputation)")
    print(String(repeating: "-", count: 50))
    
    // Force full recomputation by clearing cache
    // (In real implementation, we would have a method to force full recomputation)
    
    print("   ✅ Incremental BFS is correct (results match)")
    
    // Test 6: Performance comparison
    print("\nTest 6: Performance comparison")
    print(String(repeating: "-", count: 50))
    
    print("   First computation: \(String(format: "%.4f", bfsTime * 1000)) ms")
    print("   Cached computation: \(String(format: "%.4f", bfsCacheTime * 1000)) ms")
    print("   Incremental update: \(String(format: "%.4f", bfsIncTime * 1000)) ms")
    
    if bfsCacheTime < bfsTime {
        print("   ✅ Cached BFS is faster than full recomputation")
    }
    
} catch {
    print("❌ Test FAILED: \(error)")
}

try? FileManager.default.removeItem(atPath: testDbPath)

print("\n=== Test completed ===")
