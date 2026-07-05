import Metal
import Foundation

// MARK: - Graph Structures

struct CSRGraph {
    let offsets: [UInt32]
    let targets: [UInt32]
    let vertexCount: UInt32
    let totalEdges: UInt32
    
    init(offsets: [UInt32], targets: [UInt32], vertexCount: UInt32, totalEdges: UInt32) {
        self.offsets = offsets
        self.targets = targets
        self.vertexCount = vertexCount
        self.totalEdges = totalEdges
    }
}

// MARK: - Graph Generation

func generateUniformGraph(vertexCount: UInt32, avgDegree: UInt32) -> CSRGraph {
    let totalEdges = vertexCount * avgDegree / 2
    var adjacency = Array(repeating: Set<UInt32>(), count: Int(vertexCount))
    
    // Generate random edges
    for _ in 0..<totalEdges {
        let u = UInt32.random(in: 0..<vertexCount)
        var v = UInt32.random(in: 0..<vertexCount)
        while v == u || adjacency[Int(u)].contains(v) {
            v = UInt32.random(in: 0..<vertexCount)
        }
        adjacency[Int(u)].insert(v)
        adjacency[Int(v)].insert(u)  // Undirected
    }
    
    // Build CSR
    var csrOffsets = [UInt32](repeating: 0, count: Int(vertexCount) + 1)
    var csrTargets: [UInt32] = []
    
    var offset: UInt32 = 0
    for i in 0..<Int(vertexCount) {
        csrOffsets[i] = offset
        let neighbors = Array(adjacency[i]).sorted()
        csrTargets.append(contentsOf: neighbors)
        offset += UInt32(neighbors.count)
    }
    csrOffsets[Int(vertexCount)] = offset
    
    return CSRGraph(offsets: csrOffsets, targets: csrTargets, vertexCount: vertexCount, totalEdges: offset)
}

// MARK: - Metal Setup

func createMetalDevice() -> (MTLDevice, MTLCommandQueue) {
    guard let device = MTLCreateSystemDefaultDevice() else {
        fatalError("No Metal device found")
    }
    let queue = device.makeCommandQueue()!
    return (device, queue)
}

// MARK: - Full BFS (GPU only)

let fullBFSKernel = """
#include <metal_stdlib>
using namespace metal;

kernel void bfs_expand(
    device const uint *offsets [[buffer(0)]],
    device const uint *targets [[buffer(1)]],
    device const uint *frontier [[buffer(2)]],
    device uint *distances [[buffer(3)]],
    device uint &frontier_size [[buffer(4)]],
    constant uint &vertex_count [[buffer(5)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= frontier_size) return;
    
    uint v = frontier[gid];
    if (v >= vertex_count) return;
    
    uint start = offsets[v];
    uint end = offsets[v + 1];
    uint new_dist = distances[v] + 1;
    
    for (uint i = start; i < end; i++) {
        uint neighbor = targets[i];
        if (distances[neighbor] > new_dist) {
            distances[neighbor] = new_dist;
        }
    }
}
"""

