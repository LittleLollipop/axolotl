import Foundation
import QuartzCore

// MARK: - Core Types (Must match PersistentGraph.swift)

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

// MARK: - Persistent Graph Database (with Incremental PageRank)

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
    
    // Initialization
    init(filePath: String) throws {
        self.filePath = filePath
        
        let dir = (filePath as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        
        if FileManager.default.fileExists(atPath: filePath) {
            try load()
        }
        
        // Initialize reverse adjacency list
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
        
        // Remove all connected edges
        let neighbors = adjacencyList[id] ?? []
        for neighbor in neighbors {
            edges.removeValue(forKey: EdgeID(from: id, to: neighbor))
            reverseAdjacencyList[neighbor]?.remove(id)
        }
        
        // Remove incoming edges
        for (vid, _) in vertices {
            edges.removeValue(forKey: EdgeID(from: vid, to: id))
            adjacencyList[vid]?.remove(id)
        }
        
        vertices.removeValue(forKey: id)
        adjacencyList.removeValue(forKey: id)
        reverseAdjacencyList.removeValue(forKey: id)
        
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
    }
    
    // MARK: - Query Operations
    
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
        
        return []
    }
    
    // MARK: - PageRank (Full + Incremental)
    
    func pageRank(dampingFactor: Double = 0.85, maxIterations: Int = 100, tolerance: Double = 1e-6) -> [VertexID: Double] {
        return computeFullPageRank(dampingFactor: dampingFactor, maxIterations: maxIterations, tolerance: tolerance)
    }
    
    func incrementalPageRank(dampingFactor: Double = 0.85, maxIterations: Int = 100, tolerance: Double = 1e-6) -> [VertexID: Double] {
        let n = vertices.count
        guard n > 0 else { return [:] }
        
        if !isPageRankDirty {
            return pagerankScores
        }
        
        if dirtyVertices.isEmpty {
            // No changes, compute full PageRank
            let scores = computeFullPageRank(dampingFactor: dampingFactor, maxIterations: maxIterations, tolerance: tolerance)
            pagerankScores = scores
            isPageRankDirty = false
            return scores
        }
        
        // Incremental update
        let scores = computeIncrementalPageRank(dampingFactor: dampingFactor, maxIterations: maxIterations, tolerance: tolerance)
        pagerankScores = scores
        isPageRankDirty = false
        dirtyVertices.removeAll()
        
        return scores
    }
    
    private func computeFullPageRank(dampingFactor: Double = 0.85, maxIterations: Int = 100, tolerance: Double = 1e-6) -> [VertexID: Double] {
        let n = vertices.count
        guard n > 0 else { return [:] }
        
        var scores = [VertexID: Double]()
        let initialScore = 1.0 / Double(n)
        for vid in vertices.keys {
            scores[vid] = initialScore
        }
        
        var outDegrees = [VertexID: Double]()
        for (vid, _) in vertices {
            outDegrees[vid] = Double(adjacencyList[vid]?.count ?? 0)
        }
        
        for _ in 0..<maxIterations {
            var newScores = [VertexID: Double]()
            let damping = (1.0 - dampingFactor) / Double(n)
            
            for vid in vertices.keys {
                newScores[vid] = damping
            }
            
            for (vid, score) in scores {
                let outDegree = outDegrees[vid] ?? 0
                
                if outDegree > 0 {
                    let contribution = score * dampingFactor / outDegree
                    
                    for neighbor in adjacencyList[vid] ?? [] {
                        newScores[neighbor, default: 0] += contribution
                    }
                } else {
                    let contribution = score * dampingFactor / Double(n)
                    for (vid, _) in vertices {
                        newScores[vid, default: 0] += contribution
                    }
                }
            }
            
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
    
    private func computeIncrementalPageRank(dampingFactor: Double = 0.85, maxIterations: Int = 100, tolerance: Double = 1e-6) -> [VertexID: Double] {
        let n = vertices.count
        guard n > 0 else { return [:] }
        
        var scores = pagerankScores
        if scores.isEmpty {
            let initialScore = 1.0 / Double(n)
            for vid in vertices.keys {
                scores[vid] = initialScore
            }
        }
        
        var outDegrees = [VertexID: Double]()
        for (vid, _) in vertices {
            outDegrees[vid] = Double(adjacencyList[vid]?.count ?? 0)
        }
        
        let affectedVertices = dirtyVertices.union(
            dirtyVertices.flatMap { vid in
                return Array(reverseAdjacencyList[vid] ?? [])
            }
        )
        
        for _ in 0..<maxIterations {
            var newScores = scores
            
            for vid in affectedVertices {
                let damping = (1.0 - dampingFactor) / Double(n)
                var newScore = damping
                
                for neighbor in reverseAdjacencyList[vid] ?? [] {
                    let neighborScore = scores[neighbor] ?? 0
                    let neighborOutDegree = outDegrees[neighbor] ?? 0
                    
                    if neighborOutDegree > 0 {
                        newScore += neighborScore * dampingFactor / neighborOutDegree
                    } else {
                        newScore += neighborScore * dampingFactor / Double(n)
                    }
                }
                
                newScores[vid] = newScore
            }
            
            var maxChange = 0.0
            for vid in affectedVertices {
                let oldScore = scores[vid] ?? 0
                let newScore = newScores[vid] ?? 0
                maxChange = max(maxChange, abs(newScore - oldScore))
            }
            
            scores = newScores
            
            if maxChange < tolerance {
                break
            }
        }
        
        return scores
    }
    
    // MARK: - Property Index
    
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
    
    func findByProperty(_ propertyName: String, value: PropertyValue) -> [VertexID] {
        return Array(propertyIndex[propertyName]?[value] ?? [])
    }
    
    func getPropertyValues(_ propertyName: String) -> [PropertyValue] {
        guard let index = propertyIndex[propertyName] else {
            return []
        }
        return Array(index.keys)
    }
    
    func updateVertex(id: VertexID, properties: [String: PropertyValue]) throws {
        guard var vertex = vertices[id] else {
            throw GraphError.vertexNotFound(id)
        }
        
        updatePropertyIndex(for: id, properties: vertex.properties, isDelete: true)
        
        vertex.properties = properties
        vertices[id] = vertex
        
        updatePropertyIndex(for: id, properties: properties, isDelete: false)
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

print("=== Axolotl Incremental PageRank Test ===\n")

let testDbPath = "/tmp/test_incremental_pagerank.axolotl"

// Clean up
try? FileManager.default.removeItem(atPath: testDbPath)

do {
    // Test 1: Create database and add vertices/edges
    print("Test 1: Create database and add data")
    print(String(repeating: "-", count: 50))
    
    let db = try PersistentGraph(filePath: testDbPath)
    
    // Add vertices
    let v0 = try db.addVertex(properties: ["name": .string("Alice")])
    let v1 = try db.addVertex(properties: ["name": .string("Bob")])
    let v2 = try db.addVertex(properties: ["name": .string("Charlie")])
    let v3 = try db.addVertex(properties: ["name": .string("David")])
    let v4 = try db.addVertex(properties: ["name": .string("Eve")])
    
    // Add edges (directed graph)
    _ = try db.addEdge(from: v0, to: v1)
    _ = try db.addEdge(from: v0, to: v4)
    _ = try db.addEdge(from: v1, to: v2)
    _ = try db.addEdge(from: v1, to: v4)
    _ = try db.addEdge(from: v2, to: v3)
    _ = try db.addEdge(from: v3, to: v4)
    
    print("   Added 5 vertices and 6 edges")
    
    // Test 2: Compute full PageRank
    print("\nTest 2: Compute full PageRank")
    print(String(repeating: "-", count: 50))
    
    let fullStart = CACurrentMediaTime()
    let fullScores = db.pageRank()
    let fullTime = CACurrentMediaTime() - fullStart
    
    print("   Full PageRank time: \(String(format: "%.4f", fullTime * 1000)) ms")
    print("   Scores (top 3):")
    let sortedFull = fullScores.sorted { $0.value > $1.value }.prefix(3)
    for (vid, score) in sortedFull {
        print("     Vertex \(vid): \(String(format: "%.6f", score))")
    }
    
    // Test 3: Add new edges (incremental update)
    print("\nTest 3: Add new edges (incremental update)")
    print(String(repeating: "-", count: 50))
    
    _ = try db.addEdge(from: v4, to: v0) // New edge
    _ = try db.addEdge(from: v2, to: v0) // New edge
    
    print("   Added 2 new edges")
    print("   Dirty vertices: \(db)") // This won't work, just for illustration
    
    // Test 4: Compute incremental PageRank
    print("\nTest 4: Compute incremental PageRank")
    print(String(repeating: "-", count: 50))
    
    let incStart = CACurrentMediaTime()
    let incScores = db.incrementalPageRank()
    let incTime = CACurrentMediaTime() - incStart
    
    print("   Incremental PageRank time: \(String(format: "%.4f", incTime * 1000)) ms")
    print("   Scores (top 3):")
    let sortedInc = incScores.sorted { $0.value > $1.value }.prefix(3)
    for (vid, score) in sortedInc {
        print("     Vertex \(vid): \(String(format: "%.6f", score))")
    }
    
    // Test 5: Verify correctness (compare with full recomputation)
    print("\nTest 5: Verify correctness")
    print(String(repeating: "-", count: 50))
    
    let verifyScores = db.pageRank() // Force full recomputation
    
    var maxError: Double = 0.0
    for (vid, score) in incScores {
        let verifyScore = verifyScores[vid] ?? 0
        let error = abs(score - verifyScore)
        maxError = max(maxError, error)
    }
    
    print("   Max error: \(String(format: "%.10f", maxError))")
    
    if maxError < 1e-6 {
        print("   ✅ Incremental PageRank is correct!")
    } else {
        print("   ❌ Incremental PageRank is incorrect (max error = \(maxError))")
    }
    
    // Test 6: Performance comparison
    print("\nTest 6: Performance comparison")
    print(String(repeating: "-", count: 50))
    
    let speedup = fullTime / incTime
    print("   Full recomputation: \(String(format: "%.4f", fullTime * 1000)) ms")
    print("   Incremental update: \(String(format: "%.4f", incTime * 1000)) ms")
    print("   Speedup: \(String(format: "%.2f", speedup))x")
    
    if speedup > 1.0 {
        print("   ✅ Incremental update is faster!")
    } else {
        print("   ⚠️  Incremental update is slower (expected for small graphs)")
    }
    
} catch {
    print("❌ Test FAILED: \(error)")
}

// Clean up
print("\nCleaning up...")
try? FileManager.default.removeItem(atPath: testDbPath)
print("✅ Test file removed")

print("\n=== Test completed ===")
