import Foundation

// MARK: - Simple Binary Buffer Test

class BinaryBuffer {
    var data: Data
    private var readOffset = 0

    init() {
        self.data = Data()
    }

    init(data: Data) {
        self.data = data
    }

    // MARK: - Write Methods (Big-Endian)

    func writeUInt64(_ value: UInt64) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: 8))
    }

    func writeDouble(_ value: Double) {
        var bitPattern = value.bitPattern.bigEndian
        data.append(Data(bytes: &bitPattern, count: 8))
    }

    func writeInt64(_ value: Int64) {
        var value = value.bigEndian
        data.append(Data(bytes: &value, count: 8))
    }

    // MARK: - Read Methods (Big-Endian)

    func readUInt64() -> UInt64 {
        let bytes = [UInt8](data[readOffset..<readOffset + 8])
        readOffset += 8

        // Assume bytes are in big-endian order, convert to host order
        var value: UInt64 = 0
        for (i, byte) in bytes.enumerated() {
            value |= UInt64(byte) << UInt64(8 * (7 - i))
        }

        // value is now in host order (because we shifted according to big-endian)
        return value
    }

    func readDouble() -> Double {
        let bitPattern = readUInt64()
        // bitPattern is in host order, use directly
        return Double(bitPattern: bitPattern)
    }

    func readInt64() -> Int64 {
        let value = readUInt64()
        return Int64(bitPattern: value)
    }
}

// MARK: - Test

print("=== Simple Binary Test ===\n")

// Test 1: UInt64 round-trip
print("1. Testing UInt64 round-trip...")
let buffer1 = BinaryBuffer()
buffer1.writeUInt64(1)
buffer1.writeUInt64(255)
buffer1.writeUInt64(UInt64.max)

let buffer1read = BinaryBuffer(data: buffer1.data)
let v1 = buffer1read.readUInt64()
let v2 = buffer1read.readUInt64()
let v3 = buffer1read.readUInt64()

print("   Written: 1, 255, \(UInt64.max)")
print("   Read: \(v1), \(v2), \(v3)")
print("   Match: \(v1 == 1 && v2 == 255 && v3 == UInt64.max ? "✅" : "❌")")

// Test 2: Double round-trip
print("\n2. Testing Double round-trip...")
let buffer2 = BinaryBuffer()
buffer2.writeDouble(0.0)
buffer2.writeDouble(1.0)
buffer2.writeDouble(-1.0)
buffer2.writeDouble(95.5)
buffer2.writeDouble(Double.pi)

let buffer2read = BinaryBuffer(data: buffer2.data)
let d1 = buffer2read.readDouble()
let d2 = buffer2read.readDouble()
let d3 = buffer2read.readDouble()
let d4 = buffer2read.readDouble()
let d5 = buffer2read.readDouble()

print("   Written: 0.0, 1.0, -1.0, 95.5, pi")
print("   Read: \(d1), \(d2), \(d3), \(d4), \(d5)")
print("   Match: \(d1 == 0.0 && d2 == 1.0 && d3 == -1.0 && d4 == 95.5 && d5 == Double.pi ? "✅" : "❌")")

// Test 3: Int64 round-trip
print("\n3. Testing Int64 round-trip...")
let buffer3 = BinaryBuffer()
buffer3.writeInt64(0)
buffer3.writeInt64(1)
buffer3.writeInt64(-1)
buffer3.writeInt64(Int64.max)
buffer3.writeInt64(Int64.min)

let buffer3read = BinaryBuffer(data: buffer3.data)
let i1 = buffer3read.readInt64()
let i2 = buffer3read.readInt64()
let i3 = buffer3read.readInt64()
let i4 = buffer3read.readInt64()
let i5 = buffer3read.readInt64()

print("   Written: 0, 1, -1, \(Int64.max), \(Int64.min)")
print("   Read: \(i1), \(i2), \(i3), \(i4), \(i5)")
print("   Match: \(i1 == 0 && i2 == 1 && i3 == -1 && i4 == Int64.max && i5 == Int64.min ? "✅" : "❌")")

// Test 4: Verify byte pattern
print("\n4. Verifying byte pattern...")
let buffer4 = BinaryBuffer()
buffer4.writeUInt64(0x0102030405060708)

let bytes = [UInt8](buffer4.data)
print("   Written: 0x0102030405060708")
print("   Bytes: \(bytes.map { String(format: "%02X", $0) }.joined(separator: " "))")
print("   Expected (big-endian): 01 02 03 04 05 06 07 08")
print("   Match: \(bytes == [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08] ? "✅" : "❌")")

let buffer4read = BinaryBuffer(data: buffer4.data)
let v4 = buffer4read.readUInt64()
print("   Read back: 0x\(String(v4, radix: 16))")
print("   Match: \(v4 == 0x0102030405060708 ? "✅" : "❌")")

print("\n=== Test Complete ===")
