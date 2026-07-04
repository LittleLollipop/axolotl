import Foundation
import QuartzCore

// Performance test for incremental algorithms

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

        return edgeId
    }

    func getStatistics() -> (vertexCount: Int, edgeCount: Int) {
        return (vertices.count, edges.count)
    }

    // MARK: - BFS

    func bfs(from start: VertexID) -> [VertexID] {
        let distances = bfsDistances(from: start)

        let result = distances.sorted { (a, b) in
            if a.value != b.value {
                return a.value < b.value
            }
            return a.key < b.key
        }.map { $0.key }

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

    // MARK: - Connected Components

    func connectedComponents() -> [Set<VertexID>] {
        if !isConnectedComponentsDirty {
            return connectedComponentsCache
        }

        let components = computeWeaklyConnectedComponents()
        connectedComponentsCache = components
        isConnectedComponentsDirty = false

        return components
    }

    private func computeWeaklyConnectedComponents() -> [Set<VertexID>] {
        var visited = Set<VertexID>()
        var components: [Set<VertexID>] = []

        for vid in vertices.keys.sorted() {
            if !visited.contains(vid) {
                var component = Set<VertexID>()
                var queue: [VertexID] = [vid]

                visited.insert(vid)
                component.insert(vid)

                while !queue.isEmpty {
                    let v = queue.removeFirst()

                    for neighbor in adjacencyList[v] ?? [] {
                        if !visited.contains(neighbor) {
                            visited.insert(neighbor)
                            component.insert(neighbor)
                            queue.append(neighbor)
                        }
                    }

                    for neighbor in reverseAdjacencyList[v] ?? [] {
                        if !visited.contains(neighbor) {
                            visited.insert(neighbor)
                            component.insert(neighbor)
                            queue.append(neighbor)
                        }
                    }
                }

                components.append(component)
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
        var countedTriangles = Set<Set<VertexID>>()

        for v in vertices.keys {
            guard let neighbors = adjacencyList[v] else { continue }

            let neighborArray = Array(neighbors)

            for i in 0..<neighborArray.count {
                for j in (i+1)..<neighborArray.count {
                    let u = neighborArray[i]
                    let w = neighborArray[j]

                    let uwConnected = (adjacencyList[u]?.contains(w) == true) || (adjacencyList[w]?.contains(u) == true)

                    if uwConnected {
                        let triangle = Set([v, u, w])

                        if !countedTriangles.contains(triangle) {
                            countedTriangles.insert(triangle)
                            count += 1
                        }
                    }
                }
            }
        }

        return count
    }

    // MARK: - Full Recomputation (for performance comparison)

    func computeBFSFull(from start: VertexID) -> [VertexID: Int] {
        // Force full recomputation (ignore cache)
        bfsCache.removeValue(forKey: start)
        dirtyBFSStarts.remove(start)

        return bfsDistances(from: start)
    }

    func computeConnectedComponentsFull() -> [Set<VertexID>] {
        // Force full recomputation (ignore cache)
        isConnectedComponentsDirty = true

        return connectedComponents()
    }

    func computeTriangleCountFull() -> Int {
        // Force full recomputation (ignore cache)
        isTriangleCountDirty = true

        return triangleCount()
    }

    // MARK: - Persistence

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

// MARK: - Performance Tests

func testPerformance() {
    print("🧪 Performance Test: Incremental vs Full Recomputation")
    print(String(repeating: "=", count: 60))

    let vertexCount = 1000
    let edgeCount = 5000

    do {
        let filePath = "/tmp/test_performance.json"
        try? FileManager.default.removeItem(atPath: filePath)

        let graph = try PersistentGraph(filePath: filePath)

        // Add vertices
        print("\n📊 Creating graph with \(vertexCount) vertices and \(edgeCount) edges...")
        for i in 0..<vertexCount {
            try graph.addVertex(properties: ["name": .string("V\(i)")])
        }

        // Add random edges
        for _ in 0..<edgeCount {
            let from = UInt64(Int.random(in: 0..<vertexCount))
            let to = UInt64(Int.random(in: 0..<vertexCount))
            if from != to {
                try? graph.addEdge(from: from, to: to)
            }
        }

        let stats = graph.getStatistics()
        print("   Created graph with \(stats.vertexCount) vertices and \(stats.edgeCount) edges")

        // Test 1: BFS Performance
        print("\n📊 Test 1: BFS Performance")
        let startVertex = UInt64(0)

        // Full recomputation
        let bfsFullStart = CACurrentMediaTime()
        let bfsFull = graph.computeBFSFull(from: startVertex)
        let bfsFullTime = CACurrentMediaTime() - bfsFullStart
        print("   Full recomputation: \(String(format: "%.4f", bfsFullTime * 1000)) ms")

        // Incremental (should use cache)
        let bfsIncStart = CACurrentMediaTime()
        let bfsInc = graph.bfs(from: startVertex)
        let bfsIncTime = CACurrentMediaTime() - bfsIncStart
        print("   Incremental (cached): \(String(format: "%.4f", bfsIncTime * 1000)) ms")

        let bfsSpeedup = bfsFullTime / bfsIncTime
        print("   Speedup: \(String(format: "%.1f", bfsSpeedup))x")

        // Test 2: Connected Components Performance
        print("\n📊 Test 2: Connected Components Performance")

        // Full recomputation
        let ccFullStart = CACurrentMediaTime()
        let ccFull = graph.computeConnectedComponentsFull()
        let ccFullTime = CACurrentMediaTime() - ccFullStart
        print("   Full recomputation: \(String(format: "%.4f", ccFullTime * 1000)) ms")

        // Incremental (should use cache)
        let ccIncStart = CACurrentMediaTime()
        let ccInc = graph.connectedComponents()
        let ccIncTime = CACurrentMediaTime() - ccIncStart
        print("   Incremental (cached): \(String(format: "%.4f", ccIncTime * 1000)) ms")

        let ccSpeedup = ccFullTime / ccIncTime
        print("   Speedup: \(String(format: "%.1f", ccSpeedup))x")

        // Test 3: Triangle Counting Performance
        print("\n📊 Test 3: Triangle Counting Performance")

        // Full recomputation
        let tcFullStart = CACurrentMediaTime()
        let tcFull = graph.computeTriangleCountFull()
        let tcFullTime = CACurrentMediaTime() - tcFullStart
        print("   Full recomputation: \(String(format: "%.4f", tcFullTime * 1000)) ms")
        print("   Triangle count: \(tcFull)")

        // Incremental (should use cache)
        let tcIncStart = CACurrentMediaTime()
        let tcInc = graph.triangleCount()
        let tcIncTime = CACurrentMediaTime() - tcIncStart
        print("   Incremental (cached): \(String(format: "%.4f", tcIncTime * 1000)) ms")

        let tcSpeedup = tcFullTime / tcIncTime
        print("   Speedup: \(String(format: "%.1f", tcSpeedup))x")

        print("\n✅ Performance tests completed!")

        // Cleanup
        try? FileManager.default.removeItem(atPath: filePath)

    } catch {
        print("❌ Error: \(error)")
    }
}

testPerformance()
