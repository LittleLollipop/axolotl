import Foundation

// MARK: - Debug Test

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

struct Vertex {
    let id: VertexID
    var properties: [String: PropertyValue]
}

struct Edge {
    let id: EdgeID
    var properties: [String: PropertyValue]
    var weight: Double
}

// MARK: - Binary Buffer (Fixed)

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
            writeBytes([0])  // type = 0
            writeString(s)
        case .int(let i):
            writeBytes([1])  // type = 1
            var intValue = Int64(i).bigEndian
            data.append(Data(bytes: &intValue, count: MemoryLayout<Int64>.size))
        case .double(let d):
            writeBytes([2])  // type = 2
            writeDouble(d)
        case .bool(let b):
            writeBytes([3])  // type = 3
            writeBytes([b ? 1 : 0])
        case .null:
            writeBytes([4])  // type = 4
        }
    }

    func readBytes(_ count: Int) -> [UInt8] {
        let bytes = [UInt8](data[readOffset..<readOffset + count])
        readOffset += count
        return bytes
    }

    func readUInt16() -> UInt16 {
        let bytes = readBytes(MemoryLayout<UInt16>.size)
        var value: UInt16 = 0
        for (i, byte) in bytes.enumerated() {
            value = value | (UInt16(byte) << UInt16(8 * (MemoryLayout<UInt16>.size - 1 - i)))
        }
        return value
    }

    func readUInt32() -> UInt32 {
        let bytes = readBytes(MemoryLayout<UInt32>.size)
        var value: UInt32 = 0
        for (i, byte) in bytes.enumerated() {
            value = value | (UInt32(byte) << UInt32(8 * (MemoryLayout<UInt32>.size - 1 - i)))
        }
        return value
    }

    func readUInt64() -> UInt64 {
        let bytes = readBytes(MemoryLayout<UInt64>.size)
        var value: UInt64 = 0
        for (i, byte) in bytes.enumerated() {
            value = value | (UInt64(byte) << UInt64(8 * (MemoryLayout<UInt64>.size - 1 - i)))
        }
        return value
    }

    func readDouble() -> Double {
        let bitPattern = readUInt64()
        return Double(bitPattern: UInt64(bigEndian: bitPattern))
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
            let bytes = readBytes(MemoryLayout<Int64>.size)
            var value: Int64 = 0
            for (i, byte) in bytes.enumerated() {
                value = value | (Int64(byte) << Int64(8 * (MemoryLayout<Int64>.size - 1 - i)))
            }
            return .int(Int(Int64(bigEndian: value)))
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

// MARK: - Main Debug Test

print("=== Debug Binary Persistence ===\n")

// Create a simple test
print("1. Creating simple test data...")
let buffer = BinaryBuffer()

// Write a double
let testDouble: Double = 95.5
print("   Writing double: \(testDouble)")
buffer.writeDouble(testDouble)

// Write an int
let testInt: Int = 30
print("   Writing int: \(testInt)")
var intValue = Int64(testInt).bigEndian
buffer.data.append(Data(bytes: &intValue, count: MemoryLayout<Int64>.size))

// Read back
print("\n2. Reading back...")
let buffer2 = BinaryBuffer(data: buffer.data)

let readDouble = buffer2.readDouble()
print("   Read double: \(readDouble)")
print("   Match: \(abs(readDouble - testDouble) < 1e-10 ? "✅" : "❌")")

// Reset offset
buffer2.readOffset = 8  // Skip the double

let readInt = buffer2.readPropertyValue()
print("   Read int: \(readInt)")
print("   Match: \(readInt == PropertyValue.int(testInt) ? "✅" : "❌")")

print("\n3. Testing round-trip...")
let buffer3 = BinaryBuffer()
buffer3.writePropertyValue(.double(95.5))
buffer3.writePropertyValue(.int(30))
buffer3.writePropertyValue(.string("Alice"))
buffer3.writePropertyValue(.bool(true))

let buffer4 = BinaryBuffer(data: buffer3.data)
let v1 = buffer4.readPropertyValue()
let v2 = buffer4.readPropertyValue()
let v3 = buffer4.readPropertyValue()
let v4 = buffer4.readPropertyValue()

print("   Written: .double(95.5), .int(30), .string(\"Alice\"), .bool(true)")
print("   Read: \(v1), \(v2), \(v3), \(v4)")

print("\n=== Debug Complete ===")
