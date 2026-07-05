import Foundation

// MARK: - Graph Query Language (Simplified Cypher-like)

/*
 Syntax Examples:

 1. Find vertices by property:
    MATCH (v:Vertex WHERE v.name = "Alice")

 2. Find neighbors:
    MATCH (v:Vertex)-[e:Edge]->(neighbor)

 3. Find shortest path:
    SHORTEST PATH FROM 1 TO 5

 4. Find vertices in community:
    MATCH (v:Vertex WHERE v.community = 0)

 5. Combine queries:
    MATCH (v:Vertex WHERE v.age > 25) - [e:knows] -> (friend)
*/

// MARK: - Query AST (Abstract Syntax Tree)

enum QueryNode {
    case matchVertex(alias: String, where: [WhereCondition])
    case matchEdge(from: String, alias: String, to: String, where: [WhereCondition])
    case shortestPath(from: VertexID, to: VertexID)
    case bfs(from: VertexID, maxDepth: Int?)
    case connectedComponent(of: VertexID)
    case community(of: VertexID)
}

enum WhereCondition {
    case propertyEquals(String, PropertyValue)
    case propertyGreaterThan(String, PropertyValue)
    case propertyLessThan(String, PropertyValue)
    case hasProperty(String)
}

// MARK: - Simple Query Builder

class GraphQuery {
    private let graph: PersistentGraph
    private var results: [[String: Any]] = []

    init(graph: PersistentGraph) {
        self.graph = graph
    }

    // MARK: - Query Methods

    /// Find vertices matching conditions
    func findVertices(where conditions: [WhereCondition]) -> [VertexID] {
        var result: [VertexID] = []

        for (vid, vertex) in graph.vertices {
            if matchesConditions(vertex.properties, conditions: conditions) {
                result.append(vid)
            }
        }

        return result
    }

    /// Find edges matching conditions
    func findEdges(where conditions: [WhereCondition]) -> [EdgeID] {
        var result: [EdgeID] = []

        for (edgeId, edge) in graph.edges {
            if matchesConditions(edge.properties, conditions: conditions) {
                result.append(edgeId)
            }
        }

        return result
    }

    /// Find neighbors of a vertex
    func findNeighbors(of vertexId: VertexID, where conditions: [WhereCondition] = []) -> [VertexID] {
        let neighbors = graph.getNeighbors(of: vertexId)

        if conditions.isEmpty {
            return neighbors
        }

        // Filter neighbors by their properties
        return neighbors.filter { neighborId in
            if let vertex = graph.getVertex(id: neighborId) {
                return matchesConditions(vertex.properties, conditions: conditions)
            }
            return false
        }
    }

    /// Find shortest path
    func findShortestPath(from: VertexID, to: VertexID) -> [VertexID]? {
        return graph.shortestPath(from: from, to: to)
    }

    /// Find vertices in the same community as a vertex
    func findCommunity(of vertexId: VertexID) -> [VertexID] {
        let communities = graph.getCommunitiesGreedy()
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
        }
    }

    private func compareValues(_ lhs: PropertyValue, _ rhs: PropertyValue, operator: (Int) -> Bool) -> Bool {
        switch (lhs, rhs) {
        case (.int(let l), .int(let r)):
            return operator(l - r)
        case (.double(let l), .double(let r)):
            return operator(Int(l - r))
        case (.string(let l), .string(let r)):
            return operator(l.count - r.count)  // Simplified: compare string lengths
        default:
            return false
        }
    }
}

// MARK: - Example Usage

func exampleQueries() {
    print("=== Graph Query Language Examples ===\n")

    // Note: This is a conceptual example
    // In practice, you would use the GraphQuery class like this:

    print("// Example 1: Find vertices by property")
    print("let alice = query.findVertices(where: [.propertyEquals(\"name\", .string(\"Alice\"))])")
    print("")

    print("// Example 2: Find neighbors")
    print("let friends = query.findNeighbors(of: 1, where: [.propertyEquals(\"type\", .string(\"knows\"))])")
    print("")

    print("// Example 3: Find shortest path")
    print("if let path = query.findShortestPath(from: 1, to: 5) {")
    print("    print(\"Shortest path: \\(path)\")")
    print("}")
    print("")

    print("// Example 4: Find community")
    print("let community = query.findCommunity(of: 1)")
    print("print(\"Community members: \\(community)\")")
    print("")

    print("=== Examples Complete ===")
}

// MARK: - Main Demo

print("=== Graph Query Language Demo ===\n")

// Create a sample graph
print("1. Creating sample graph...")
let graph = PersistentGraph(filePath: "/tmp/query_demo.json")

graph.addVertex(id: 1, properties: ["name": .string("Alice"), "age": .int(30), "active": .bool(true)])
graph.addVertex(id: 2, properties: ["name": .string("Bob"), "age": .int(25), "active": .bool(false)])
graph.addVertex(id: 3, properties: ["name": .string("Charlie"), "age": .int(35), "active": .bool(true)])
graph.addVertex(id: 4, properties: ["name": .string("Diana"), "age": .int(28), "active": .bool(true)])

graph.addEdge(from: 1, to: 2, properties: ["type": .string("knows")], weight: 1.0)
graph.addEdge(from: 2, to: 3, properties: ["type": .string("works_with")], weight: 1.0)
graph.addEdge(from: 3, to: 4, properties: ["type": .string("knows")], weight: 1.0)
graph.addEdge(from: 4, to: 1, properties: ["type": .string("knows")], weight: 1.0)

print("   Created graph with 4 vertices and 4 edges\n")

// Create query
let query = GraphQuery(graph: graph)

// Example 1: Find vertices by property
print("2. Query: Find vertices where name = 'Alice'")
let alice = query.findVertices(where: [.propertyEquals("name", .string("Alice"))])
print("   Result: \(alice)\n")

// Example 2: Find all active users
print("3. Query: Find vertices where active = true")
let activeUsers = query.findVertices(where: [.propertyEquals("active", .bool(true))])
print("   Result: \(activeUsers)\n")

// Example 3: Find neighbors of vertex 1
print("4. Query: Find neighbors of vertex 1")
let neighbors = query.findNeighbors(of: 1)
print("   Result: \(neighbors)\n")

// Example 4: Find shortest path
print("5. Query: Find shortest path from 1 to 3")
if let path = query.findShortestPath(from: 1, to: 3) {
    print("   Result: \(path)")
} else {
    print("   Result: No path found")
}
print("")

// Example 5: Find community
print("6. Query: Find community of vertex 1")
let community = query.findCommunity(of: 1)
print("   Result: \(community)\n")

print("=== Demo Complete ===")