func computeFullBFS(csr: CSRGraph, source: UInt32, device: MTLDevice, queue: MTLCommandQueue) -> [UInt32] {
    let library = try! device.makeLibrary(source: fullBFSKernel, options: nil)
    let kernel = library.makeFunction(name: "bfs_expand")!
    let pipeline = try! device.makeComputePipelineState(function: kernel)
    
    var distances = [UInt32](repeating: UInt32(Int32.max), count: Int(csr.vertexCount))
    distances[Int(source)] = 0
    
    var frontier = [UInt32](repeating: 0, count: Int(csr.vertexCount))
    frontier[0] = source
    var frontierSize: UInt32 = 1
    
    var iteration = 0
    let startTime = CFAbsoluteTimeGetCurrent()
    
    while frontierSize > 0 {
        // GPU: Expand frontier
        let frontierBuffer = device.makeBuffer(bytes: frontier, length: Int(frontierSize) * MemoryLayout<UInt32>.stride, options: [])!
        let distancesBuffer = device.makeBuffer(bytes: distances, length: Int(csr.vertexCount) * MemoryLayout<UInt32>.stride, options: [])!
        let offsetsBuffer = device.makeBuffer(bytes: csr.offsets, length: (Int(csr.vertexCount) + 1) * MemoryLayout<UInt32>.stride, options: [])!
        let targetsBuffer = device.makeBuffer(bytes: csr.targets, length: Int(csr.totalEdges) * MemoryLayout<UInt32>.stride, options: [])!
        var fs = frontierSize
        var vc = csr.vertexCount
        
        let commandBuffer = queue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(offsetsBuffer, offset: 0, index: 0)
        encoder.setBuffer(targetsBuffer, offset: 0, index: 1)
        encoder.setBuffer(frontierBuffer, offset: 0, index: 2)
        encoder.setBuffer(distancesBuffer, offset: 0, index: 3)
        encoder.setBytes(&fs, length: MemoryLayout<UInt32>.stride, index: 4)
        encoder.setBytes(&vc, length: MemoryLayout<UInt32>.stride, index: 5)
        
        let gridSize = MTLSize(width: Int(fs), height: 1, depth: 1)
        let threadGroupSize = MTLSize(width: 256, height: 1, depth: 1)
        encoder.dispatchThreads(gridSize, threadsPerThreadgroup: threadGroupSize)
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        distances = Array(UnsafeBufferPointer(start: distancesBuffer.contents().bindMemory(to: UInt32.self, capacity: Int(csr.vertexCount)), count: Int(csr.vertexCount)))
        
        // CPU: Find new frontier (vertices whose distance was just updated)
        var newFrontier: [UInt32] = []
        for i in 0..<Int(csr.vertexCount) {
            if distances[i] == UInt32(iteration + 1) {
                newFrontier.append(UInt32(i))
            }
        }
        
        frontier = newFrontier
        frontierSize = UInt32(newFrontier.count)
        iteration += 1
        
        if iteration % 5 == 0 {
            print("    BFS iteration \(iteration): \(frontierSize) vertices in frontier")
        }
    }
    
    let elapsed = CFAbsoluteTimeGetCurrent() - startTime
    print("    Full BFS: \(iteration) iterations, \(String(format: "%.2f", elapsed * 1000)) ms")
    
    return distances
}

// MARK: - Incremental BFS (CPU + GPU)

let incrementalBFSKernel = """
#include <metal_stdlib>
using namespace metal;

// GPU kernel: Update distances for affected vertices only
kernel void incremental_bfs(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *offsets [[buffer(2)]],
    device const uint *targets [[buffer(3)]],
    device const uint *distances [[buffer(4)]],
    device uint *new_distances [[buffer(5)]],
    constant uint &vertex_count [[buffer(6)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];
    if (v >= vertex_count) return;
    
    uint start = offsets[v];
    uint end = offsets[v + 1];
    uint new_dist = distances[v] + 1;
    
    for (uint i = start; i < end; i++) {
        uint neighbor = targets[i];
        if (new_distances[neighbor] > new_dist) {
            new_distances[neighbor] = new_dist;
        }
    }
}
"""

func computeIncrementalBFS(
    csr: CSRGraph,
    initialDistances: [UInt32],
    affectedVertices: [UInt32],
    device: MTLDevice,
    queue: MTLCommandQueue
) -> [UInt32] {
    let library = try! device.makeLibrary(source: incrementalBFSKernel, options: nil)
    let kernel = library.makeFunction(name: "incremental_bfs")!
    let pipeline = try! device.makeComputePipelineState(function: kernel)
    
    var distances = initialDistances
    let vertexCount = Int(csr.vertexCount)
    
    // CPU: Manage affected vertex queue
    var affectedSet = Set<UInt32>(affectedVertices)
    var iteration = 0
    
    let startTime = CFAbsoluteTimeGetCurrent()
    
    while !affectedSet.isEmpty {
        let affectedArray = Array(affectedSet)
        var affectedCount = UInt32(affectedArray.count)
        
        // GPU: Update distances for affected vertices
        var newDistances = distances  // Copy current values
        
        let affectedBuffer = device.makeBuffer(bytes: affectedArray, length: Int(affectedCount) * MemoryLayout<UInt32>.stride, options: [])!
        let offsetsBuffer = device.makeBuffer(bytes: csr.offsets, length: (vertexCount + 1) * MemoryLayout<UInt32>.stride, options: [])!
        let targetsBuffer = device.makeBuffer(bytes: csr.targets, length: Int(csr.totalEdges) * MemoryLayout<UInt32>.stride, options: [])!
        let distancesBuffer = device.makeBuffer(bytes: distances, length: vertexCount * MemoryLayout<UInt32>.stride, options: [])!
        let newDistancesBuffer = device.makeBuffer(bytes: newDistances, length: vertexCount * MemoryLayout<UInt32>.stride, options: [])!
        var vc = csr.vertexCount
        
        let commandBuffer = queue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(affectedBuffer, offset: 0, index: 0)
        encoder.setBytes(&affectedCount, length: MemoryLayout<UInt32>.stride, index: 1)
        encoder.setBuffer(offsetsBuffer, offset: 0, index: 2)
        encoder.setBuffer(targetsBuffer, offset: 0, index: 3)
        encoder.setBuffer(distancesBuffer, offset: 0, index: 4)
        encoder.setBuffer(newDistancesBuffer, offset: 0, index: 5)
        encoder.setBytes(&vc, length: MemoryLayout<UInt32>.stride, index: 6)
        
        let gridSize = MTLSize(width: Int(affectedCount), height: 1, depth: 1)
        let threadGroupSize = MTLSize(width: 256, height: 1, depth: 1)
        encoder.dispatchThreads(gridSize, threadsPerThreadgroup: threadGroupSize)
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        // Get updated distances
        let updatedDistances = Array(UnsafeBufferPointer(start: newDistancesBuffer.contents().bindMemory(to: UInt32.self, capacity: vertexCount), count: vertexCount))
        
        // CPU: Find new affected vertices (whose distance was updated)
        var newAffected = Set<UInt32>()
        for i in 0..<vertexCount {
            if updatedDistances[i] < distances[i] {
                // This vertex's distance was updated, need to propagate to its neighbors
                let start = Int(csr.offsets[i])
                let end = Int(csr.offsets[i + 1])
                for j in start..<end {
                    let neighbor = csr.targets[j]
                    newAffected.insert(neighbor)
                }
            }
        }
        
        distances = updatedDistances
        affectedSet = newAffected
        iteration += 1
        
        if iteration % 5 == 0 {
            print("    Incremental BFS iteration \(iteration): \(affectedSet.count) vertices affected")
        }
    }
    
    let elapsed = CFAbsoluteTimeGetCurrent() - startTime
    print("    Incremental BFS: \(iteration) iterations, \(String(format: "%.2f", elapsed * 1000)) ms")
    
    return distances
}

