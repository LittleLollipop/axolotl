import Foundation

// Debug version to test connected components

typealias VertexID = UInt64

struct EdgeID: Hashable {
    let from: VertexID
    let to: VertexID
}

class SimpleGraph {
    var vertices: [VertexID: String] = [:]
    var adjacencyList: [VertexID: Set<VertexID>] = [:]

    func addVertex(id: VertexID, name: String) {
        vertices[id] = name
        if adjacencyList[id] == nil {
            adjacencyList[id] = []
        }
    }

    func addEdge(from: VertexID, to: VertexID) {
        adjacencyList[from]?.insert(to)
    }

    func bfs(from start: VertexID) -> [VertexID] {
        guard vertices[start] != nil else { return [] }

        var visited = Set<VertexID>()
        var queue = [start]
        var result: [VertexID] = []

        visited.insert(start)

        while !queue.isEmpty {
            let v = queue.removeFirst()
            result.append(v)

            for neighbor in adjacencyList[v] ?? [] {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor)
                    queue.append(neighbor)
                }
            }
        }

        return result
    }

    func connectedComponents() -> [Set<VertexID>] {
        var visited = Set<VertexID>()
        var components: [Set<VertexID>] = []

        for vid in vertices.keys {
            if !visited.contains(vid) {
                let component = bfs(from: vid)
                let componentSet = Set(component)
                components.append(componentSet)
                visited.formUnion(componentSet)
            }
        }

        return components
    }

    func printGraph() {
        print("Vertices:")
        for (vid, name) in vertices.sorted(by: { $0.key < $1.key }) {
            print("  v\(vid): \(name)")
        }

        print("\nAdjacency List:")
        for (vid, neighbors) in adjacencyList.sorted(by: { $0.key < $1.key }) {
            print("  v\(vid) -> \(neighbors.sorted().map { "v\($0)" }.joined(separator: ", "))")
        }
    }
}

func testConnectedComponents() {
    print("🧪 Testing Connected Components")
    print(String(repeating: "=", count: 50))

    let graph = SimpleGraph()

    // Add vertices
    graph.addVertex(id: 0, name: "A")
    graph.addVertex(id: 1, name: "B")
    graph.addVertex(id: 2, name: "C")
    graph.addVertex(id: 3, name: "D")
    graph.addVertex(id: 4, name: "E")
    graph.addVertex(id: 5, name: "F")

    // Add edges
    graph.addEdge(from: 0, to: 1)
    graph.addEdge(from: 1, to: 2)
    graph.addEdge(from: 2, to: 3)
    graph.addEdge(from: 3, to: 4)
    graph.addEdge(from: 0, to: 2)
    graph.addEdge(from: 1, to: 3)

    print("✅ Graph created")

    // Print graph structure
    graph.printGraph()

    // Test BFS from v0
    let bfsResult = graph.bfs(from: 0)
    print("\n📊 BFS from v0:")
    print("   \(bfsResult.map { "v\($0)" }.joined(separator: " -> "))")

    // Test connected components
    let components = graph.connectedComponents()
    print("\n📊 Connected Components:")
    print("   Count: \(components.count)")
    for (i, component) in components.enumerated() {
        print("   Component \(i): \(component.map { "v\($0)" }.sorted().joined(separator: ", "))")
    }

    // All vertices should be in one component
    print("\n   Expected: 1 component")
    print("   Actual: \(components.count) components")

    if components.count == 1 {
        print("   ✅ Correct!")
    } else {
        print("   ❌ Wrong!")
    }
}

testConnectedComponents()
