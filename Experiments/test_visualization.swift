// Test Graph Visualization (DOT format export)
// Compile: swift test_visualization.swift

import Foundation

// Quick test
class TestGraph {
    var vertices: [UInt64: String] = [:]
    var edges: [(UInt64, UInt64)] = []

    func addVertex(id: UInt64, label: String) {
        vertices[id] = label
    }

    func addEdge(from: UInt64, to: UInt64) {
        edges.append((from, to))
    }

    func exportToDOT() -> String {
        var dot = "digraph G {\n"
        dot += "  node [shape=circle, style=filled];\n\n"

        // Vertices
        for (id, label) in vertices {
            dot += "  \(id) [label=\"\(label)\"];\n"
        }

        dot += "\n"

        // Edges
        for (from, to) in edges {
            dot += "  \(from) -> \(to);\n"
        }

        dot += "}\n"
        return dot
    }
}

print("=== Testing Graph Visualization (DOT format) ===\n")

// Create a sample graph
let graph = TestGraph()
graph.addVertex(id: 0, label: "Alice")
graph.addVertex(id: 1, label: "Bob")
graph.addVertex(id: 2, label: "Charlie")
graph.addVertex(id: 3, label: "David")

graph.addEdge(from: 0, to: 1)
graph.addEdge(from: 1, to: 2)
graph.addEdge(from: 2, to: 0)
graph.addEdge(from: 2, to: 3)

// Export to DOT format
let dot = graph.exportToDOT()
print("DOT format output:")
print(dot)

// Save to file
let filePath = "/tmp/axolotl_tmp/Experiments/sample_graph.dot"
do {
    try dot.write(to: URL(fileURLWithPath: filePath), atomically: true, encoding: .utf8)
    print("✅ DOT file saved to: \(filePath)")
    print("\nTo render: dot -Tpng \(filePath) -o graph.png")
    print("Or upload to: https://dreampuf.github.io/GraphvizOnline/")
} catch {
    print("❌ Error saving file: \(error)")
}

print("\n=== Test Complete ===")
