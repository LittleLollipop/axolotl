import Foundation

// MARK: - Binary File Format (Simplified Version 1.0)

/*
File Structure:
===============
 
[Header] (64 bytes fixed size)
- Magic: "AXOL" (4 bytes)
- Version: 0x0100 (2 bytes major, 2 bytes minor)
- Vertex Count (8 bytes, UInt64)
- Edge Count (8 bytes, UInt64)
- Created At (8 bytes, timestamp)
- Reserved (34 bytes, for future use)
 
[Vertex Table] (16 bytes per vertex)
- Vertex ID (8 bytes, UInt64)
- Properties Offset (8 bytes, UInt64, 0 = no properties)
 
[Edge Table] (32 bytes per edge)
- Source ID (8 bytes, UInt64)
- Target ID (8 bytes, UInt64)
- Properties Offset (8 bytes, UInt64, 0 = no properties)
- Weight (8 bytes, Float64, default = 1.0)
 
[Properties Section] (Variable length)
- Null-terminated JSON strings
- Each vertex/edge's properties are stored as a JSON object
*/

struct BinaryGraphFormat {
    // MARK: - Constants
    static let magic = "AXOL".data(using: .ascii)!
    static let versionMajor: UInt16 = 1
    static let versionMinor: UInt16 = 0
    static let headerSize = 64
    static let vertexRecordSize = 16
    static let edgeRecordSize = 32
    
    // MARK: - Header
    struct Header {
        let magic: Data
        let versionMajor: UInt16
        let versionMinor: UInt16
        let vertexCount: UInt64
        let edgeCount: UInt64
        let createdAt: UInt64
        let reserved: [UInt8]
        
        init(vertexCount: UInt64, edgeCount: UInt64) {
            self.magic = BinaryGraphFormat.magic
            self.versionMajor = BinaryGraphFormat.versionMajor
            self.versionMinor = BinaryGraphFormat.versionMinor
            self.vertexCount = vertexCount
            self.edgeCount = edgeCount
            self.createdAt = UInt64(Date().timeIntervalSince1970)
            self.reserved = [UInt8](repeating: 0, count: 34)
        }
        
        init?(data: Data) {
            guard data.count >= BinaryGraphFormat.headerSize else { return nil }
            
            let magicData = data.prefix(4)
            guard magicData == BinaryGraphFormat.magic else { return nil }
            
            self.magic = magicData
            self.versionMajor = data.subdata(in: 4..<6).withUnsafeBytes { $0.load(as: UInt16.self).bigEndian }
            self.versionMinor = data.subdata(in: 6..<8).withUnsafeBytes { $0.load(as: UInt16.self).bigEndian }
            self.vertexCount = data.subdata(in: 8..<16).withUnsafeBytes { $0.load(as: UInt64.self).bigEndian }
            self.edgeCount = data.subdata(in: 16..<24).withUnsafeBytes { $0.load(as: UInt64.self).bigEndian }
            self.createdAt = data.subdata(in: 24..<32).withUnsafeBytes { $0.load(as: UInt64.self).bigEndian }
            self.reserved = [UInt8](data[32..<64])
        }
        
        func serialize() -> Data {
            var data = Data()
            data.append(magic)
            data.append(withUnsafeBytes(of: versionMajor.bigEndian) { Data($0) })
            data.append(withUnsafeBytes(of: versionMinor.bigEndian) { Data($0) })
            data.append(withUnsafeBytes(of: vertexCount.bigEndian) { Data($0) })
            data.append(withUnsafeBytes(of: edgeCount.bigEndian) { Data($0) })
            data.append(withUnsafeBytes(of: createdAt.bigEndian) { Data($0) })
            data.append(contentsOf: reserved)
            return data
        }
    }
    
