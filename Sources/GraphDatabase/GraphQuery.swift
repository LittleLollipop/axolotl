import Foundation

// MARK: - Graph Query Language (Simplified Cypher-like)

/*
 Usage Examples:

 1. Find vertices by property:
    let results = graph.query().matchVertex(alias: "v").where(property: "name", equals: .string("Alice")).execute()

 2. Find neighbors:
    let results = graph.query().matchNeighbors(of: 1).execute()

 3. Find shortest path:
    let path = graph.query().shortestPath(from: 1, to: 5)

 4. Find vertices in community:
    let community = graph.query().matchVertex(alias: "v").whereCommunity(is: 0).execute()
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

    /// Add WHERE clause
    func `where`(property: String, equals value: PropertyValue) -> GraphQueryBuilder {
        if case .matchVertex(let alias, let conditions) = steps.last {
            var newConditions = conditions
            newConditions.append(.propertyEquals(property, value))
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

// MARK: - Extension to PersistentGraph

extension PersistentGraph {
    /// Create a query builder
    func query() -> GraphQueryBuilder {
        return GraphQueryBuilder(graph: self)
    }

    /// Find vertices matching conditions
    func findVertices(where conditions: [WhereCondition]) -> [VertexID] {
        var result: [VertexID] = []

        for (vid, vertex) in vertices {
            if matchesConditions(vertex.properties, conditions: conditions) {
                result.append(vid)
            }
        }

        return result
    }

    /// Find neighbors of a vertex matching conditions
    func findNeighbors(of vertexId: VertexID, where conditions: [WhereCondition]) -> [VertexID] {
        let neighbors = getNeighbors(of: vertexId)

        if conditions.isEmpty {
            return neighbors
        }

        return neighbors.filter { neighborId in
            if let vertex = getVertex(id: neighborId) {
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

    // MARK: - Helper Methods

    private func matchesConditions(_ properties: [String: PropertyValue], conditions: [WhereCondition]) -> Bool {
        for condition in conditions {
            if !matchesCondition(properties, condition: condition) {
                return false
            }
        }
        return true
    }

    private func matchesCondition(_ properties: [String: PropertyValue], condition: WhereCondition) -> Bool {
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
            let vertexCommunity = communities.firstIndex { $0.contains(where: { $0 == getVertex(id: 1) != nil }) }
            return vertexCommunity == communityId
        }
    }

    private func compareValues(_ lhs: PropertyValue, _ rhs: PropertyValue, operator: (Int) -> Bool) -> Bool {
        switch (lhs, rhs) {
        case (.int(let l), .int(let r)):
            return operator(l - r)
        case (.double(let l), .double(let r)):
            return operator(Int(l - r))
        default:
            return false
        }
    }
}
