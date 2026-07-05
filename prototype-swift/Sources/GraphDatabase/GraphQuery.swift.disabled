import Foundation

// MARK: - Graph Query Language (Simplified Cypher-like)

/*
 Usage Examples:

1. Find vertices by property:
    let results = graph.query().matchVertex(alias: "v").`where`(property: "name", equals: .string("Alice")).execute()

2. Find neighbors:
    let results = graph.query().matchNeighbors(of: 1).execute()

3. Find shortest path:
    let path = graph.query().shortestPath(from: 1, to: 5)

4. Find vertices in community:
    let community = graph.query().matchVertex(alias: "v").`where`(community: 0).execute()

5. Aggregation:
    let stats = graph.aggregate(property: "age", functions: [.count, .avg, .max, .min])

6. Find all paths:
    let paths = graph.findAllPaths(from: 1, to: 5, maxDepth: 5)

7. Extract subgraph:
    let subgraph = graph.extractSubgraph(vertices: [1, 2, 3, 4])
*/

// MARK: - Query Builder

class GraphQueryBuilder {
    private let graph: PersistentGraph
    private var steps: [QueryStep] = []

    init(graph: PersistentGraph) {
        self.graph = graph
    }

    // MARK: - Query Steps

    /// Match vertices with conditions
    func matchVertex(alias: String) -> GraphQueryBuilder {
        steps.append(.matchVertex(alias: alias, conditions: []))
        return self
    }

    /// Add WHERE clause (property equals)
    func `where`(property: String, equals value: PropertyValue) -> GraphQueryBuilder {
        if case .matchVertex(let alias, let conditions) = steps.last {
            var newConditions = conditions
            newConditions.append(.propertyEquals(property, value))
            steps[steps.count - 1] = .matchVertex(alias: alias, conditions: newConditions)
        }
        return self
    }

    /// Add WHERE clause (in community)
    func `where`(community: Int) -> GraphQueryBuilder {
        if case .matchVertex(let alias, let conditions) = steps.last {
            var newConditions = conditions
            newConditions.append(.inCommunity(community))
            steps[steps.count - 1] = .matchVertex(alias: alias, conditions: newConditions)
        }
        return self
    }

    /// Find neighbors of a vertex
    func matchNeighbors(of vertexId: VertexID) -> GraphQueryBuilder {
        steps.append(.matchNeighbors(of: vertexId, conditions: []))
        return self
    }

    /// Execute query and return vertex IDs
    func execute() -> [VertexID] {
        var results: [VertexID] = []

        for step in steps {
            switch step {
            case .matchVertex(_, let conditions):
                results = graph.findVertices(where: conditions)

            case .matchNeighbors(let vertexId, let conditions):
                results = graph.findNeighbors(of: vertexId, where: conditions)
            }
        }

        return results
    }

    /// Execute query and return detailed results
    func executeDetailed() -> [[String: Any]] {
        let vertexIds = execute()
        var results: [[String: Any]] = []

        for vid in vertexIds {
            if let vertex = graph.getVertex(id: vid) {
                var row: [String: Any] = [:]
                row["id"] = vid
                row["properties"] = vertex.properties
                results.append(row)
            }
        }

        return results
    }

    /// Find shortest path
    func shortestPath(from: VertexID, to: VertexID) -> [VertexID]? {
        return graph.shortestPath(from: from, to: to)
    }
}

// MARK: - Query Steps

enum QueryStep {
    case matchVertex(alias: String, conditions: [WhereCondition])
    case matchNeighbors(of: VertexID, conditions: [WhereCondition])
}

// MARK: - Where Conditions

enum WhereCondition {
    case propertyEquals(String, PropertyValue)
    case propertyGreaterThan(String, PropertyValue)
    case propertyLessThan(String, PropertyValue)
    case hasProperty(String)
    case inCommunity(Int)
}

// MARK: - Aggregation Functions

enum AggregationFunction {
    case count
    case sum
    case avg
    case max
    case min
}

// MARK: - Extension to PersistentGraph

extension PersistentGraph {
    /// Create a query builder
    func query() -> GraphQueryBuilder {
        return GraphQueryBuilder(graph: self)
    }

    /// Find vertices matching conditions
    func findVertices(where conditions: [WhereCondition]) -> [VertexID] {
        var result: [VertexID] = []

        for (vid, _) in vertices {
            if matchesConditions(vid, conditions: conditions) {
                result.append(vid)
            }
        }

        return result
    }

    /// Find neighbors of a vertex matching conditions
    func findNeighbors(of vertexId: VertexID, where conditions: [WhereCondition]) -> [VertexID] {
        let neighbors = getAllNeighbors(of: vertexId)

        if conditions.isEmpty {
            return Array(neighbors)
        }

        return neighbors.filter { neighborId in
            if let vertex = vertices[neighborId] {
                return matchesConditions(vertex.properties, conditions: conditions)
            }
            return false
        }
    }

    /// Find community of a vertex
    func findCommunity(of vertexId: VertexID) -> [VertexID] {
        let communities = getCommunitiesGreedy()
        let targetCommunity = communities.first { $0.contains(vertexId) }

        return targetCommunity ?? []
    }

