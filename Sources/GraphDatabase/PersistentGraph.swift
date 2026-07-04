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
    
    // Custom Codable implementation
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
    // MARK: - Properties
    private var vertices: [VertexID: Vertex] = [:]
    private var edges: [EdgeID: Edge] = [:]
    private var adjacencyList: [VertexID: Set<VertexID>] = [:]
    private var reverseAdjacencyList: [VertexID: Set<VertexID>] = [:] // For PageRank (incoming edges)
    private var propertyIndex: [String: [PropertyValue: Set<VertexID>]] = [:] // propertyName -> (value -> vertex IDs)
    private let filePath: String
    
    // MARK: - Incremental Algorithm Support
    private var dirtyVertices: Set<VertexID> = [] // Vertices affected by recent changes
    private var pagerankScores: [VertexID: Double] = [:] // Cached PageRank scores
    private var isPageRankDirty: Bool = true // Whether PageRank scores need to be recomputed
    private var bfsCache: [VertexID: [VertexID: Int]] = [:] // start -> (vertex -> distance)
    private var dirtyBFSStarts: Set<VertexID> = [] // BFS results that need to be recomputed
    
    // MARK: - Initialization
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
        
        // Update property index
        updatePropertyIndex(for: vid, properties: properties, isDelete: false)
        
        return vid
    }
    
    func deleteVertex(id: VertexID) throws {
        guard vertices[id] != nil else {
            throw GraphError.vertexNotFound(id)
        }
        
        // Remove from property index
        if let vertex = vertices[id] {
            updatePropertyIndex(for: id, properties: vertex.properties, isDelete: true)
        }
        
        // Remove all connected edges
        let neighbors = adjacencyList[id] ?? []
        for neighbor in neighbors {
            edges.removeValue(forKey: EdgeID(from: id, to: neighbor))
            adjacencyList[neighbor]?.remove(id)
        }
        
        // Remove incoming edges
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
        reverseAdjacencyList[to]?.insert(from) // Maintain reverse adjacency list
        
        // Mark affected vertices as dirty for incremental algorithms
        dirtyVertices.insert(from)
        dirtyVertices.insert(to)
        isPageRankDirty = true
        
        // Mark BFS cache as dirty for affected start vertices
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
        reverseAdjacencyList[to]?.remove(from) // Maintain reverse adjacency list
        
        // Mark affected vertices as dirty for incremental algorithms
        dirtyVertices.insert(from)
        dirtyVertices.insert(to)
        isPageRankDirty = true
        
        // Mark BFS cache as dirty for affected start vertices
        markBFSCacheDirty(for: from)
        markBFSCacheDirty(for: to)
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
    
    /// BFS traversal from start vertex
    /// Returns array of vertices in BFS order (up to maxDepth)
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
    
    /// Shortest path from start to target (unweighted BFS)
    /// Returns array of vertex IDs representing the path, or empty array if no path
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
    
    /// Incremental BFS (uses cache when possible)
    /// Returns dictionary mapping vertex ID to distance from start
    func bfsDistances(from start: VertexID) -> [VertexID: Int] {
        guard vertices[start] != nil else { return [:] }
        
        // Check if cache is valid
        if let cached = bfsCache[start], !dirtyBFSStarts.contains(start) {
            return cached
        }
        
        // Recompute BFS
        let distances = computeBFS(from: start)
        bfsCache[start] = distances
        dirtyBFSStarts.remove(start)
        
        return distances
    }
    
    /// Get BFS traversal order (uses cache)
    func bfs(from start: VertexID, maxDepth: Int = Int.max) -> [VertexID] {
        let distances = bfsDistances(from: start)
        
        // Sort by distance, then by vertex ID for determinism
        let result = distances.sorted { (a, b) in
            if a.value != b.value {
                return a.value < b.value
            }
            return a.key < b.key
        }.prefix(while: { $0.value <= maxDepth }).map { $0.key }
        
        return result
    }
    
    /// Mark BFS cache as dirty for affected start vertices
    private func markBFSCacheDirty(for vertexId: VertexID) {
        // If vertexId is reachable from a start vertex, that start vertex's BFS cache is dirty
        // For simplicity, mark all cached BFS results as dirty
        // In a more optimized version, we would only mark BFS results for start vertices that can reach vertexId
        for start in bfsCache.keys {
            if isReachable(from: start, to: vertexId) || isReachable(from: vertexId, to: start) {
                dirtyBFSStarts.insert(start)
            }
        }
    }
    
    /// Check if there's a path from start to target
    private func isReachable(from start: VertexID, to target: VertexID) -> Bool {
        if let distances = bfsCache[start] {
            return distances[target] != nil
        }
        return true // Conservative: assume reachable if we don't have cache
    }
    
    /// Compute BFS distances from start vertex
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
                
                if !visited.contains(neighbor) {
                    visited.insert(neighbor)
                    parent[neighbor] = v
                    queue.append(neighbor)
                }
            }
        }
        
        return [] // No path found
    }
    
    /// PageRank algorithm (iterative)
    /// Returns dictionary mapping vertex ID to PageRank score
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
    
    /// Incremental PageRank (only updates dirty vertices)
    /// This is much faster than full recomputation when only a few edges have changed
    /// Returns dictionary mapping vertex ID to PageRank score
    func incrementalPageRank(dampingFactor: Double = 0.85, maxIterations: Int = 100, tolerance: Double = 1e-6) -> [VertexID: Double] {
        let n = vertices.count
        guard n > 0 else { return [:] }
        
        // If no vertices are dirty, return cached scores
        if !isPageRankDirty && !dirtyVertices.isEmpty {
            // Only a few vertices changed, do incremental update
            return computeIncrementalPageRank(dampingFactor: dampingFactor, maxIterations: maxIterations, tolerance: tolerance)
        }
        
        // Otherwise, compute full PageRank
        let scores = computeFullPageRank(dampingFactor: dampingFactor, maxIterations: maxIterations, tolerance: tolerance)
        pagerankScores = scores
        isPageRankDirty = false
        dirtyVertices.removeAll()
        
        return scores
    }
    
    /// Get PageRank scores (uses incremental update if possible)
    func getPageRank(dampingFactor: Double = 0.85) -> [VertexID: Double] {
        if !isPageRankDirty {
            // Return cached scores
            return pagerankScores
        }
        
        // Need to recompute
        let scores = incrementalPageRank(dampingFactor: dampingFactor)
        return scores
    }
    
    // MARK: - Private PageRank Helpers
    
    private func computeFullPageRank(dampingFactor: Double = 0.85, maxIterations: Int = 100, tolerance: Double = 1e-6) -> [VertexID: Double] {
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
    
    private func computeIncrementalPageRank(dampingFactor: Double = 0.85, maxIterations: Int = 100, tolerance: Double = 1e-6) -> [VertexID: Double] {
        let n = vertices.count
        guard n > 0 else { return [:] }
        
        // Start with cached scores (or initialize if empty)
        var scores = pagerankScores
        if scores.isEmpty {
            let initialScore = 1.0 / Double(n)
            for vid in vertices.keys {
                scores[vid] = initialScore
            }
        }
        
        // Precompute out-degrees
        var outDegrees = [VertexID: Double]()
        for (vid, _) in vertices {
            outDegrees[vid] = Double(adjacencyList[vid]?.count ?? 0)
        }
        
        // Only process dirty vertices and their neighbors
        let affectedVertices = dirtyVertices.union(
            dirtyVertices.flatMap { vid in
                return Array(reverseAdjacencyList[vid] ?? [])
            }
        )
        
        // Power iteration (only update affected vertices)
        for _ in 0..<maxIterations {
            var newScores = scores
            
            // Only update scores for affected vertices
            for vid in affectedVertices {
                let damping = (1.0 - dampingFactor) / Double(n)
                var newScore = damping
                
                // Sum contributions from incoming edges
                for neighbor in reverseAdjacencyList[vid] ?? [] {
                    let neighborScore = scores[neighbor] ?? 0
                    let neighborOutDegree = outDegrees[neighbor] ?? 0
                    
                    if neighborOutDegree > 0 {
                        newScore += neighborScore * dampingFactor / neighborOutDegree
                    } else {
                        // Dangling node
                        newScore += neighborScore * dampingFactor / Double(n)
                    }
                }
                
                newScores[vid] = newScore
            }
            
            // Check convergence for affected vertices only
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
        
        // Update cache
        pagerankScores = scores
        isPageRankDirty = false
        dirtyVertices.removeAll()
        
        return scores
    }
    
    // MARK: - Property Index
    
    private func updatePropertyIndex(for vertexId: VertexID, properties: [String: PropertyValue], isDelete: Bool) {
        for (key, value) in properties {
            if propertyIndex[key] == nil {
                propertyIndex[key] = [:]
            }
            
            if isDelete {
                // Remove from index
                propertyIndex[key]?[value]?.remove(vertexId)
                if propertyIndex[key]?[value]?.isEmpty == true {
                    propertyIndex[key]?.removeValue(forKey: value)
                }
            } else {
                // Add to index
                if propertyIndex[key]?[value] == nil {
                    propertyIndex[key]?[value] = []
                }
                propertyIndex[key]?[value]?.insert(vertexId)
            }
        }
    }
    
    /// Find vertices by property value
    /// Returns array of vertex IDs that have the given property with the given value
    func findByProperty(_ propertyName: String, value: PropertyValue) -> [VertexID] {
        return Array(propertyIndex[propertyName]?[value] ?? [])
    }
    
    /// Get all unique values for a property
    func getPropertyValues(_ propertyName: String) -> [PropertyValue] {
        guard let index = propertyIndex[propertyName] else {
            return []
        }
        return Array(index.keys)
    }
    
    /// Update vertex properties (and update index)
    func updateVertex(id: VertexID, properties: [String: PropertyValue]) throws {
        guard var vertex = vertices[id] else {
            throw GraphError.vertexNotFound(id)
        }
        
        // Remove old properties from index
        updatePropertyIndex(for: id, properties: vertex.properties, isDelete: true)
        
        // Update vertex
        vertex.properties = properties
        vertices[id] = vertex
        
        // Add new properties to index
        updatePropertyIndex(for: id, properties: properties, isDelete: false)
    }
    
    // MARK: - Persistence (JSON format)
    
    func save() throws {
        let data = try serializeToJSON()
        
        // Atomic write
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
        
        // Rebuild property index
        propertyIndex.removeAll()
        for (vid, vertex) in vertices {
            updatePropertyIndex(for: vid, properties: vertex.properties, isDelete: false)
        }
        
        // Rebuild reverse adjacency list
        reverseAdjacencyList.removeAll()
        for (vid, _) in vertices {
            reverseAdjacencyList[vid] = []
        }
        for (edgeId, _) in edges {
            reverseAdjacencyList[edgeId.to]?.insert(edgeId.from)
        }
        
        // Mark PageRank as dirty
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
