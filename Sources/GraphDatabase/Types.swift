//
//  Types.swift
//  Core types for Axolotl Graph Database
//

import Foundation

// MARK: - Core Types

/// Vertex ID (64-bit unsigned integer)
public typealias VertexID = UInt64

/// Edge ID (pair of vertex IDs)
public typealias EdgeID = (from: VertexID, to: VertexID)

// MARK: - Data Models

/// Vertex representation
public struct Vertex: Codable, Equatable, Hashable {
    public let id: VertexID
    public var properties: [String: PropertyValue]
    
    public init(id: VertexID, properties: [String: PropertyValue] = [:]) {
        self.id = id
        self.properties = properties
    }
}

/// Edge representation
public struct Edge: Codable, Equatable, Hashable {
    public let from: VertexID
    public let to: VertexID
    public var properties: [String: PropertyValue]
    public var weight: Float
    
    public init(from: VertexID, to: VertexID, properties: [String: PropertyValue] = [:], weight: Float = 1.0) {
        self.from = from
        self.to = to
        self.properties = properties
        self.weight = weight
    }
    
    public var id: EdgeID {
        return (from, to)
    }
}

/// Property value (flexible type storage)
public enum PropertyValue: Codable, Equatable, Hashable {
    case int(Int)
    case double(Double)
    case string(String)
    case bool(Bool)
    case float(Float)
    case data(Data)
    
    // Codable implementation
    enum CodingKeys: String, CodingKey {
        case type, value
    }
    
    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .int(let value):
            try container.encode("int", forKey: .type)
            try container.encode(value, forKey: .value)
        case .double(let value):
            try container.encode("double", forKey: .type)
            try container.encode(value, forKey: .value)
        case .string(let value):
            try container.encode("string", forKey: .type)
            try container.encode(value, forKey: .value)
        case .bool(let value):
            try container.encode("bool", forKey: .type)
            try container.encode(value, forKey: .value)
        case .float(let value):
            try container.encode("float", forKey: .type)
            try container.encode(value, forKey: .value)
        case .data(let value):
            try container.encode("data", forKey: .type)
            try container.encode(value, forKey: .value)
        }
    }
    
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "int":
            self = .int(try container.decode(Int.self, forKey: .value))
        case "double":
            self = .double(try container.decode(Double.self, forKey: .value))
        case "string":
            self = .string(try container.decode(String.self, forKey: .value))
        case "bool":
            self = .bool(try container.decode(Bool.self, forKey: .value))
        case "float":
            self = .float(try container.decode(Float.self, forKey: .value))
        case "data":
            self = .data(try container.decode(Data.self, forKey: .value))
        default:
            throw DecodingError.dataCorrupted(DecodingError.Context(codingPath: [], debugDescription: "Unknown type: \(type)"))
        }
    }
}

// MARK: - Error Types

/// Graph database errors
public enum GraphError: Error, LocalizedError {
    case vertexNotFound(VertexID)
    case edgeNotFound(VertexID, VertexID)
    case vertexAlreadyExists(VertexID)
    case edgeAlreadyExists(VertexID, VertexID)
    case invalidOperation(String)
    
    public var errorDescription: String? {
        switch self {
        case .vertexNotFound(let id):
            return "Vertex \(id) not found"
        case .edgeNotFound(let from, let to):
            return "Edge (\(from), \(to)) not found"
        case .vertexAlreadyExists(let id):
            return "Vertex \(id) already exists"
        case .edgeAlreadyExists(let from, let to):
            return "Edge (\(from), \(to)) already exists"
        case .invalidOperation(let msg):
            return "Invalid operation: \(msg)"
        }
    }
}

// MARK: - Change Log (for Incremental Persistence)

/// Change record for incremental persistence
public enum Change: Codable {
    case vertexAdded(Vertex)
    case vertexDeleted(VertexID)
    case vertexUpdated(VertexID, [String: PropertyValue])
    case edgeAdded(Edge)
    case edgeDeleted(EdgeID)
    case edgeUpdated(EdgeID, [String: PropertyValue])
}

// MARK: - File Header (for Binary Format)

/// File header for AxolotlDB binary format
struct FileHeader {
    static let magicNumber: [UInt8] = [65, 120, 111, 108, 111, 116, 108, 0]  // "Axolotl\0"
    static let currentVersion: UInt32 = 1
    
    var magic: [UInt8] = magicNumber
    var version: UInt32 = currentVersion
    var vertexCount: UInt64 = 0
    var edgeCount: UInt64 = 0
    var indexOffset: UInt64 = 0
    var dataOffset: UInt64 = 0
    var reserved: [UInt8] = Array(repeating: 0, count: 32)  // For future use
    
    static let size = 64  // Header is always 64 bytes
}
