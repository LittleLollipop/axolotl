import Foundation

// MARK: - Binary Format Constants

struct BinaryFormat {
    static let magic: [UInt8] = [0x41, 0x58, 0x4F, 0x4C]  // "AXOL"
    static let version: UInt16 = 1
    static let headerSize = 40  // 4 + 2 + 8 + 8 + 16 = 40 bytes

    enum PropertyType: UInt8 {
        case string = 0
        case int = 1
        case double = 2
        case bool = 3
        case null = 4
    }
}

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
    private var ssspCache: [VertexID: [VertexID: Int]] = [:] // start -> (vertex -> distance) for SSSP
    private var dirtySSSPStarts: Set<VertexID> = [] // SSSP results that need to be recomputed
    private var connectedComponentsCache: [Set<VertexID>] = [] // Cached connected components
    private var isConnectedComponentsDirty: Bool = true
    private var triangleCountCache: Int = 0 // Cached triangle count
    private var isTriangleCountDirty: Bool = true
    private var betweennessScores: [VertexID: Double] = [:] // Cached Betweenness Centrality scores
    private var isBetweennessDirty: Bool = true

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
        isConnectedComponentsDirty = true
        isTriangleCountDirty = true

        // Mark BFS cache as dirty for affected start vertices
        markBFSCacheDirty(for: from)
        markBFSCacheDirty(for: to)

        // Mark SSSP cache as dirty
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
        reverseAdjacencyList[to]?.remove(from) // Maintain reverse adjacency list

        // Mark affected vertices as dirty for incremental algorithms
        dirtyVertices.insert(from)
        dirtyVertices.insert(to)
        isPageRankDirty = true
        isConnectedComponentsDirty = true
        isTriangleCountDirty = true

        // Mark BFS cache as dirty for affected start vertices
        markBFSCacheDirty(for: from)
        markBFSCacheDirty(for: to)

        // Mark SSSP cache as dirty
        markSSSPCacheDirty(for: from)
        markSSSPCacheDirty(for: to)
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

                if !visited.contains(neighbor) {
                    visited.insert(neighbor)
                    parent[neighbor] = v
                    queue.append(neighbor)
                }
            }
        }

        return [] // No path found
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

    // MARK: - Incremental SSSP (Shortest Path)

    /// Get SSSP distances (uses incremental update if possible)
    func sssp(from start: VertexID) -> [VertexID: Int] {
        guard vertices[start] != nil else { return [:] }

        // Check if cache is valid
        if let cached = ssspCache[start], !dirtySSSPStarts.contains(start) {
            return cached
        }

        // Recompute SSSP
        let distances = computeSSSP(from: start)
        ssspCache[start] = distances
        dirtySSSPStarts.remove(start)

        return distances
    }

    /// Compute SSSP using BFS (unweighted)
    private func computeSSSP(from start: VertexID) -> [VertexID: Int] {
        return computeBFS(from: start)
    }

    /// Mark SSSP cache as dirty for affected start vertices
    private func markSSSPCacheDirty(for vertexId: VertexID) {
        for start in ssspCache.keys {
            if isReachable(from: start, to: vertexId) || isReachable(from: vertexId, to: start) {
                dirtySSSPStarts.insert(start)
            }
        }
    }

    // MARK: - Incremental Connected Components (Weakly Connected)

    /// Get weakly connected components (uses incremental update if possible)
    /// Weakly connected components ignore edge direction
    func connectedComponents() -> [Set<VertexID>] {
        if !isConnectedComponentsDirty {
            return connectedComponentsCache
        }

        let components = computeWeaklyConnectedComponents()
        connectedComponentsCache = components
        isConnectedComponentsDirty = false

        return components
    }

    /// Compute weakly connected components (ignore edge direction)
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

                    // Traverse outgoing edges
                    for neighbor in adjacencyList[v] ?? [] {
                        if !visited.contains(neighbor) {
                            visited.insert(neighbor)
                            component.insert(neighbor)
                            queue.append(neighbor)
                        }
                    }

                    // Traverse incoming edges (ignore direction)
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

    // MARK: - Incremental Triangle Counting

    /// Get triangle count (uses incremental update if possible)
    /// Counts undirected triangles (ignores edge direction)
    func triangleCount() -> Int {
        if !isTriangleCountDirty {
            return triangleCountCache
        }

        let count = computeTriangleCount()
        triangleCountCache = count
        isTriangleCountDirty = false

        return count
    }

    /// Compute triangle count (undirected)
    /// A triangle is a set of 3 vertices where each pair is connected by at least one edge (in either direction)
    private func computeTriangleCount() -> Int {
        var count = 0
        var countedTriangles = Set<Set<VertexID>>() // To avoid double-counting

        for v in vertices.keys {
            guard let neighbors = adjacencyList[v] else { continue }

            // Convert neighbors to array for combination
            let neighborArray = Array(neighbors)

            // Count triangles of form (v, u, w) where u < w in neighborArray
            for i in 0..<neighborArray.count {
                for j in (i+1)..<neighborArray.count {
                    let u = neighborArray[i]
                    let w = neighborArray[j]

                    // Check if u and w are connected (in either direction)
                    let uwConnected = (adjacencyList[u]?.contains(w) == true) || (adjacencyList[w]?.contains(u) == true)

                    if uwConnected {
                        // Create a triangle set (unordered)
                        let triangle = Set([v, u, w])

                        // Only count if not already counted
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

    // MARK: - PageRank

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

    // MARK: - Betweenness Centrality

    /// Betweenness Centrality algorithm (BFS-based for unweighted graphs)
    /// Measures how often a vertex appears on shortest paths between other vertices
    /// Returns dictionary mapping vertex ID to betweenness centrality score
    func betweennessCentrality() -> [VertexID: Double] {
        let n = vertices.count
        guard n > 2 else { return [:] }

        // Check cache
        if !isBetweennessDirty {
            return betweennessScores
        }

        // Initialize betweenness scores
        var betweenness: [VertexID: Double] = [:]
        for vid in vertices.keys {
            betweenness[vid] = 0.0
        }

        // Compute betweenness for each vertex as source
        for s in vertices.keys {
            // BFS from s
            var stack: [VertexID] = []
            var distances: [VertexID: Int] = [:]
            var numSP: [VertexID: Double] = [:]
            var predecessors: [VertexID: [VertexID]] = [:]

            // Initialize
            for v in vertices.keys {
                distances[v] = -1
                numSP[v] = 0.0
                predecessors[v] = []
            }

            distances[s] = 0
            numSP[s] = 1.0

            var queue: [VertexID] = [s]

            // BFS
            while !queue.isEmpty {
                let v = queue.removeFirst()
                stack.append(v)

                for neighbor in adjacencyList[v] ?? [] {
                    // Path discovery
                    if distances[neighbor]! < 0 {
                        queue.append(neighbor)
                        distances[neighbor] = distances[v]! + 1
                    }

                    // Path counting
                    if distances[neighbor] == distances[v]! + 1 {
                        numSP[neighbor] = numSP[neighbor]! + numSP[v]!
                        predecessors[neighbor]?.append(v)
                    }
                }
            }

            // Accumulation (backward pass)
            var delta: [VertexID: Double] = [:]
            for v in vertices.keys {
                delta[v] = 0.0
            }

            // Process vertices in reverse BFS order
            while !stack.isEmpty {
                let w = stack.removeLast()
                if let preds = predecessors[w] {
                    for v in preds {
                        let contribution = (1.0 + delta[w]!) * (numSP[v]! / numSP[w]!)
                        delta[v] = delta[v]! + contribution
                    }
                }
                if w != s {
                    betweenness[w] = betweenness[w]! + delta[w]!
                }
            }
        }

        // Normalize (for undirected graphs)
        let normalizeFactor = Double(n - 1) * Double(n - 2) / 2.0
        if normalizeFactor > 0 {
            for vid in betweenness.keys {
                betweenness[vid] = betweenness[vid]! / normalizeFactor
            }
        }

        // Update cache
        betweennessScores = betweenness
        isBetweennessDirty = false

        return betweenness
    }

    /// Get Betweenness Centrality scores (uses cache if possible)
    func getBetweennessCentrality() -> [VertexID: Double] {
        if !isBetweennessDirty {
            return betweennessScores
        }

        return betweennessCentrality()
    }

    // MARK: - Community Detection (Greedy Modularity Optimization)

    /// Greedy Modularity Optimization for community detection
    /// Starts with each vertex as its own community, then iteratively merges communities
    /// Returns dictionary mapping vertex ID to community ID
    func detectCommunitiesGreedy(maxIterations: Int = 100) -> [VertexID: Int] {
        let n = vertices.count
        guard n > 0 else { return [:] }

        // Initialize: each vertex is its own community
        var communities: [VertexID: Int] = [:]
        var communityIds = Array(vertices.keys)
        for (i, vid) in communityIds.enumerated() {
            communities[vid] = i
        }

        // Compute initial modularity
        var bestModularity = computeModularity(communities: communities)

        // Iteratively merge communities
        for _ in 0..<min(maxIterations, n) {
            var bestMerge: (Int, Int, Double)? = nil

            // Try all pairs of communities
            let uniqueCommunities = Set(communities.values)
            let communityList = Array(uniqueCommunities)

            for i in 0..<communityList.count {
                for j in (i+1)..<communityList.count {
                    let c1 = communityList[i]
                    let c2 = communityList[j]

                    // Try merging c1 and c2
                    var testCommunities = communities
                    for vid in testCommunities.keys {
                        if testCommunities[vid] == c2 {
                            testCommunities[vid] = c1
                        }
                    }

                    let modularity = computeModularity(communities: testCommunities)

                    if modularity > bestModularity {
                        bestModularity = modularity
                        bestMerge = (c1, c2, modularity)
                    }
                }
            }

            // Apply best merge
            if let (c1, c2, _) = bestMerge {
                for vid in communities.keys {
                    if communities[vid] == c2 {
                        communities[vid] = c1
                    }
                }
            } else {
                break  // No improvement
            }
        }

        // Renumber communities consecutively
        let uniqueCommunities = Set(communities.values)
        let sortedCommunities = uniqueCommunities.sorted()
        var communityMapping: [Int: Int] = [:]
        for (i, c) in sortedCommunities.enumerated() {
            communityMapping[c] = i
        }

        for vid in communities.keys {
            communities[vid] = communityMapping[communities[vid]!]!
        }

        return communities
    }

    /// Compute modularity of current community assignment
    private func computeModularity(communities: [VertexID: Int]) -> Double {
        let m = Double(edges.count * 2)  // Total number of undirected edges (count both directions)
        guard m > 0 else { return 0.0 }

        var Q = 0.0

        // For each pair of vertices
        for (vid, community) in communities {
            guard let neighbors = adjacencyList[vid] else { continue }

            for neighbor in neighbors {
                guard let neighborCommunity = communities[neighbor] else { continue }

                // Fraction of edges within community
                let ki = Double(adjacencyList[vid]?.count ?? 0)
                let kj = Double(adjacencyList[neighbor]?.count ?? 0)

                let delta = (community == neighborCommunity) ? 1.0 : 0.0
                let expected = (ki * kj) / (2.0 * m)

                Q += (delta - expected) / (2.0 * m)
            }
        }

        return Q
    }

    /// Get communities (grouped by label) using greedy optimization
    func getCommunitiesGreedy(maxIterations: Int = 100) -> [[VertexID]] {
        let communities = detectCommunitiesGreedy(maxIterations: maxIterations)

        // Group vertices by community
        var result: [Int: [VertexID]] = [:]
        for (vid, community) in communities {
            result[community, default: []].append(vid)
        }

        return Array(result.values)
    }

    // MARK: - Community Detection (Louvain Algorithm)

    /// Louvain Algorithm for community detection (with resolution parameter)
    /// - Parameters:
    ///   - maxIterations: Maximum number of iterations
    ///   - resolution: Resolution parameter (default: 1.0)
    ///     - resolution > 1: Prefer smaller communities
    ///     - resolution < 1: Prefer larger communities (helps with star graphs)
    /// - Returns: Dictionary mapping vertex ID to community ID
    func detectCommunitiesLouvain(maxIterations: Int = 100, resolution: Double = 1.0) -> [VertexID: Int] {
        let n = vertices.count
        guard n > 0 else { return [:] }

        // Initialize: each vertex is its own community
        var communities: [VertexID: Int] = [:]
        for (i, vid) in vertices.keys.enumerated() {
            communities[vid] = i
        }

        // Compute total number of edges (for modularity calculation)
        let m = Double(edges.count) / 2.0 // Assuming undirected edges
        guard m > 0 else { return communities }

        // Louvain iterations (with community aggregation)
        var currentGraph = self
        var currentCommunities = communities
        var iteration = 0

        while iteration < maxIterations {
            // Phase 1: Modularity optimization
            var improved = false
            let vertexOrder = Array(currentGraph.vertices.keys).shuffled()

            for vid in vertexOrder {
                let neighbors = currentGraph.getAllNeighbors(of: vid)
                if neighbors.isEmpty { continue }

                let currentCommunity = currentCommunities[vid]!

                // Compute sigma_tot for each community (sum of degrees)
                var sigmaTot: [Int: Double] = [:]
                for (otherVid, community) in currentCommunities {
                    let degree = Double(currentGraph.getAllNeighbors(of: otherVid).count)
                    sigmaTot[community, default: 0.0] += degree
                }

                // Compute k_i (degree of vid)
                let k_i = Double(neighbors.count)

                // Compute k_i_in for each community (sum of edge weights from vid to community)
                var k_i_in: [Int: Double] = [:]
                for neighbor in neighbors {
                    let neighborCommunity = currentCommunities[neighbor]!
                    let weight = currentGraph.getEdgeWeight(from: vid, to: neighbor)
                    k_i_in[neighborCommunity, default: 0.0] += weight
                }

                // Find best community for vid
                var bestCommunity = currentCommunity
                var bestGain = 0.0

                // Compute gain for removing vid from current community
                let k_i_in_current = k_i_in[currentCommunity] ?? 0.0
                let removeGain = k_i_in_current / (2.0 * m) -
                    (sigmaTot[currentCommunity]! - k_i) * k_i / (4.0 * m * m * resolution)

                // Try adding vid to each neighboring community
                for (community, k_i_in_val) in k_i_in {
                    if community == currentCommunity { continue }

                    let addGain = k_i_in_val / (2.0 * m) -
                        sigmaTot[community]! * k_i / (4.0 * m * m * resolution)

                    if addGain > bestGain {
                        bestGain = addGain
                        bestCommunity = community
                    }
                }

                // Move vertex if improvement
                let totalGain = bestGain - removeGain
                if bestCommunity != currentCommunity && totalGain > 0 {
                    currentCommunities[vid] = bestCommunity
                    improved = true
                }
            }

            if !improved {
                break
            }

            // Phase 2: Community aggregation
            let uniqueCommunities = Set(currentCommunities.values)
            if uniqueCommunities.count == currentGraph.vertices.count {
                // No more merging possible
                break
            }

            // Create new graph where communities are super-vertices
            // For now, just continue with current partition
            // Full aggregation would require creating a new graph
            break
        }

        return currentCommunities
    }

    /// Get all neighbors (both outgoing and incoming edges)
    private func getAllNeighbors(of vertex: VertexID) -> Set<VertexID> {
        var neighbors: Set<VertexID> = []
        if let outgoing = adjacencyList[vertex] {
            neighbors.formUnion(outgoing)
        }
        if let incoming = reverseAdjacencyList[vertex] {
            neighbors.formUnion(incoming)
        }
        return neighbors
    }

    /// Get edge weight between two vertices
    private func getEdgeWeight(from: VertexID, to: VertexID) -> Double {
        if let edge = edges[EdgeID(from: from, to: to)] {
            return edge.weight
        }
        return 1.0 // Default weight
    }

    /// Get communities using Louvain algorithm (grouped by label)
    /// - Parameters:
    ///   - maxIterations: Maximum number of iterations
    ///   - resolution: Resolution parameter (default: 1.0)
    /// - Returns: Array of communities, each community is an array of vertex IDs
    func getCommunitiesLouvain(maxIterations: Int = 100, resolution: Double = 1.0) -> [[VertexID]] {
        let communities = detectCommunitiesLouvain(maxIterations: maxIterations, resolution: resolution)

        // Group vertices by community
        var result: [Int: [VertexID]] = [:]
        for (vid, community) in communities {
            result[community, default: []].append(vid)
        }

        return Array(result.values)
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

    // MARK: - Graph Visualization (DOT format)

    /// Export graph to Graphviz DOT format
    /// - Parameters:
    ///   - directed: Whether to use directed edges (digraph) or undirected (graph)
    ///   - labelProperty: Property name to use as vertex label (default: "name")
    ///   - coloredByCommunity: Whether to color vertices by community
    /// - Returns: DOT format string
    func exportToDOT(directed: Bool = true, labelProperty: String = "name", coloredByCommunity: Bool = false) -> String {
        var dot = ""

        // Header
        if directed {
            dot += "digraph G {\n"
        } else {
            dot += "graph G {\n"
        }

        dot += "  // Graph generated by Axolotl\n"
        dot += "  node [shape=circle, style=filled];\n\n"

        // Compute communities if needed
        var communities: [VertexID: Int] = [:]
        if coloredByCommunity {
            communities = detectCommunitiesGreedy()
        }

        // Vertex colors for communities
        let colors = ["#FF6B6B", "#4ECDC4", "#45B7D1", "#96CEB4", "#FFEAA7", "#DDA0DD", "#98D8C8", "#F7DC6F", "#BB8FCE", "#85C1E9"]

        // Vertices
        dot += "  // Vertices\n"
        for (vid, vertex) in vertices {
            let label = vertex.properties[labelProperty]?.description ?? "\(vid)"

            var attrs = "label=\"\(label)\""
            if coloredByCommunity, let community = communities[vid] {
                let colorIndex = community % colors.count
                attrs += ", fillcolor=\"\(colors[colorIndex])\""
            }

            dot += "  \(vid) [\(attrs)];\n"
        }

        dot += "\n"

        // Edges
        dot += "  // Edges\n"
        for (edgeId, edge) in edges {
            let edgeSymbol = directed ? "->" : "--"

            var attrs = ""
            if edge.weight != 1.0 {
                attrs = " [weight=\(edge.weight)]"
            }

            dot += "  \(edgeId.from) \(edgeSymbol) \(edgeId.to)\(attrs);\n"
        }

        dot += "}\n"

        return dot
    }

    /// Save graph to DOT file
    func saveToDOT(filePath: String, directed: Bool = true, labelProperty: String = "name", coloredByCommunity: Bool = false) throws {
        let dot = exportToDOT(directed: directed, labelProperty: labelProperty, coloredByCommunity: coloredByCommunity)
        try dot.write(to: URL(fileURLWithPath: filePath), atomically: true, encoding: .utf8)
    }

    /// Export graph with communities highlighted
    func exportWithCommunities(filePath: String) throws {
        try saveToDOT(filePath: filePath, directed: false, labelProperty: "name", coloredByCommunity: true)
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

    // MARK: - Persistence (Binary format)

    /// Save graph to binary file
    func saveBinary() throws {
        let data = try serializeToBinary()

        // Atomic write
        let tempPath = filePath.replacingOccurrences(of: ".json", with: ".bin") + ".tmp"
        if FileManager.default.fileExists(atPath: tempPath) {
            try FileManager.default.removeItem(atPath: tempPath)
        }

        try data.write(to: URL(fileURLWithPath: tempPath))

        let binaryPath = filePath.replacingOccurrences(of: ".json", with: ".bin")
        if FileManager.default.fileExists(atPath: binaryPath) {
            try FileManager.default.removeItem(atPath: binaryPath)
        }

        try FileManager.default.moveItem(atPath: tempPath, toPath: binaryPath)

        print("✅ Database saved to \(binaryPath) (binary format)")
        print("   Vertices: \(vertices.count)")
        print("   Edges: \(edges.count)")
    }

    /// Load graph from binary file
    func loadBinary() throws {
        let binaryPath = filePath.replacingOccurrences(of: ".json", with: ".bin")
        let data = try Data(contentsOf: URL(fileURLWithPath: binaryPath))
        try deserializeFromBinary(data: data)

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
        isConnectedComponentsDirty = true
        isTriangleCountDirty = true

        print("✅ Database loaded from \(binaryPath) (binary format)")
        print("   Vertices: \(vertices.count)")
        print("   Edges: \(edges.count)")
    }

    /// Serialize graph to binary format
    private func serializeToBinary() throws -> Data {
        let buffer = BinaryBuffer()

        // Write header
        buffer.writeBytes(BinaryFormat.magic)  // Magic: "AXOL"
        buffer.writeUInt16(BinaryFormat.version)  // Version
        buffer.writeUInt64(UInt64(vertices.count))  // Vertex count
        buffer.writeUInt64(UInt64(edges.count))  // Edge count
        buffer.writeBytes([UInt8](repeating: 0, count: 16))  // Reserved

        // Write vertices
        for (_, vertex) in vertices {
            buffer.writeUInt64(vertex.id)
            buffer.writeUInt32(UInt32(vertex.properties.count))

            for (key, value) in vertex.properties {
                buffer.writeString(key)
                buffer.writePropertyValue(value)
            }
        }

        // Write edges
        for (_, edge) in edges {
            buffer.writeUInt64(edge.id.from)
            buffer.writeUInt64(edge.id.to)
            buffer.writeDouble(edge.weight)
            buffer.writeUInt32(UInt32(edge.properties.count))

            for (key, value) in edge.properties {
                buffer.writeString(key)
                buffer.writePropertyValue(value)
            }
        }

        return buffer.data
    }

    /// Deserialize graph from binary format
    private func deserializeFromBinary(data: Data) throws {
        let buffer = BinaryBuffer(data: data)

        // Read header
        let magic = buffer.readBytes(4)
        guard magic == BinaryFormat.magic else {
            throw GraphError.invalidFormat
        }

        let version = buffer.readUInt16()
        guard version == BinaryFormat.version else {
            throw GraphError.invalidFormat
        }

        let vertexCount = buffer.readUInt64()
        let edgeCount = buffer.readUInt64()
        buffer.readBytes(16)  // Skip reserved

        // Clear existing data
        vertices.removeAll()
        edges.removeAll()
        adjacencyList.removeAll()

        // Read vertices
        for _ in 0..<vertexCount {
            let id = buffer.readUInt64()
            let propertyCount = buffer.readUInt32()

            var properties: [String: PropertyValue] = [:]
            for _ in 0..<propertyCount {
                let key = buffer.readString()
                let value = buffer.readPropertyValue()
                properties[key] = value
            }

            let vertex = Vertex(id: id, properties: properties)
            vertices[id] = vertex
            adjacencyList[id] = []
        }

        // Read edges
        for _ in 0..<edgeCount {
            let from = buffer.readUInt64()
            let to = buffer.readUInt64()
            let weight = buffer.readDouble()
            let propertyCount = buffer.readUInt32()

            var properties: [String: PropertyValue] = [:]
            for _ in 0..<propertyCount {
                let key = buffer.readString()
                let value = buffer.readPropertyValue()
                properties[key] = value
            }

            let edgeId = EdgeID(from: from, to: to)
            let edge = Edge(id: edgeId, properties: properties, weight: weight)
            edges[edgeId] = edge
            adjacencyList[from]?.insert(to)
        }
    }
}

// MARK: - Binary Buffer Helper

class BinaryBuffer {
    var data: Data

    init() {
        self.data = Data()
    }

    init(data: Data) {
        self.data = data
    }

    // Write methods

    func writeBytes(_ bytes: [UInt8]) {
        data.append(contentsOf: bytes)
    }

    func writeUInt16(_ value: UInt16) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: MemoryLayout<UInt16>.size))
    }

    func writeUInt32(_ value: UInt32) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: MemoryLayout<UInt32>.size))
    }

    func writeUInt64(_ value: UInt64) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: MemoryLayout<UInt64>.size))
    }

    func writeDouble(_ value: Double) {
        var value = value.bitPattern.bigEndian
        data.append(Data(bytes: &value, count: MemoryLayout<UInt64>.size))
    }

    func writeString(_ string: String) {
        let bytes = [UInt8](string.utf8)
        writeUInt32(UInt32(bytes.count))
        writeBytes(bytes)
    }

    func writePropertyValue(_ value: PropertyValue) {
        switch value {
        case .string(let s):
            writeBytes([BinaryFormat.PropertyType.string.rawValue])
            writeString(s)
        case .int(let i):
            writeBytes([BinaryFormat.PropertyType.int.rawValue])
            var intValue = Int64(i).bigEndian
            data.append(Data(bytes: &intValue, count: MemoryLayout<Int64>.size))
        case .double(let d):
            writeBytes([BinaryFormat.PropertyType.double.rawValue])
            writeDouble(d)
        case .bool(let b):
            writeBytes([BinaryFormat.PropertyType.bool.rawValue])
            writeBytes([b ? 1 : 0])
        case .null:
            writeBytes([BinaryFormat.PropertyType.null.rawValue])
        }
    }

    // Read methods

    private var readOffset = 0

    func readBytes(_ count: Int) -> [UInt8] {
        let bytes = [UInt8](data[readOffset..<readOffset + count])
        readOffset += count
        return bytes
    }

    func readUInt16() -> UInt16 {
        let value = data.withUnsafeBytes { $0.load(fromByteOffset: readOffset, as: UInt16.self) }
        readOffset += MemoryLayout<UInt16>.size
        return UInt16(bigEndian: value)
    }

    func readUInt32() -> UInt32 {
        let value = data.withUnsafeBytes { $0.load(fromByteOffset: readOffset, as: UInt32.self) }
        readOffset += MemoryLayout<UInt32>.size
        return UInt32(bigEndian: value)
    }

    func readUInt64() -> UInt64 {
        let value = data.withUnsafeBytes { $0.load(fromByteOffset: readOffset, as: UInt64.self) }
        readOffset += MemoryLayout<UInt64>.size
        return UInt64(bigEndian: value)
    }

    func readDouble() -> Double {
        let bitPattern = readUInt64()
        return Double(bitPattern: bitPattern)
    }

    func readString() -> String {
        let length = readUInt32()
        let bytes = readBytes(Int(length))
        return String(bytes: bytes, encoding: .utf8) ?? ""
    }

    func readPropertyValue() -> PropertyValue {
        let typeRaw = readBytes(1)[0]
        guard let type = BinaryFormat.PropertyType(rawValue: typeRaw) else {
            return .null
        }

        switch type {
        case .string:
            return .string(readString())
        case .int:
            var value: Int64 = 0
            let bytes = readBytes(MemoryLayout<Int64>.size)
            value = bytes.withUnsafeBufferPointer {
                $0.baseAddress!.withMemoryRebound(to: Int64.self, capacity: 1) {
                    Int64(bigEndian: $0.pointee)
                }
            }
            return .int(Int(value))
        case .double:
            return .double(readDouble())
        case .bool:
            let b = readBytes(1)[0]
            return .bool(b == 1)
        case .null:
            return .null
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