    // MARK: - Save Graph to Binary File
    static func saveGraph(
        vertices: [VertexID: Vertex],
        edges: [EdgeID: Edge],
        to filePath: String
    ) throws {
        let vertexCount = UInt64(vertices.count)
        let edgeCount = UInt64(edges.count)
        
        // Build header
        var fileData = Header(vertexCount: vertexCount, edgeCount: edgeCount).serialize()
        
        // Pad to header size
        fileData.append(contentsOf: [UInt8](repeating: 0, count: headerSize - fileData.count))
        
        // Build vertex table
        var vertexTable = Data()
        var propertiesData = Data()
        var vertexPropertiesOffsets: [VertexID: UInt64] = [:]
        
        for (vid, vertex) in vertices.sorted(by: { $0.key < $1.key }) {
            // Vertex record (16 bytes)
            var record = Data()
            record.append(withUnsafeBytes(of: vid.bigEndian) { Data($0) })
            
            // Properties offset (will be filled later)
            let propertiesOffsetPlaceholder = vertexTable.count + 8
            record.append(contentsOf: [UInt8](repeating: 0, count: 8))
            
            vertexTable.append(record)
            
            // Serialize properties to JSON
            if !vertex.properties.isEmpty {
                let jsonData = try JSONSerialization.data(withJSONObject: vertex.properties.mapValues { $0.toAny() })
                vertexPropertiesOffsets[vid] = UInt64(propertiesData.count + vertexTable.count + headerSize)
                propertiesData.append(jsonData)
                propertiesData.append(0) // Null terminator
            }
        }
        
        // Update properties offsets in vertex table
        // (Simplified: In production, need to rebuild vertex table with correct offsets)
        
        // Build edge table
        var edgeTable = Data()
        var edgePropertiesOffsets: [EdgeID: UInt64] = [:]
        
        for (eid, edge) in edges.sorted(by: { $0.key.from < $1.key.from || ($0.key.from == $1.key.from && $0.key.to < $1.key.to) }) {
            // Edge record (32 bytes)
            var record = Data()
            record.append(withUnsafeBytes(of: eid.from.bigEndian) { Data($0) })
            record.append(withUnsafeBytes(of: eid.to.bigEndian) { Data($0) })
            
            // Properties offset (will be filled later)
            record.append(contentsOf: [UInt8](repeating: 0, count: 8))
            
            // Weight (8 bytes)
            var weight = edge.weight
            record.append(withUnsafeBytes(of: weight.bitPattern.bigEndian) { Data($0) })
            
            edgeTable.append(record)
            
            // Serialize properties to JSON
            if !edge.properties.isEmpty {
                let jsonData = try JSONSerialization.data(withJSONObject: edge.properties.mapValues { $0.toAny() })
                edgePropertiesOffsets[eid] = UInt64(propertiesData.count + edgeTable.count + vertexTable.count + headerSize)
                propertiesData.append(jsonData)
                propertiesData.append(0) // Null terminator
            }
        }
        
        // Assemble final file
        fileData.append(vertexTable)
        fileData.append(edgeTable)
        fileData.append(propertiesData)
        
        // Write to file
        try fileData.write(to: URL(fileURLWithPath: filePath))
        
        print("✅ Graph saved to \(filePath)")
        print("   Vertices: \(vertexCount)")
        print("   Edges: \(edgeCount)")
        print("   File size: \(fileData.count) bytes")
    }
    
    // MARK: - Load Graph from Binary File (Simplified - JSON version for now)
    static func loadGraph(from filePath: String) throws -> (vertices: [VertexID: Vertex], edges: [EdgeID: Edge]) {
        // For simplicity, use JSON format first
        // TODO: Implement true binary format loading
        
        let data = try Data(contentsOf: URL(fileURLWithPath: filePath))
        
        guard data.count >= headerSize else {
            throw GraphError.ioError("File too small to be a valid graph database")
        }
        
        // Parse header
        guard let header = Header(data: data) else {
            throw GraphError.ioError("Invalid file format (bad magic number or corrupted header)")
        }
        
        print("✅ Graph loaded from \(filePath)")
        print("   Version: \(header.versionMajor).\(header.versionMinor)")
        print("   Vertices: \(header.vertexCount)")
        print("   Edges: \(header.edgeCount)")
        print("   Created: \(Date(timeIntervalSince1970: TimeInterval(header.createdAt)))")
        
        // Simplified: Return empty graph for now
        // TODO: Implement full binary parsing
        return ([:], [:])
    }
}

// MARK: - Helper Extensions

extension PropertyValue {
    func toAny() -> Any {
        switch self {
        case .string(let s): return s
        case .int(let i): return i
        case .double(let d): return d
        case .bool(let b): return b
        case .null: return NSNull()
        }
    }
}

// MARK: - Graph Error

enum GraphError: Error, CustomStringConvertible {
    case vertexNotFound(VertexID)
    case edgeNotFound(VertexID, VertexID)
    case ioError(String)
    
    var description: String {
        switch self {
        case .vertexNotFound(let id):
            return "Vertex \(id) not found"
        case .edgeNotFound(let from, let to):
            return "Edge (\(from) -> \(to)) not found"
        case .ioError(let msg):
            return "I/O Error: \(msg)"
        }
    }
}
