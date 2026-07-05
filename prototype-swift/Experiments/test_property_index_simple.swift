import Foundation

print("=== Axolotl Graph Database - Property Index Test (Simplified) ===\n")

let testDbPath = "/tmp/test_graph_index_simple.axolotl"

// Create a fresh database
try? FileManager.default.removeItem(atPath: testDbPath)

print("1. Creating database and adding vertices...")
do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    _ = try db.addVertex(properties: ["name": .string("Alice"), "age": .int(30)])
    _ = try db.addVertex(properties: ["name": .string("Bob"), "age": .int(25)])
    _ = try db.addVertex(properties: ["name": .string("Alice"), "age": .int(28)]) // Duplicate name
    
    try db.save()
    print("   ✅ Added 3 vertices, saved to disk\n")
} catch {
    print("   ❌ Failed: \(error)")
    exit(1)
}

print("2. Testing property index (find by name)...")
do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    let aliceVertices = db.findByProperty("name", value: .string("Alice"))
    print("   Vertices named 'Alice': \(aliceVertices.count) (expected: 2)")
    
    if aliceVertices.count == 2 {
        print("   ✅ Property index works correctly\n")
    } else {
        print("   ❌ Property index failed\n")
    }
} catch {
    print("   ❌ Failed: \(error)")
}

print("3. Testing property index update...")
do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    // Find and update an Alice vertex
    let aliceVertices = db.findByProperty("name", value: .string("Alice"))
    
    if let firstAlice = aliceVertices.first {
        try db.updateVertex(id: firstAlice, properties: ["name": .string("Alicia")])
        try db.save() // Important: save changes!
        
        print("   Updated vertex \(firstAlice) (Alice -> Alicia), saved to disk")
    }
    
    // Verify
    let aliceAfter = db.findByProperty("name", value: .string("Alice"))
    let aliciaAfter = db.findByProperty("name", value: .string("Alicia"))
    
    print("   Vertices named 'Alice' after update: \(aliceAfter.count) (expected: 1)")
    print("   Vertices named 'Alicia' after update: \(aliciaAfter.count) (expected: 1)")
    
    if aliceAfter.count == 1 && aliciaAfter.count == 1 {
        print("   ✅ Property index update works correctly\n")
    } else {
        print("   ❌ Property index update failed\n")
    }
} catch {
    print("   ❌ Failed: \(error)")
}

print("4. Testing property index after reload...")
do {
    let db = try PersistentGraph(filePath: testDbPath)
    
    let aliceVertices = db.findByProperty("name", value: .string("Alice"))
    let aliciaVertices = db.findByProperty("name", value: .string("Alicia"))
    
    print("   Vertices named 'Alice' after reload: \(aliceVertices.count) (expected: 1)")
    print("   Vertices named 'Alicia' after reload: \(aliciaVertices.count) (expected: 1)")
    
    if aliceVertices.count == 1 && aliciaVertices.count == 1 {
        print("   ✅ Property index rebuilt correctly after reload\n")
    } else {
        print("   ❌ Property index not rebuilt correctly\n")
    }
} catch {
    print("   ❌ Failed: \(error)")
}

// Cleanup
print("Cleaning up...")
try? FileManager.default.removeItem(atPath: testDbPath)
print("✅ Test file removed")

print("\n=== All tests completed ===")
