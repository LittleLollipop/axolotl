import Foundation

// MARK: - Core Types (Simplified for performance test)

typealias VertexID = UInt64

struct EdgeID: Hashable {
    let from: VertexID
    let to: VertexID
}

enum PropertyValue: Equatable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case null
}

struct Vertex: Equatable {
    let id: VertexID
    var properties: [String: PropertyValue]
}

struct Edge: Equatable {
    let id: EdgeID
    var properties: [String: PropertyValue]
    var weight: Double
}

// MARK: - Binary Buffer (Optimized)

class BinaryBuffer {
    var data: Data
    private var readOffset = 0

    init() {
        self.data = Data()
    }

    init(data: Data) {
        self.data = data
    }

    func writeBytes(_ bytes: [UInt8]) {
        data.append(contentsOf: bytes)
    }

    func writeUInt64(_ value: UInt64) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: 8))
    }

    func writeDouble(_ value: Double) {
        var bitPattern = value.bitPattern.bigEndian
        data.append(Data(bytes: &bitPattern, count: 8))
    }

    func writeString(_ string: String) {
        let bytes = [UInt8](string.utf8)
        writeUInt32(UInt32(bytes.count))
        writeBytes(bytes)
    }

    func writeUInt32(_ value: UInt32) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: 4))
    }

    func writePropertyValue(_ value: PropertyValue) {
        switch value {
        case .string(let s):
            writeBytes([0])
            writeString(s)
        case .int(let i):
            writeBytes([1])
            var intValue = Int64(i).bigEndian
            data.append(Data(bytes: &intValue, count: 8))
        case .double(let d):
            writeBytes([2])
            writeDouble(d)
        case .bool(let b):
            writeBytes([3])
            writeBytes([b ? 1 : 0])
        case .null:
            writeBytes([4])
        }
    }

    func readBytes(_ count: Int) -> [UInt8] {
        let bytes = [UInt8](data[readOffset..<readOffset + count])
        readOffset += count
        return bytes
    }

    func readUInt64() -> UInt64 {
        let bytes = readBytes(8)
        var value: UInt64 = 0
        for (i, byte) in bytes.enumerated() {
            value |= UInt64(byte) << UInt64(8 * (7 - i))
        }
        return value
    }

    func readDouble() -> Double {
        let bitPattern = readUInt64()
        return Double(bitPattern: bitPattern)
    }

    func readUInt32() -> UInt32 {
        let bytes = readBytes(4)
        var value: UInt32 = 0
        for (i, byte) in bytes.enumerated() {
            value |= UInt32(byte) << UInt32(8 * (3 - i))
        }
        return value
    }

    func readString() -> String {
        let length = Int(readUInt32())
        let bytes = readBytes(length)
        return String(bytes: bytes, encoding: .utf8) ?? ""
    }

    func readPropertyValue() -> PropertyValue {
        let typeRaw = readBytes(1)[0]

        switch typeRaw {
        case 0:
            return .string(readString())
        case 1:
            let bytes = readBytes(8)
            var value: Int64 = 0
            for (i, byte) in bytes.enumerated() {
                value |= Int64(byte) << Int64(8 * (7 - i))
            }
            return .int(Int(value))
        case 2:
            return .double(readDouble())
        case 3:
            let b = readBytes(1)[0]
            return .bool(b == 1)
        case 4:
            return .null
        default:
            return .null
        }
    }
}

// MARK: - Graph with Both Persistence Formats

class Graph {
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
        var verticesArray: [[String: Any]] = []
        for (_, vertex) in vertices {
            var vertexDict: [String: Any] = [:]
            vertexDict["id"] = vertex.id
            var propertiesDict: [String: Any] = [:]
            for (key, value) in vertex.properties {
                switch value {
                case .string(let s): propertiesDict[key] = s
                case .int(let i): propertiesDict[key] = i
                case .double(let d): propertiesDict[key] = d
                case .bool(let b): propertiesDict[key] = b
                case .null: propertiesDict[key] = NSNull()
                }
            }
            vertexDict["properties"] = propertiesDict
            verticesArray.append(vertexDict)
        }

