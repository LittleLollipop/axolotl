import Foundation

// MARK: - Core Types

typealias VertexID = UInt64

struct EdgeID: Hashable, Codable {
    let from: VertexID
    let to: VertexID
}

enum PropertyValue: Codable, Equatable, Hashable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case null

    enum CodingKeys: String, CodingKey {
        case type, value
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)

        switch type {
        case "string":
            self = .string(try container.decode(String.self, forKey: .value))
        case "int":
            self = .int(try container.decode(Int.self, forKey: .value))
        case "double":
            self = .double(try container.decode(Double.self, forKey: .value))
        case "bool":
            self = .bool(try container.decode(Bool.self, forKey: .value))
        case "null":
            self = .null
        default:
            self = .null
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)

        switch self {
        case .string(let s):
            try container.encode("string", forKey: .type)
            try container.encode(s, forKey: .value)
        case .int(let i):
            try container.encode("int", forKey: .type)
            try container.encode(i, forKey: .value)
        case .double(let d):
            try container.encode("double", forKey: .type)
            try container.encode(d, forKey: .value)
        case .bool(let b):
            try container.encode("bool", forKey: .type)
            try container.encode(b, forKey: .value)
        case .null:
            try container.encode("null", forKey: .type)
            try container.encodeNil(forKey: .value)
        }
    }

    func toAny() -> Any {
        switch self {
        case .string(let s): return s
        case .int(let i): return i
        case .double(let d): return d
        case .bool(let b): return b
        case .null: return NSNull()
        }
    }

    static func fromAny(_ any: Any) -> PropertyValue {
        if let s = any as? String { return .string(s) }
        if let i = any as? Int { return .int(i) }
        if let d = any as? Double { return .double(d) }
        if let b = any as? Bool { return .bool(b) }
        if any is NSNull { return .null }
        return .null
    }
}

struct Vertex: Codable {
    let id: VertexID
    var properties: [String: PropertyValue]
}

struct Edge: Codable {
    let id: EdgeID
    var properties: [String: PropertyValue]
    var weight: Double
}

// MARK: - Binary Format Constants

struct BinaryFormat {
    static let magic: [UInt8] = [0x41, 0x58, 0x4F, 0x4C]  // "AXOL"
    static let version: UInt16 = 1
    static let headerSize = 40  // 4 + 2 + 8 + 8 + 16 = 40 bytes

    enum PropertyType: UInt8 {
        case string = 0
        case int = 1
        case double = 2
        case bool = 3
        case null = 4
    }
}

// MARK: - Binary Buffer Helper

class BinaryBuffer {
    var data: Data
    private var readOffset = 0

    init() {
        self.data = Data()
    }

    init(data: Data) {
        self.data = data
    }

    // Write methods

    func writeBytes(_ bytes: [UInt8]) {
        data.append(contentsOf: bytes)
    }

