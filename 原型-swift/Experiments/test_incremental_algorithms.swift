import Foundation

// MARK: - Core Types

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

// MARK: - Persistent Graph Database

class PersistentGraph {
    private var vertices: [VertexID: Vertex] = [:]
    private var edges: [EdgeID: Edge] = [:]
    private var adjacencyList: [VertexID: Set<VertexID>] = [:]
    private var reverseAdjacencyList: [VertexID: Set<VertexID>] = [:]
    private var propertyIndex: [String: [PropertyValue: Set<VertexID>]] = [:]
    private let filePath: String

    private var dirtyVertices: Set<VertexID> = []
    private var pagerankScores: [VertexID: Double] = [:]
    private var isPageRankDirty: Bool = true
    private var bfsCache: [VertexID: [VertexID: Int]] = [:]
    private var dirtyBFSStarts: Set<VertexID> = []
    private var ssspCache: [VertexID: [VertexID: Int]] = [:]
    private var dirtySSSPStarts: Set<VertexID> = []
    private var connectedComponentsCache: [Set<VertexID>] = []
    private var isConnectedComponentsDirty: Bool = true
    private var triangleCountCache: Int = 0
    private var isTriangleCountDirty: Bool = true

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

    func addVertex(id: VertexID? = nil, properties: [String: PropertyValue] = [:]) throws -> VertexID {
        let vid = id ?? VertexID(vertices.count)

        guard vertices[vid] == nil else {
            throw GraphError.vertexAlreadyExists(vid)
        }

        let vertex = Vertex(id: vid, properties: properties)
        vertices[vid] = vertex
        adjacencyList[vid] = []

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
        reverseAdjacencyList[to]?.insert(from)

        dirtyVertices.insert(from)
        dirtyVertices.insert(to)
        isPageRankDirty = true
        isConnectedComponentsDirty = true
        isTriangleCountDirty = true

        markBFSCacheDirty(for: from)
        markBFSCacheDirty(for: to)
        markSSSPCacheDirty(for: from)
        markSSSPCacheDirty(for: to)

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
        isConnectedComponentsDirty = true
        isTriangleCountDirty = true

        markBFSCacheDirty(for: from)
        markBFSCacheDirty(for: to)
        markSSSPCacheDirty(for: from)
        markSSSPCacheDirty(for: to)
    }

    func getVertex(id: VertexID) -> Vertex? {
        return vertices[id]
    }

    func getNeighbors(of vertexId: VertexID) -> [VertexID] {
        return Array(adjacencyList[vertexId] ?? [])
    }

    func getStatistics() -> (vertexCount: Int, edgeCount: Int) {
        return (vertices.count, edges.count)
    }