        var edgesArray: [[String: Any]] = []
        for (_, edge) in edges {
            var edgeDict: [String: Any] = [:]
            edgeDict["from"] = edge.id.from
            edgeDict["to"] = edge.id.to
            edgeDict["weight"] = edge.weight
            var propertiesDict: [String: Any] = [:]
            for (key, value) in edge.properties {
                switch value {
                case .string(let s): propertiesDict[key] = s
                case .int(let i): propertiesDict[key] = i
                case .double(let d): propertiesDict[key] = d
                case .bool(let b): propertiesDict[key] = b
                case .null: propertiesDict[key] = NSNull()
                }
            }
            edgeDict["properties"] = propertiesDict
            edgesArray.append(edgeDict)
        }

        let jsonObject: [String: Any] = [
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
            throw NSError(domain: "Invalid format", code: 1)
        }

        vertices.removeAll()
        edges.removeAll()
        adjacencyList.removeAll()

        for vertexDict in verticesArray {
            guard let id = vertexDict["id"] as? UInt64,
                  let propertiesDict = vertexDict["properties"] as? [String: Any] else {
                continue
            }

            var properties: [String: PropertyValue] = [:]
            for (key, value) in propertiesDict {
                if let s = value as? String { properties[key] = .string(s) }
                else if let i = value as? Int { properties[key] = .int(i) }
                else if let d = value as? Double { properties[key] = .double(d) }
                else if let b = value as? Bool { properties[key] = .bool(b) }
                else if value is NSNull { properties[key] = .null }
            }

            addVertex(id: id, properties: properties)
        }

        for edgeDict in edgesArray {
            guard let from = edgeDict["from"] as? UInt64,
                  let to = edgeDict["to"] as? UInt64,
                  let propertiesDict = edgeDict["properties"] as? [String: Any] else {
                continue
            }

            let weight = edgeDict["weight"] as? Double ?? 1.0
            var properties: [String: PropertyValue] = [:]
            for (key, value) in propertiesDict {
                if let s = value as? String { properties[key] = .string(s) }
                else if let i = value as? Int { properties[key] = .int(i) }
                else if let d = value as? Double { properties[key] = .double(d) }
                else if let b = value as? Bool { properties[key] = .bool(b) }
                else if value is NSNull { properties[key] = .null }
            }

            addEdge(from: from, to: to, properties: properties, weight: weight)
        }
    }

    // MARK: - Binary Persistence

    func saveBinary(to filePath: String) throws {
        let buffer = BinaryBuffer()

        // Header
        buffer.writeBytes([0x41, 0x58, 0x4F, 0x4C])  // "AXOL"
        buffer.writeUInt64(UInt64(vertices.count))
        buffer.writeUInt64(UInt64(edges.count))

        // Vertices
        for (_, vertex) in vertices {
            buffer.writeUInt64(vertex.id)
            buffer.writeUInt32(UInt32(vertex.properties.count))
            for (key, value) in vertex.properties {
                buffer.writeString(key)
                buffer.writePropertyValue(value)
            }
        }

        // Edges
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

        // Header
        let magic = buffer.readBytes(4)
        guard magic == [0x41, 0x58, 0x4F, 0x4C] else {
            throw NSError(domain: "Invalid format", code: 1)
        }

        let vertexCount = Int(buffer.readUInt64())
        let edgeCount = Int(buffer.readUInt64())

        vertices.removeAll()
        edges.removeAll()
        adjacencyList.removeAll()

        // Vertices
        for _ in 0..<vertexCount {
            let id = buffer.readUInt64()
            let propertyCount = Int(buffer.readUInt32())

            var properties: [String: PropertyValue] = [:]
            for _ in 0..<propertyCount {
                let key = buffer.readString()
                let value = buffer.readPropertyValue()
                properties[key] = value
            }

            addVertex(id: id, properties: properties)
        }

        // Edges
        for _ in 0..<edgeCount {
            let from = buffer.readUInt64()
            let to = buffer.readUInt64()
            let weight = buffer.readDouble()
            let propertyCount = Int(buffer.readUInt32())

            var properties: [String: PropertyValue] = [:]
            for _ in 0..<propertyCount {
                let key = buffer.readString()
                let value = buffer.readPropertyValue()
                properties[key] = value
            }

            addEdge(from: from, to: to, properties: properties, weight: weight)
        }
    }
}