    func writeUInt16(_ value: UInt16) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: MemoryLayout<UInt16>.size))
    }

    func writeUInt32(_ value: UInt32) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: MemoryLayout<UInt32>.size))
    }

    func writeUInt64(_ value: UInt64) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: MemoryLayout<UInt64>.size))
    }

    func writeDouble(_ value: Double) {
        var value = value.bitPattern.bigEndian
        data.append(Data(bytes: &value, count: MemoryLayout<UInt64>.size))
    }

    func writeString(_ string: String) {
        let bytes = [UInt8](string.utf8)
        writeUInt32(UInt32(bytes.count))
        writeBytes(bytes)
    }

    func writePropertyValue(_ value: PropertyValue) {
        switch value {
        case .string(let s):
            writeBytes([BinaryFormat.PropertyType.string.rawValue])
            writeString(s)
        case .int(let i):
            writeBytes([BinaryFormat.PropertyType.int.rawValue])
            var intValue = Int64(i).bigEndian
            data.append(Data(bytes: &intValue, count: MemoryLayout<Int64>.size))
        case .double(let d):
            writeBytes([BinaryFormat.PropertyType.double.rawValue])
            writeDouble(d)
        case .bool(let b):
            writeBytes([BinaryFormat.PropertyType.bool.rawValue])
            writeBytes([b ? 1 : 0])
        case .null:
            writeBytes([BinaryFormat.PropertyType.null.rawValue])
        }
    }

    // Read methods

    func readBytes(_ count: Int) -> [UInt8] {
        let bytes = [UInt8](data[readOffset..<readOffset + count])
        readOffset += count
        return bytes
    }

    func readUInt16() -> UInt16 {
        let value = data.withUnsafeBytes { $0.load(fromByteOffset: readOffset, as: UInt16.self) }
        readOffset += MemoryLayout<UInt16>.size
        return UInt16(bigEndian: value)
    }

    func readUInt32() -> UInt32 {
        let value = data.withUnsafeBytes { $0.load(fromByteOffset: readOffset, as: UInt32.self) }
        readOffset += MemoryLayout<UInt32>.size
        return UInt32(bigEndian: value)
    }

    func readUInt64() -> UInt64 {
        let value = data.withUnsafeBytes { $0.load(fromByteOffset: readOffset, as: UInt64.self) }
        readOffset += MemoryLayout<UInt64>.size
        return UInt64(bigEndian: value)
    }

    func readDouble() -> Double {
        let bitPattern = readUInt64()
        return Double(bitPattern: bitPattern)
    }

    func readString() -> String {
        let length = readUInt32()
        let bytes = readBytes(Int(length))
        return String(bytes: bytes, encoding: .utf8) ?? ""
    }

    func readPropertyValue() -> PropertyValue {
        let typeRaw = readBytes(1)[0]
        guard let type = BinaryFormat.PropertyType(rawValue: typeRaw) else {
            return .null
        }

        switch type {
        case .string:
            return .string(readString())
        case .int:
            var value: Int64 = 0
            let bytes = readBytes(MemoryLayout<Int64>.size)
            value = bytes.withUnsafeBufferPointer {
                $0.baseAddress!.withMemoryRebound(to: Int64.self, capacity: 1) {
                    Int64(bigEndian: $0.pointee)
                }
            }
            return .int(Int(value))
        case .double:
            return .double(readDouble())
        case .bool:
            let b = readBytes(1)[0]
            return .bool(b == 1)
        case .null:
            return .null
        }
    }
}

// MARK: - Simple Graph for Testing

class SimpleGraph {
    var vertices: [VertexID: Vertex] = [:]
    var edges: [EdgeID: Edge] = [:]
    var adjacencyList: [VertexID: Set<VertexID>] = [:]

    func addVertex(id: VertexID, properties: [String: PropertyValue]) {
        let vertex = Vertex(id: id, properties: properties)
        vertices[id] = vertex
        adjacencyList[id] = []
    }

    func addEdge(from: VertexID, to: VertexID, properties: [String: PropertyValue], weight: Double) {
        let edgeId = EdgeID(from: from, to: to)
        let edge = Edge(id: edgeId, properties: properties, weight: weight)
        edges[edgeId] = edge
        adjacencyList[from]?.insert(to)
    }

    // MARK: - JSON Persistence

    func saveJSON(to filePath: String) throws {
        let verticesArray = vertices.values.map { vertex in
            return [
                "id": vertex.id,
                "properties": vertex.properties.mapValues { $0.toAny() }
            ] as [String: Any]
        }

        let edgesArray = edges.values.map { edge in
            return [
                "from": edge.id.from,
                "to": edge.id.to,
                "properties": edge.properties.mapValues { $0.toAny() },
                "weight": edge.weight
            ] as [String: Any]
        }

        let jsonObject: [String: Any] = [
            "version": "1.0",
            "vertexCount": vertices.count,
            "edgeCount": edges.count,
            "vertices": verticesArray,
            "edges": edgesArray
        ]

        let data = try JSONSerialization.data(withJSONObject: jsonObject, options: [.prettyPrinted])
        try data.write(to: URL(fileURLWithPath: filePath))
    }

