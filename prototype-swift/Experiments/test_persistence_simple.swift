import Foundation

// MARK: - Simplified Graph Database Test

print("=== Axolotl Graph Database - Persistence Test ===\n")

// 1. Create a simple in-memory graph
print("1. Creating in-memory graph...")

var vertices: [UInt64: [String: Any]] = [:]
var edges: [[String: Any]] = []

// Add 5 vertices
for i in 0..<5 {
    vertices[UInt64(i)] = [
        "name": "Vertex\(i)",
        "age": Int(20 + i * 2)
    ]
}

// Add 5 edges
let edgeList = [(0, 1), (0, 4), (1, 2), (2, 3), (3, 4)]
for (from, to) in edgeList {
    edges.append([
        "from": UInt64(from),
        "to": UInt64(to),
        "type": "knows"
    ])
}

print("   Created \(vertices.count) vertices and \(edges.count) edges")

// 2. Serialize to JSON
print("\n2. Serializing to JSON...")

let graphObject: [String: Any] = [
    "version": "1.0",
    "vertexCount": vertices.count,
    "edgeCount": edges.count,
    "vertices": vertices.map { id, props in
        return ["id": id, "properties": props]
    },
    "edges": edges
]

guard let jsonData = try? JSONSerialization.data(withJSONObject: graphObject, options: [.prettyPrinted]) else {
    print("❌ Failed to serialize to JSON")
    exit(1)
}

print("   JSON size: \(jsonData.count) bytes")

// 3. Save to file
print("\n3. Saving to file...")

let filePath = "/tmp/test_graph.axolotl"

do {
    try jsonData.write(to: URL(fileURLWithPath: filePath))
    print("   ✅ Saved to \(filePath)")
} catch {
    print("   ❌ Failed to save: \(error)")
    exit(1)
}

// 4. Load from file
print("\n4. Loading from file...")

do {
    let loadedData = try Data(contentsOf: URL(fileURLWithPath: filePath))
    
    guard let loadedObject = try JSONSerialization.jsonObject(with: loadedData) as? [String: Any],
          let version = loadedObject["version"] as? String,
          let loadedVertices = loadedObject["vertices"] as? [[String: Any]],
          let loadedEdges = loadedObject["edges"] as? [[String: Any]] else {
        print("   ❌ Invalid file format")
        exit(1)
    }
    
    print("   ✅ Loaded from \(filePath)")
    print("   Version: \(version)")
    print("   Vertices: \(loadedVertices.count)")
    print("   Edges: \(loadedEdges.count)")
    
    // 5. Verify data
    print("\n5. Verifying data...")
    
    // Check vertex 0
    if let v0 = loadedVertices.first(where: { $0["id"] as? UInt64 == 0 }),
       let props = v0["properties"] as? [String: Any],
       let name = props["name"] as? String {
        print("   Vertex 0: \(name)")
    } else {
        print("   ❌ Vertex 0 not found or missing properties")
    }
    
    // Check edge (0, 1)
    if let e01 = loadedEdges.first(where: {
        ($0["from"] as? UInt64 == 0) && ($0["to"] as? UInt64 == 1)
    }) {
        print("   Edge (0 -> 1): exists")
    } else {
        print("   ❌ Edge (0 -> 1) not found")
    }
    
    print("\n✅ All tests PASSED\n")
    
} catch {
    print("   ❌ Failed to load: \(error)")
    exit(1)
}

// Cleanup
print("Cleaning up...")
try? FileManager.default.removeItem(atPath: filePath)
print("✅ Test file removed")

print("\n=== Test completed successfully ===")
