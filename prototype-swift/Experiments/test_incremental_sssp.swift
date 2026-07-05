import Foundation

// Test incremental SSSP using PersistentGraph

func testIncrementalSSSP() {
    print("🧪 Testing Incremental SSSP")
    print(String(repeating: "=", count: 50))

    do {
        let filePath = "/tmp/test_sssp_graph.json"
        let graph = try PersistentGraph(filePath: filePath)

        // Add vertices
        let v0 = try graph.addVertex(properties: ["name": .string("A")])
        let v1 = try graph.addVertex(properties: ["name": .string("B")])
        let v2 = try graph.addVertex(properties: ["name": .string("C")])
        let v3 = try graph.addVertex(properties: ["name": .string("D")])
        let v4 = try graph.addVertex(properties: ["name": .string("E")])

        // Add edges: A -> B -> C -> D -> E (long path)
        try graph.addEdge(from: v0, to: v1)
        try graph.addEdge(from: v1, to: v2)
        try graph.addEdge(from: v2, to: v3)
        try graph.addEdge(from: v3, to: v4)

        print("✅ Graph created with 5 vertices and 4 edges")

        // Compute SSSP from v0
        let sssp1 = graph.sssp(from: v0)
        print("\n📊 SSSP from v0 (before shortcut):")
        for (vid, dist) in sssp1.sorted(by: { $0.key < $1.key }) {
            print("   v\(vid): distance = \(dist)")
        }

        // Verify distances
        assert(sssp1[v0] == 0)
        assert(sssp1[v1] == 1)
        assert(sssp1[v2] == 2)
        assert(sssp1[v3] == 3)
        assert(sssp1[v4] == 4)
        print("✅ SSSP distances correct")

        // Add shortcut: A -> E (direct edge)
        try graph.addEdge(from: v0, to: v4)
        print("\n➕ Added shortcut: A -> E")

        // Recompute SSSP from v0 (should use cache)
        let sssp2 = graph.sssp(from: v0)
        print("\n📊 SSSP from v0 (after shortcut):")
        for (vid, dist) in sssp2.sorted(by: { $0.key < $1.key }) {
            print("   v\(vid): distance = \(dist)")
        }

        // Verify distances (v4 should now be distance 1)
        assert(sssp2[v0] == 0)
        assert(sssp2[v1] == 1)
        assert(sssp2[v2] == 2)
        assert(sssp2[v3] == 3)
        assert(sssp2[v4] == 1) // Shortcut!
        print("✅ SSSP distances correct (v4 is now distance 1)")

        // Test shortest path
        let path = graph.shortestPath(from: v0, to: v4)
        print("\n📊 Shortest path from v0 to v4:")
        print("   Path: \(path.map { "v\($0)" }.joined(separator: " -> "))")
        assert(path == [v0, v4])
        print("✅ Shortest path correct")

        print("\n✅ Incremental SSSP test passed!")

        // Cleanup
        try? FileManager.default.removeItem(atPath: filePath)

    } catch {
        print("❌ Error: \(error)")
    }
}

testIncrementalSSSP()
