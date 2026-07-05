import Foundation

// MARK: - Core Types (with Equatable)

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

// MARK: - Binary Buffer (Simplified)

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

// MARK: - Graph with Binary Persistence

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

    func printDebug() {
        print("   Vertices:")
        for (id, vertex) in vertices {
            print("     \(id): \(vertex.properties)")
        }
        print("   Edges:")
        for (id, edge) in edges {
            print("     \(id.from)->\(id.to): weight=\(edge.weight), properties=\(edge.properties)")
        }
    }
}

// MARK: - Main Test

print("=== Debug Binary Persistence ===\n")

// Create test graph
print("1. Creating test graph...")
let graph = Graph()
graph.addVertex(id: 1, properties: ["name": .string("Alice"), "age": .int(30), "score": .double(95.5), "active": .bool(true)])
graph.addVertex(id: 2, properties: ["name": .string("Bob"), "age": .int(25)])
graph.addEdge(from: 1, to: 2, properties: ["type": .string("knows")], weight: 1.0)

print("   Original graph:")
graph.printDebug()

// Save to binary
print("\n2. Saving to binary...")
let binaryPath = "/tmp/test_debug.bin"
try graph.saveBinary(to: binaryPath)
print("   Saved to \(binaryPath)")

// Load from binary
print("\n3. Loading from binary...")
let loadedGraph = Graph()
try loadedGraph.loadBinary(from: binaryPath)
print("   Loaded graph:")
loadedGraph.printDebug()

// Compare
print("\n4. Comparing...")
let verticesMatch = graph.vertices == loadedGraph.vertices
let edgesMatch = graph.edges == loadedGraph.edges

print("   Vertices match: \(verticesMatch ? "✅" : "❌")")
print("   Edges match: \(edgesMatch ? "✅" : "❌")")

if !verticesMatch {
    print("\n   Vertex differences:")
    for (id, vertex) in graph.vertices {
        if let loadedVertex = loadedGraph.vertices[id] {
            if vertex != loadedVertex {
                print("     Vertex \(id):")
                print("       Original: \(vertex.properties)")
                print("       Loaded: \(loadedVertex.properties)")
            }
        } else {
            print("     Vertex \(id) not found in loaded graph")
        }
    }
}

if !edgesMatch {
    print("\n   Edge differences:")
    for (id, edge) in graph.edges {
        if let loadedEdge = loadedGraph.edges[id] {
            if edge != loadedEdge {
                print("     Edge \(id.from)->\(id.to):")
                print("       Original: weight=\(edge.weight), properties=\(edge.properties)")
                print("       Loaded: weight=\(loadedEdge.weight), properties=\(loadedEdge.properties)")
            }
        } else {
            print("     Edge \(id.from)->\(id.to) not found in loaded graph")
        }
    }
}

// Cleanup
print("\n5. Cleaning up...")
try FileManager.default.removeItem(atPath: binaryPath)
print("   Done!")

print("\n=== Test Complete ===")