    // MARK: - Aggregation Functions

    /// Aggregate vertex properties
    /// - Parameter property: Property name to aggregate
    /// - Parameter functions: Aggregation functions to compute
    /// - Returns: Dictionary mapping function name to result
    func aggregate(property: String, functions: [AggregationFunction]) -> [String: Double] {
        var results: [String: Double] = [:]

        // Extract numeric values
        var values: [Double] = []
        for (_, vertex) in vertices {
            if let propValue = vertex.properties[property] {
                switch propValue {
                case .int(let i):
                    values.append(Double(i))
                case .double(let d):
                    values.append(d)
                default:
                    continue
                }
            }
        }

        guard !values.isEmpty else { return [:] }

        for function in functions {
            switch function {
            case .count:
                results["count"] = Double(values.count)

            case .sum:
                results["sum"] = values.reduce(0, +)

            case .avg:
                results["avg"] = values.reduce(0, +) / Double(values.count)

            case .max:
                results["max"] = values.max()!

            case .min:
                results["min"] = values.min()!
            }
        }

        return results
    }

    // MARK: - Path Queries

    /// Find all paths between two vertices (with depth limit)
    /// - Parameters:
    ///   - from: Source vertex
    ///   - to: Target vertex
    ///   - maxDepth: Maximum path length (default: 5)
    /// - Returns: Array of paths (each path is an array of vertex IDs)
    func findAllPaths(from: VertexID, to: VertexID, maxDepth: Int = 5) -> [[VertexID]] {
        var paths: [[VertexID]] = []
        var currentPath: [VertexID] = [from]
        var visited: Set<VertexID> = [from]

        findAllPathsHelper(current: from, target: to, maxDepth: maxDepth, currentDepth: 0, currentPath: &currentPath, visited: &visited, paths: &paths)

        return paths
    }

    private func findAllPathsHelper(current: VertexID, target: VertexID, maxDepth: Int, currentDepth: Int, currentPath: inout [VertexID], visited: inout Set<VertexID>, paths: inout [[VertexID]]) {
        if currentDepth > maxDepth {
            return
        }

        if current == target && currentDepth > 0 {
            paths.append(currentPath)
            return
        }

        let neighbors = getAllNeighbors(of: current)
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                visited.insert(neighbor)
                currentPath.append(neighbor)
                findAllPathsHelper(current: neighbor, target: target, maxDepth: maxDepth, currentDepth: currentDepth + 1, currentPath: &currentPath, visited: &visited, paths: &paths)
                currentPath.removeLast()
                visited.remove(neighbor)
            }
        }
    }

    // MARK: - Subgraph Extraction

    /// Extract subgraph containing only specified vertices and edges between them
    /// - Parameter vertexIds: Vertices to include in subgraph
    /// - Returns: New PersistentGraph containing only the specified vertices and edges between them
    func extractSubgraph(vertices vertexIds: [VertexID]) -> PersistentGraph {
        let subgraph = PersistentGraph()

        // Add vertices
        for vid in vertexIds {
            if let vertex = vertices[vid] {
                try! subgraph.addVertex(id: vid, properties: vertex.properties)
            }
        }

        // Add edges between specified vertices
        for vid in vertexIds {
            let neighbors = getAllNeighbors(of: vid)
            for neighbor in neighbors {
                if vertexIds.contains(neighbor) {
                    if let edge = edges[EdgeID(from: vid, to: neighbor)] {
                        try! subgraph.addEdge(from: vid, to: neighbor, properties: edge.properties, weight: edge.weight)
                    }
                }
            }
        }

        return subgraph
    }

    // MARK: - Helper Methods

    private func matchesConditions(_ vertexId: VertexID, conditions: [WhereCondition]) -> Bool {
        for condition in conditions {
            if !matchesCondition(vertexId: vertexId, condition: condition) {
                return false
            }
        }
        return true
    }

    private func matchesCondition(vertexId: VertexID, condition: WhereCondition) -> Bool {
        guard let vertex = vertices[vertexId] else { return false }
        let properties = vertex.properties

        switch condition {
        case .propertyEquals(let key, let value):
            return properties[key] == value

        case .propertyGreaterThan(let key, let value):
            guard let propValue = properties[key] else { return false }
            return compareValues(propValue, value, operator: >)

        case .propertyLessThan(let key, let value):
            guard let propValue = properties[key] else { return false }
            return compareValues(propValue, value, operator: <)

        case .hasProperty(let key):
            return properties[key] != nil

        case .inCommunity(let communityId):
            let communities = getCommunitiesGreedy()
            for (idx, community) in communities.enumerated() {
                if community.contains(vertexId) {
                    return idx == communityId
                }
            }
            return false
        }
    }

    private func compareValues(_ lhs: PropertyValue, _ rhs: PropertyValue, operator: (Int) -> Bool) -> Bool {
        switch (lhs, rhs) {
        case (.int(let l), .int(let r)):
            return `operator`(l - r)
        case (.double(let l), .double(let r)):
            return `operator`(Int(l - r))
        default:
            return false
        }
    }
}
