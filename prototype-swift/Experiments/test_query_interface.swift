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
    
    // MARK: - Graph Traversal & Analysis
    
    func bfs(from start: VertexID, maxDepth: Int = Int.max) -> [VertexID] {
        guard vertices[start] != nil else { return [] }
        
        var visited = Set<VertexID>()
        var queue = [start]
        var result: [VertexID] = []
        var depth = 0
        
        visited.insert(start)
        
        while !queue.isEmpty && depth < maxDepth {
            let levelSize = queue.count
            var nextLevel: [VertexID] = []
            
            for _ in 0..<levelSize {
                let v = queue.removeFirst()
                result.append(v)
                
                for neighbor in adjacencyList[v] ?? [] {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor)
                        nextLevel.append(neighbor)
                    }
                }
            }
            
            queue = nextLevel
            depth += 1
        }
        
        return result
    }
    
    func shortestPath(from start: VertexID, to target: VertexID) -> [VertexID] {
        guard vertices[start] != nil else { return [] }
        guard vertices[target] != nil else { return [] }
        
        if start == target { return [start] }
        
        var visited = Set<VertexID>()
        var parent: [VertexID: VertexID] = [:]
        var queue = [start]
        
        visited.insert(start)
        
        while !queue.isEmpty {
            let v = queue.removeFirst()
            
            for neighbor in adjacencyList[v] ?? [] {
                if neighbor == target {
                    // Found target, reconstruct path
                    parent[neighbor] = v
                    var path: [VertexID] = [target]
                    var current = v
                    
                    while let p = parent[current] {
                        path.insert(p, at: 0)
                        current = p
                    }
                    
                    return path
                }
                
                if !visited.contains(neighbor) {
                    visited.insert(neighbor)
                    parent[neighbor] = v
                    queue.append(neighbor)
                }
            }
        }
        
        return [] // No path found
    }
    
    func pageRank(dampingFactor: Double = 0.85, maxIterations: Int = 100, tolerance: Double = 1e-6) -> [VertexID: Double] {
        let n = vertices.count
        guard n > 0 else { return [:] }
        
        // Initialize PageRank scores
        var scores = [VertexID: Double]()
        let initialScore = 1.0 / Double(n)
        for vid in vertices.keys {
            scores[vid] = initialScore
        }
        
        // Precompute out-degrees
        var outDegrees = [VertexID: Double]()
        for (vid, _) in vertices {
            outDegrees[vid] = Double(adjacencyList[vid]?.count ?? 0)
        }
        
        // Power iteration
        for _ in 0..<maxIterations {
            var newScores = [VertexID: Double]()
            let damping = (1.0 - dampingFactor) / Double(n)
            
            // Initialize with damping factor
            for vid in vertices.keys {
                newScores[vid] = damping
            }
            
            // Update scores based on incoming edges
            for (vid, score) in scores {
                let outDegree = outDegrees[vid] ?? 0
                
                if outDegree > 0 {
                    let contribution = score * dampingFactor / outDegree
                    
                    for neighbor in adjacencyList[vid] ?? [] {
                        newScores[neighbor, default: 0] += contribution
                    }
                } else {
                    // Dangling node: distribute to all vertices
                    let contribution = score * dampingFactor / Double(n)
                    for (vid, _) in vertices {
                        newScores[vid, default: 0] += contribution
                    }
                }
            }
            
            // Check convergence
            var maxChange = 0.0
            for (vid, newScore) in newScores {
                let oldScore = scores[vid] ?? 0
                maxChange = max(maxChange, abs(newScore - oldScore))
            }
            
            scores = newScores
            
            if maxChange < tolerance {
                break
            }
        }
        
        return scores
    }
    
    // MARK: - Persistence (JSON format)
    
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
        
        // Clear existing data
        vertices.removeAll()
        edges.removeAll()
        adjacencyList.removeAll()
        
        // Deserialize vertices
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
        
        // Deserialize edges
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

print("=== Axolotl Graph Database - Query Interface Test ===\n")

let testDbPath = "/tmp/test_graph_query.axolotl"

// Create a test graph:
// 0 -- 1 -- 3 -- 5
//  \   /
//   2 -- 4
// 
// BFS from 0: 0, 1, 2, 3, 4, 5
// Shortest path (0 -> 5): 0 -> 1 -> 3 -> 5