    func loadJSON(from filePath: String) throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: filePath))
        guard let jsonObject = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let verticesArray = jsonObject["vertices"] as? [[String: Any]],
              let edgesArray = jsonObject["edges"] as? [[String: Any]] else {
            throw GraphError.invalidFormat
        }

        vertices.removeAll()
        edges.removeAll()
        adjacencyList.removeAll()

        for vertexDict in verticesArray {
            guard let id = vertexDict["id"] as? UInt64,
                  let propertiesDict = vertexDict["properties"] as? [String: Any] else {
                continue
            }

            let properties = propertiesDict.mapValues { PropertyValue.fromAny($0) }
            addVertex(id: id, properties: properties)
        }

        for edgeDict in edgesArray {
            guard let from = edgeDict["from"] as? UInt64,
                  let to = edgeDict["to"] as? UInt64,
                  let propertiesDict = edgeDict["properties"] as? [String: Any] else {
                continue
            }

            let properties = propertiesDict.mapValues { PropertyValue.fromAny($0) }
            let weight = edgeDict["weight"] as? Double ?? 1.0
            addEdge(from: from, to: to, properties: properties, weight: weight)
        }
    }

    // MARK: - Binary Persistence

    func saveBinary(to filePath: String) throws {
        let buffer = BinaryBuffer()

        // Write header
        buffer.writeBytes(BinaryFormat.magic)  // Magic: "AXOL"
        buffer.writeUInt16(BinaryFormat.version)  // Version
        buffer.writeUInt64(UInt64(vertices.count))  // Vertex count
        buffer.writeUInt64(UInt64(edges.count))  // Edge count
        buffer.writeBytes([UInt8](repeating: 0, count: 16))  // Reserved

        // Write vertices
        for (_, vertex) in vertices {
            buffer.writeUInt64(vertex.id)
            buffer.writeUInt32(UInt32(vertex.properties.count))

            for (key, value) in vertex.properties {
                buffer.writeString(key)
                buffer.writePropertyValue(value)
            }
        }

        // Write edges
        for (_, edge) in edges {
            buffer.writeUInt64(edge.id.from)
            buffer.writeUInt64(edge.id.to)
            buffer.writeDouble(edge.weight)
            buffer.writeUInt32(UInt32(edge.properties.count))

            for (key, value) in edge.properties {
                buffer.writeString(key)
                buffer.writePropertyValue(value)
            }
        }

        try buffer.data.write(to: URL(fileURLWithPath: filePath))
    }

    func loadBinary(from filePath: String) throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: filePath))
        let buffer = BinaryBuffer(data: data)

        // Read header
        let magic = buffer.readBytes(4)
        guard magic == BinaryFormat.magic else {
            throw GraphError.invalidFormat
        }

        let version = buffer.readUInt16()
        guard version == BinaryFormat.version else {
            throw GraphError.invalidFormat
        }

        let vertexCount = buffer.readUInt64()
        let edgeCount = buffer.readUInt64()
        buffer.readBytes(16)  // Skip reserved

        vertices.removeAll()
        edges.removeAll()
        adjacencyList.removeAll()

        // Read vertices
        for _ in 0..<vertexCount {
            let id = buffer.readUInt64()
            let propertyCount = buffer.readUInt32()

            var properties: [String: PropertyValue] = [:]
            for _ in 0..<propertyCount {
                let key = buffer.readString()
                let value = buffer.readPropertyValue()
                properties[key] = value
            }

            addVertex(id: id, properties: properties)
        }

        // Read edges
        for _ in 0..<edgeCount {
            let from = buffer.readUInt64()
            let to = buffer.readUInt64()
            let weight = buffer.readDouble()
            let propertyCount = buffer.readUInt32()

            var properties: [String: PropertyValue] = [:]
            for _ in 0..<propertyCount {
                let key = buffer.readString()
                let value = buffer.readPropertyValue()
                properties[key] = value
            }

            addEdge(from: from, to: to, properties: properties, weight: weight)
        }
    }

    // MARK: - Verification

    func isEqual(to other: SimpleGraph) -> Bool {
        // Check vertex count
        guard vertices.count == other.vertices.count else { return false }
        guard edges.count == other.edges.count else { return false }

        // Check vertices
        for (id, vertex) in vertices {
            guard let otherVertex = other.vertices[id] else { return false }
            guard vertex.properties == otherVertex.properties else { return false }
        }

        // Check edges
        for (id, edge) in edges {
            guard let otherEdge = other.edges[id] else { return false }
            guard edge.properties == otherEdge.properties else { return false }
            guard abs(edge.weight - otherEdge.weight) < 1e-10 else { return false }
        }

        return true
    }
}