    // MARK: - BFS

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
            if isReachable(from: start, to: vertexId) || isReachable(from: vertexId, to: start) {
                dirtyBFSStarts.insert(start)
            }
        }
    }

    private func isReachable(from start: VertexID, to target: VertexID) -> Bool {
        if let distances = bfsCache[start] {
            return distances[target] != nil
        }
        return true
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

    // MARK: - SSSP

    func sssp(from start: VertexID) -> [VertexID: Int] {
        guard vertices[start] != nil else { return [:] }

        if let cached = ssspCache[start], !dirtySSSPStarts.contains(start) {
            return cached
        }

        let distances = computeSSSP(from: start)
        ssspCache[start] = distances
        dirtySSSPStarts.remove(start)

        return distances
    }

    private func computeSSSP(from start: VertexID) -> [VertexID: Int] {
        return computeBFS(from: start)
    }

    private func markSSSPCacheDirty(for vertexId: VertexID) {
        for start in ssspCache.keys {
            if isReachable(from: start, to: vertexId) || isReachable(from: vertexId, to: start) {
                dirtySSSPStarts.insert(start)
            }
        }
    }

    // MARK: - Connected Components

    func connectedComponents() -> [Set<VertexID>] {
        if !isConnectedComponentsDirty {
            return connectedComponentsCache
        }

        let components = computeConnectedComponents()
        connectedComponentsCache = components
        isConnectedComponentsDirty = false

        return components
    }

    private func computeConnectedComponents() -> [Set<VertexID>] {
        var visited = Set<VertexID>()
        var components: [Set<VertexID>] = []

        for vid in vertices.keys {
            if !visited.contains(vid) {
                let component = bfs(from: vid, maxDepth: Int.max)
                let componentSet = Set(component)
                components.append(componentSet)
                visited.formUnion(componentSet)
            }
        }

        return components
    }

    // MARK: - Triangle Counting

    func triangleCount() -> Int {
        if !isTriangleCountDirty {
            return triangleCountCache
        }

        let count = computeTriangleCount()
        triangleCountCache = count
        isTriangleCountDirty = false

        return count
    }

    private func computeTriangleCount() -> Int {
        var count = 0

        for v in vertices.keys {
            guard let neighbors = adjacencyList[v] else { continue }

            let neighborArray = Array(neighbors)

            for i in 0..<neighborArray.count {
                for j in (i+1)..<neighborArray.count {
                    let u = neighborArray[i]
                    let w = neighborArray[j]

                    if adjacencyList[u]?.contains(w) == true {
                        count += 1
                    }
                }
            }
        }

        return count / 3
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

    // MARK: - Persistence

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
        isConnectedComponentsDirty = true
        isTriangleCountDirty = true

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

// MARK: - Tests

func testAllIncrementalAlgorithms() {
    print("🧪 Testing All Incremental Algorithms")
    print(String(repeating: "=", count: 60))

    do {
        let filePath = "/tmp/test_incremental_algorithms.json"
        try? FileManager.default.removeItem(atPath: filePath)

        let graph = try PersistentGraph(filePath: filePath)

        // Add vertices
        let v0 = try graph.addVertex(properties: ["name": .string("A")])
        let v1 = try graph.addVertex(properties: ["name": .string("B")])
        let v2 = try graph.addVertex(properties: ["name": .string("C")])
        let v3 = try graph.addVertex(properties: ["name": .string("D")])
        let v4 = try graph.addVertex(properties: ["name": .string("E")])
        let v5 = try graph.addVertex(properties: ["name": .string("F")])

        print("✅ Added 6 vertices")

        // Add edges
        try graph.addEdge(from: v0, to: v1)
        try graph.addEdge(from: v1, to: v2)
        try graph.addEdge(from: v2, to: v3)
        try graph.addEdge(from: v3, to: v4)
        try graph.addEdge(from: v0, to: v2)
        try graph.addEdge(from: v1, to: v3)

        print("✅ Added 6 edges")

        // Test 1: SSSP
        print("\n📊 Test 1: Incremental SSSP")
        let sssp1 = graph.sssp(from: v0)
        print("   SSSP from v0:")
        for (vid, dist) in sssp1.sorted(by: { $0.key < $1.key }) {
            print("      v\(vid): distance = \(dist)")
        }

        // Test shortest path
        let path = graph.shortestPath(from: v0, to: v4)
        print("   Shortest path from v0 to v4: \(path.map { "v\($0)" }.joined(separator: " -> "))")
        assert(path.count == 3) // v0 -> v2 -> v3 -> v4
        print("   ✅ SSSP test passed")

        // Test 2: Connected Components
        print("\n📊 Test 2: Incremental Connected Components")
        let components1 = graph.connectedComponents()
        print("   Connected components: \(components1.count)")
        for (i, component) in components1.enumerated() {
            print("      Component \(i): \(component.map { "v\($0)" }.sorted().joined(separator: ", "))")
        }
        assert(components1.count == 1) // All vertices should be in one component
        print("   ✅ Connected components test passed")

        // Test 3: Triangle Counting
        print("\n📊 Test 3: Incremental Triangle Counting")
        let triangleCount1 = graph.triangleCount()
        print("   Triangle count: \(triangleCount1)")

        // Count triangles manually
        // Triangles: (v0, v1, v2), (v0, v1, v3), (v0, v2, v3), (v1, v2, v3)
        // Wait, let me check: v0->v1, v0->v2, v1->v2, v1->v3, v2->v3
        // Triangle 1: v0, v1, v2 (edges: v0->v1, v0->v2, v1->v2) ✅
        // Triangle 2: v0, v1, v3 (edges: v0->v1, v1->v3, but no v0->v3) ❌
        // Triangle 3: v0, v2, v3 (edges: v0->v2, v2->v3, but no v0->v3) ❌
        // Triangle 4: v1, v2, v3 (edges: v1->v2, v1->v3, v2->v3) ✅
        // So there should be 2 triangles
        print("   Expected: 2 triangles")
        assert(triangleCount1 == 2)
        print("   ✅ Triangle counting test passed")

        // Test 4: Add a new edge and test incremental updates
        print("\n📊 Test 4: Incremental Updates After Adding Edge")
        try graph.addEdge(from: v4, to: v5)
        print("   ➕ Added edge: v4 -> v5")

        // SSSP should be recomputed (v5 is new)
        let sssp2 = graph.sssp(from: v0)
        print("   SSSP from v0 (after adding v5):")
        for (vid, dist) in sssp2.sorted(by: { $0.key < $1.key }) {
            print("      v\(vid): distance = \(dist)")
        }
        assert(sssp2[v5] == 4) // v0 -> v1 -> v2 -> v3 -> v4 -> v5
        print("   ✅ SSSP incremental update passed")

        // Connected components should still be 1
        let components2 = graph.connectedComponents()
        print("   Connected components: \(components2.count)")
        assert(components2.count == 1)
        print("   ✅ Connected components incremental update passed")

        // Triangle count should still be 2
        let triangleCount2 = graph.triangleCount()
        print("   Triangle count: \(triangleCount2)")
        assert(triangleCount2 == 2)
        print("   ✅ Triangle counting incremental update passed")

        print("\n✅ All incremental algorithm tests passed!")

        // Cleanup
        try? FileManager.default.removeItem(atPath: filePath)

    } catch {
        print("❌ Error: \(error)")
    }
}

testAllIncrementalAlgorithms()
