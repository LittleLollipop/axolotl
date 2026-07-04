import Foundation

// MARK: - Simplified Test (Uses PersistentGraph from Sources)

func testPropertyIndexPersistence() {
    print("=== Axolotl Property Index Test (Simplified) ===\n")
    
    let testDbPath = "/tmp/test_property_index.axolotl"
    
    // Clean up
    try? FileManager.default.removeItem(atPath: testDbPath)
    
    // Test: Create database, add vertices, save, reload, verify index
    print("Test: Property index persistence")
    print(String(repeating: "-", count: 50))
    
    do {
        // Step 1: Create database and add vertices
        print("\n1. Creating database and adding vertices...")
        
        let db = try PersistentGraph(filePath: testDbPath)
        
        _ = try db.addVertex(properties: ["name": .string("Alice"), "age": .int(30)])
        _ = try db.addVertex(properties: ["name": .string("Bob"), "age": .int(25)])
        _ = try db.addVertex(properties: ["name": .string("Alice"), "age": .int(28)]) // Duplicate name
        
        print("   Added 3 vertices")
        
        // Step 2: Verify index before save
        print("\n2. Verifying index before save...")
        
        let aliceBefore = db.findByProperty("name", value: .string("Alice"))
        let bobBefore = db.findByProperty("name", value: .string("Bob"))
        
        print("   Vertices named 'Alice' (before save): \(aliceBefore.count) (expected: 2)")
        print("   Vertices named 'Bob' (before save): \(bobBefore.count) (expected: 1)")
        
        // Step 3: Save to disk
        print("\n3. Saving to disk...")
        try db.save()
        print("   ✅ Saved")
        
        // Step 4: Reload from disk
        print("\n4. Reloading from disk...")
        let db2 = try PersistentGraph(filePath: testDbPath)
        print("   ✅ Reloaded")
        
        // Step 5: Verify index after reload
        print("\n5. Verifying index after reload...")
        
        let aliceAfter = db2.findByProperty("name", value: .string("Alice"))
        let bobAfter = db2.findByProperty("name", value: .string("Bob"))
        
        print("   Vertices named 'Alice' (after reload): \(aliceAfter.count) (expected: 2)")
        print("   Vertices named 'Bob' (after reload): \(bobAfter.count) (expected: 1)")
        
        if aliceAfter.count == 2 && bobAfter.count == 1 {
            print("\n✅ Property index persistence test PASSED\n")
        } else {
            print("\n❌ Property index persistence test FAILED\n")
        }
        
    } catch {
        print("❌ Test FAILED: \(error)")
    }
    
    // Clean up
    print("Cleaning up...")
    try? FileManager.default.removeItem(atPath: testDbPath)
    print("✅ Test file removed")
    
    print("\n=== Test completed ===")
}

// Run test
testPropertyIndexPersistence()