enum GraphError: Error {
    case invalidFormat
}

// MARK: - Main Test

print("=== Binary Persistence Test ===\n")

// Create test graph
print("1. Creating test graph...")
let graph = SimpleGraph()

graph.addVertex(id: 1, properties: ["name": .string("Alice"), "age": .int(30), "score": .double(95.5), "active": .bool(true)])
graph.addVertex(id: 2, properties: ["name": .string("Bob"), "age": .int(25), "score": .double(88.0), "active": .bool(false)])
graph.addVertex(id: 3, properties: ["name": .string("Charlie"), "age": .int(35), "score": .double(92.3), "active": .bool(true)])
graph.addVertex(id: 4, properties: ["name": .string("Diana"), "age": .int(28), "score": .double(91.0), "active": .bool(true)])

graph.addEdge(from: 1, to: 2, properties: ["type": .string("knows"), "since": .int(2020)], weight: 1.0)
graph.addEdge(from: 2, to: 3, properties: ["type": .string("works_with")], weight: 0.8)
graph.addEdge(from: 3, to: 4, properties: ["type": .string("knows"), "since": .int(2019)], weight: 1.2)
graph.addEdge(from: 4, to: 1, properties: [:], weight: 0.5)

print("   Vertices: \(graph.vertices.count)")
print("   Edges: \(graph.edges.count)")

// Test JSON persistence
print("\n2. Testing JSON persistence...")
let jsonPath = "/tmp/test_graph.json"

let jsonSaveStart = CFAbsoluteTimeGetCurrent()
try graph.saveJSON(to: jsonPath)
let jsonSaveTime = (CFAbsoluteTimeGetCurrent() - jsonSaveStart) * 1000

let jsonLoadStart = CFAbsoluteTimeGetCurrent()
let jsonGraph = SimpleGraph()
try jsonGraph.loadJSON(from: jsonPath)
let jsonLoadTime = (CFAbsoluteTimeGetCurrent() - jsonLoadStart) * 1000

print("   Save time: \(String(format: "%.2f", jsonSaveTime)) ms")
print("   Load time: \(String(format: "%.2f", jsonLoadTime)) ms")

let jsonFileSize = try FileManager.default.attributesOfItem(atPath: jsonPath)[.size] as! UInt64
print("   File size: \(jsonFileSize) bytes")

// Test Binary persistence
print("\n3. Testing Binary persistence...")
let binaryPath = "/tmp/test_graph.bin"

let binarySaveStart = CFAbsoluteTimeGetCurrent()
try graph.saveBinary(to: binaryPath)
let binarySaveTime = (CFAbsoluteTimeGetCurrent() - binarySaveStart) * 1000

let binaryLoadStart = CFAbsoluteTimeGetCurrent()
let binaryGraph = SimpleGraph()
try binaryGraph.loadBinary(from: binaryPath)
let binaryLoadTime = (CFAbsoluteTimeGetCurrent() - binaryLoadStart) * 1000

print("   Save time: \(String(format: "%.2f", binarySaveTime)) ms")
print("   Load time: \(String(format: "%.2f", binaryLoadTime)) ms")

let binaryFileSize = try FileManager.default.attributesOfItem(atPath: binaryPath)[.size] as! UInt64
print("   File size: \(binaryFileSize) bytes")

// Verify correctness
print("\n4. Verifying correctness...")
let jsonMatch = graph.isEqual(to: jsonGraph)
let binaryMatch = graph.isEqual(to: binaryGraph)

print("   JSON load correct: \(jsonMatch ? "✅" : "❌")")
print("   Binary load correct: \(binaryMatch ? "✅" : "❌")")

// Performance comparison
print("\n5. Performance comparison:")
print("   Save speedup: \(String(format: "%.2f", jsonSaveTime / binarySaveTime))x")
print("   Load speedup: \(String(format: "%.2f", jsonLoadTime / binaryLoadTime))x")
print("   Size reduction: \(String(format: "%.2f", Double(jsonFileSize) / Double(binaryFileSize)))x")

// Cleanup
print("\n6. Cleaning up...")
try FileManager.default.removeItem(atPath: jsonPath)
try FileManager.default.removeItem(atPath: binaryPath)
print("   Done!")

print("\n=== Test Complete ===")