// MARK: - Main Performance Test

print("=== Performance Test: JSON vs Binary ===\n")

// Generate test graph with different sizes
let testSizes = [100, 500, 1000]

for size in testSizes {
    print("📊 Testing with \(size) vertices...\n")

    // Create graph
    let graph = Graph()

    for i in 1...size {
        let properties: [String: PropertyValue] = [
            "name": .string("User\(i)"),
            "age": .int(Int.random(in: 18...80)),
            "score": .double(Double.random(in: 0...100)),
            "active": .bool(i % 2 == 0)
        ]
        graph.addVertex(id: UInt64(i), properties: properties)
    }

    // Add random edges
    let edgeCount = size * 2
    for _ in 0..<edgeCount {
        let from = UInt64(Int.random(in: 1...size))
        let to = UInt64(Int.random(in: 1...size))
        if from != to {
            let properties: [String: PropertyValue] = [
                "type": .string("knows"),
                "since": .int(Int.random(in: 2000...2024))
            ]
            graph.addEdge(from: from, to: to, properties: properties, weight: 1.0)
        }
    }

    print("   Generated graph with \(graph.vertices.count) vertices and \(graph.edges.count) edges")

    // Test JSON
    let jsonPath = "/tmp/perf_test_\(size).json"

    let jsonSaveStart = CFAbsoluteTimeGetCurrent()
    try graph.saveJSON(to: jsonPath)
    let jsonSaveTime = (CFAbsoluteTimeGetCurrent() - jsonSaveStart) * 1000

    let jsonLoadStart = CFAbsoluteTimeGetCurrent()
    let jsonGraph = Graph()
    try jsonGraph.loadJSON(from: jsonPath)
    let jsonLoadTime = (CFAbsoluteTimeGetCurrent() - jsonLoadStart) * 1000

    let jsonFileSize = try FileManager.default.attributesOfItem(atPath: jsonPath)[.size] as! UInt64

    print("\n   JSON Format:")
    print("     Save time: \(String(format: "%.2f", jsonSaveTime)) ms")
    print("     Load time: \(String(format: "%.2f", jsonLoadTime)) ms")
    print("     File size: \(jsonFileSize) bytes")

    // Test Binary
    let binaryPath = "/tmp/perf_test_\(size).bin"

    let binarySaveStart = CFAbsoluteTimeGetCurrent()
    try graph.saveBinary(to: binaryPath)
    let binarySaveTime = (CFAbsoluteTimeGetCurrent() - binarySaveStart) * 1000

    let binaryLoadStart = CFAbsoluteTimeGetCurrent()
    let binaryGraph = Graph()
    try binaryGraph.loadBinary(from: binaryPath)
    let binaryLoadTime = (CFAbsoluteTimeGetCurrent() - binaryLoadStart) * 1000

    let binaryFileSize = try FileManager.default.attributesOfItem(atPath: binaryPath)[.size] as! UInt64

    print("\n   Binary Format:")
    print("     Save time: \(String(format: "%.2f", binarySaveTime)) ms")
    print("     Load time: \(String(format: "%.2f", binaryLoadTime)) ms")
    print("     File size: \(binaryFileSize) bytes")

    // Performance comparison
    print("\n   Performance Comparison:")
    print("     Save speedup: \(String(format: "%.2f", jsonSaveTime / binarySaveTime))x")
    print("     Load speedup: \(String(format: "%.2f", jsonLoadTime / binaryLoadTime))x")
    print("     Size reduction: \(String(format: "%.2f", Double(jsonFileSize) / Double(binaryFileSize)))x")

    // Verify correctness
    let verticesMatch = graph.vertices == binaryGraph.vertices
    let edgesMatch = graph.edges == binaryGraph.edges
    print("\n   Correctness: \(verticesMatch && edgesMatch ? "✅" : "❌")")

    // Cleanup
    try FileManager.default.removeItem(atPath: jsonPath)
    try FileManager.default.removeItem(atPath: binaryPath)

    print("\n" + String(repeating: "-", count: 50) + "\n")
}

print("=== Performance Test Complete ===")