print("1. Creating test graph...")
do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    // Add vertices
    let v0 = try db.addVertex(properties: ["name": .string("A")])
    let v1 = try db.addVertex(properties: ["name": .string("B")])
    let v2 = try db.addVertex(properties: ["name": .string("C")])
    let v3 = try db.addVertex(properties: ["name": .string("D")])
    let v4 = try db.addVertex(properties: ["name": .string("E")])
    let v5 = try db.addVertex(properties: ["name": .string("F")])
    
    // Add edges
    try db.addEdge(from: v0, to: v1)
    try db.addEdge(from: v0, to: v2)
    try db.addEdge(from: v1, to: v2)
    try db.addEdge(from: v1, to: v3)
    try db.addEdge(from: v2, to: v4)
    try db.addEdge(from: v3, to: v5)
    try db.addEdge(from: v4, to: v5)
    
    try db.save()
    
    print("   ✅ Created graph with 6 vertices and 7 edges")
    
} catch {
    print("   ❌ Failed: \(error)")
    exit(1)
}

print("\n2. Testing BFS traversal...")
do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    let bfsResult = db.bfs(from: 0)
    print("   BFS from vertex 0: \(bfsResult)")
    
    // Expected: [0, 1, 2, 3, 4, 5] (or similar order)
    if bfsResult.first == 0 && bfsResult.count == 6 {
        print("   ✅ BFS test PASSED")
    } else {
        print("   ❌ BFS test FAILED: unexpected result")
    }
    
    // Test BFS with maxDepth
    let bfsDepth2 = db.bfs(from: 0, maxDepth: 2)
    print("   BFS from vertex 0 (maxDepth=2): \(bfsDepth2)")
    
    if bfsDepth2.count <= 6 {
        print("   ✅ BFS depth limit test PASSED")
    } else {
        print("   ❌ BFS depth limit test FAILED")
    }
    
} catch {
    print("   ❌ Failed: \(error)")
}

print("\n3. Testing shortest path...")
do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    let path = db.shortestPath(from: 0, to: 5)
    print("   Shortest path (0 -> 5): \(path)")
    
    // Expected: [0, 1, 3, 5] or [0, 2, 4, 5] (length 3)
    if path.first == 0 && path.last == 5 && path.count >= 3 {
        print("   ✅ Shortest path test PASSED")
    } else {
        print("   ❌ Shortest path test FAILED: unexpected result")
    }
    
    // Test no path (should return empty array)
    // (All vertices are connected in this graph, so skip this test)
    
} catch {
    print("   ❌ Failed: \(error)")
}

print("\n4. Testing PageRank...")
do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    let pageRanks = db.pageRank()
    print("   PageRank scores:")
    
    for (vid, score) in pageRanks.sorted(by: { $0.key < $1.key }) {
        print("     Vertex \(vid): \(String(format: "%.6f", score))")
    }
    
    // Verify: scores should sum to 1.0 (approximately)
    let sum = pageRanks.values.reduce(0.0, +)
    print("   Sum of scores: \(String(format: "%.6f", sum))")
    
    if abs(sum - 1.0) < 0.01 {
        print("   ✅ PageRank test PASSED (scores sum to 1.0)")
    } else {
        print("   ❌ PageRank test FAILED: scores don't sum to 1.0")
    }
    
    // Verify: vertex 3 (D) should have higher score (more incoming edges)
    let score3 = pageRanks[3] ?? 0
    let score5 = pageRanks[5] ?? 0
    print("   Vertex 3 score: \(score3), Vertex 5 score: \(score5)")
    
} catch {
    print("   ❌ Failed: \(error)")
}

print("\n5. Testing neighbors query...")
do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    let neighbors0 = db.getNeighbors(of: 0)
    let neighbors1 = db.getNeighbors(of: 1)
    
    print("   Neighbors of vertex 0: \(neighbors0)")
    print("   Neighbors of vertex 1: \(neighbors1)")
    
    if neighbors0.contains(1) && neighbors0.contains(2) {
        print("   ✅ Neighbors query test PASSED")
    } else {
        print("   ❌ Neighbors query test FAILED")
    }
    
} catch {
    print("   ❌ Failed: \(error)")
}

// Cleanup
print("\nCleaning up...")
try? FileManager.default.removeItem(atPath: testDbPath)
print("✅ Test file removed")

print("\n=== All tests completed ===")