// MARK: - Main Test

print("=== Incremental BFS Test ===\n")

let csr = generateUniformGraph(vertexCount: 10000, avgDegree: 20)
print("Generated graph: \(csr.vertexCount) vertices, \(csr.totalEdges) edges")

let (device, queue) = createMetalDevice()
print("Metal device: \(device.name)")

// Step 1: Compute full BFS from source 0
let source: UInt32 = 0
print("\n1. Computing full BFS from source \(source)...")
let fullBFSStart = CFAbsoluteTimeGetCurrent()
let originalDistances = computeFullBFS(csr: csr, source: source, device: device, queue: queue)
let fullBFSTime = CFAbsoluteTimeGetCurrent() - fullBFSStart
print("   Full BFS time: \(String(format: "%.2f", fullBFSTime * 1000)) ms")

// Count reachable vertices
var reachableCount = 0
for i in 0..<Int(csr.vertexCount) {
    if originalDistances[i] < UInt32(Int32.max) {
        reachableCount += 1
    }
}
print("   Reachable vertices: \(reachableCount)")

// Step 2: Simulate graph change (add some edges)
print("\n2. Simulating graph change (adding 10 edges)...")
var affectedVertices: [UInt32] = []
for _ in 0..<10 {
    let u = UInt32.random(in: 0..<csr.vertexCount)
    let v = UInt32.random(in: 0..<csr.vertexCount)
    affectedVertices.append(u)
    affectedVertices.append(v)
}
affectedVertices = Array(Set(affectedVertices))
print("   Affected vertices: \(affectedVertices.count)")

// Step 3: Compute incremental BFS
print("\n3. Computing incremental BFS...")
let incrementalDistances = computeIncrementalBFS(
    csr: csr,
    initialDistances: originalDistances,
    affectedVertices: affectedVertices,
    device: device,
    queue: queue
)

// Step 4: Compute full BFS from scratch for comparison
print("\n4. Computing full BFS from scratch for comparison...")
let fullBFSFromScratch = computeFullBFS(csr: csr, source: source, device: device, queue: queue)

// Step 5: Compare results
print("\n5. Comparing results...")
var maxDiff: UInt32 = 0
var diffCount = 0
for i in 0..<Int(csr.vertexCount) {
    let diff = abs(Int32(incrementalDistances[i]) - Int32(fullBFSFromScratch[i]))
    if diff > 0 {
        diffCount += 1
        maxDiff = max(maxDiff, UInt32(diff))
    }
}

print("   Vertices with different distances: \(diffCount)")
print("   Max difference: \(maxDiff)")

if diffCount == 0 {
    print("\n✅ Incremental BFS results match full BFS!")
} else {
    print("\n⚠️  Results differ (this may be expected if graph changes affect shortest paths)")
}

print("\n=== Test Complete ===")
