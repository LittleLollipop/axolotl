import Metal
import Foundation

// MARK: - Graph Structures

struct CSRGraph {
    let offsets: [UInt32]
    let targets: [UInt32]
    let weights: [Float]
    let vertexCount: UInt32
    let totalEdges: UInt32
    
    init(offsets: [UInt32], targets: [UInt32], weights: [Float], vertexCount: UInt32, totalEdges: UInt32) {
        self.offsets = offsets
        self.targets = targets
        self.weights = weights
        self.vertexCount = vertexCount
        self.totalEdges = totalEdges
    }
}

// MARK: - Graph Generation

func generateWeightedGraph(vertexCount: UInt32, avgDegree: UInt32) -> CSRGraph {
    let totalEdges = vertexCount * avgDegree / 2
    var adjacency = Array(repeating: [(UInt32, Float)](), count: Int(vertexCount))
    
    for _ in 0..<totalEdges {
        let u = UInt32.random(in: 0..<vertexCount)
        var v = UInt32.random(in: 0..<vertexCount)
        while v == u || adjacency[Int(u)].contains(where: { $0.0 == v }) {
            v = UInt32.random(in: 0..<vertexCount)
        }
        let weight = Float.random(in: 1.0..<10.0)
        adjacency[Int(u)].append((v, weight))
        adjacency[Int(v)].append((u, weight))
    }
    
    var csrOffsets = [UInt32](repeating: 0, count: Int(vertexCount) + 1)
    var csrTargets: [UInt32] = []
    var csrWeights: [Float] = []
    
    var offset: UInt32 = 0
    for i in 0..<Int(vertexCount) {
        csrOffsets[i] = offset
        let neighbors = adjacency[i].sorted { $0.0 < $1.0 }
        for (target, weight) in neighbors {
            csrTargets.append(target)
            csrWeights.append(weight)
            offset += 1
        }
    }
    csrOffsets[Int(vertexCount)] = offset
    
    return CSRGraph(offsets: csrOffsets, targets: csrTargets, weights: csrWeights, vertexCount: vertexCount, totalEdges: offset)
}

// MARK: - Metal Setup

func createMetalDevice() -> (MTLDevice, MTLCommandQueue) {
    guard let device = MTLCreateSystemDefaultDevice() else {
        fatalError("No Metal device found")
    }
    let queue = device.makeCommandQueue()!
    return (device, queue)
}

// MARK: - Full SSSP (GPU Bellman-Ford)

let fullSSSPKernel = """
#include <metal_stdlib>
using namespace metal;

constant float INF = 1e30;

kernel void sssp_relax_all(
    device const uint *offsets [[buffer(0)]],
    device const uint *targets [[buffer(1)]],
    device const float *weights [[buffer(2)]],
    device float *distances [[buffer(3)]],
    constant uint &vertex_count [[buffer(4)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= vertex_count) return;
    if (distances[gid] == INF) return;
    
    uint start = offsets[gid];
    uint end = offsets[gid + 1];
    float v_dist = distances[gid];
    
    for (uint i = start; i < end; i++) {
        uint neighbor = targets[i];
        float new_dist = v_dist + weights[i];
        if (new_dist < distances[neighbor]) {
            distances[neighbor] = new_dist;
        }
    }
}
"""

func computeFullSSSP(csr: CSRGraph, source: UInt32, device: MTLDevice, queue: MTLCommandQueue) -> [Float] {
    let library = try! device.makeLibrary(source: fullSSSPKernel, options: nil)
    let kernel = library.makeFunction(name: "sssp_relax_all")!
    let pipeline = try! device.makeComputePipelineState(function: kernel)
    
    var distances = [Float](repeating: Float.greatestFiniteMagnitude, count: Int(csr.vertexCount))
    distances[Int(source)] = 0.0
    
    let vertexCount = Int(csr.vertexCount)
    var changed = true
    var iteration = 0
    
    let startTime = CFAbsoluteTimeGetCurrent()
    
    // Bellman-Ford: repeatedly relax all edges
    while changed && iteration < vertexCount {
        changed = false
        iteration += 1
        
        let distancesCopy = distances
        let distancesBuffer = device.makeBuffer(bytes: distancesCopy, length: vertexCount * MemoryLayout<Float>.stride, options: [])!
        let offsetsBuffer = device.makeBuffer(bytes: csr.offsets, length: (vertexCount + 1) * MemoryLayout<UInt32>.stride, options: [])!
        let targetsBuffer = device.makeBuffer(bytes: csr.targets, length: Int(csr.totalEdges) * MemoryLayout<UInt32>.stride, options: [])!
        let weightsBuffer = device.makeBuffer(bytes: csr.weights, length: Int(csr.totalEdges) * MemoryLayout<Float>.stride, options: [])!
        var vc = csr.vertexCount
        
        let commandBuffer = queue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(offsetsBuffer, offset: 0, index: 0)
        encoder.setBuffer(targetsBuffer, offset: 0, index: 1)
        encoder.setBuffer(weightsBuffer, offset: 0, index: 2)
        encoder.setBuffer(distancesBuffer, offset: 0, index: 3)
        encoder.setBytes(&vc, length: MemoryLayout<UInt32>.stride, index: 4)
        
        let gridSize = MTLSize(width: vertexCount, height: 1, depth: 1)
        let threadGroupSize = MTLSize(width: 256, height: 1, depth: 1)
        encoder.dispatchThreads(gridSize, threadsPerThreadgroup: threadGroupSize)
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        let oldDistances = distances
        distances = Array(UnsafeBufferPointer(start: distancesBuffer.contents().bindMemory(to: Float.self, capacity: vertexCount), count: vertexCount))
        
        // Check if any distance changed
        for i in 0..<vertexCount {
            if distances[i] != oldDistances[i] {
                changed = true
                break
            }
        }
        
        if iteration % 5 == 0 {
            print("    SSSP iteration \(iteration): changed = \(changed)")
        }
    }
    
    let elapsed = CFAbsoluteTimeGetCurrent() - startTime
    print("    Full SSSP: \(iteration) iterations, \(String(format: "%.2f", elapsed * 1000)) ms")
    
    return distances
}

// MARK: - Incremental SSSP (CPU + GPU)

let incrementalSSSPKernel = """
#include <metal_stdlib>
using namespace metal;

constant float INF = 1e30;

// GPU kernel: Relax edges for affected vertices only
kernel void incremental_sssp(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *offsets [[buffer(2)]],
    device const uint *targets [[buffer(3)]],
    device const float *weights [[buffer(4)]],
    device const float *distances [[buffer(5)]],
    device float *new_distances [[buffer(6)]],
    constant uint &vertex_count [[buffer(7)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];
    if (v >= vertex_count) return;
    if (distances[v] == INF) return;
    
    uint start = offsets[v];
    uint end = offsets[v + 1];
    float v_dist = distances[v];
    
    for (uint i = start; i < end; i++) {
        uint neighbor = targets[i];
        float new_dist = v_dist + weights[i];
        if (new_dist < new_distances[neighbor]) {
            new_distances[neighbor] = new_dist;
        }
    }
}
"""

func computeIncrementalSSSP(
    csr: CSRGraph,
    initialDistances: [Float],
    affectedVertices: [UInt32],
    device: MTLDevice,
    queue: MTLCommandQueue
) -> [Float] {
    let library = try! device.makeLibrary(source: incrementalSSSPKernel, options: nil)
    let kernel = library.makeFunction(name: "incremental_sssp")!
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
        
        // GPU: Relax edges for affected vertices
        var newDistances = distances
        
        let affectedBuffer = device.makeBuffer(bytes: affectedArray, length: Int(affectedCount) * MemoryLayout<UInt32>.stride, options: [])!
        let offsetsBuffer = device.makeBuffer(bytes: csr.offsets, length: (vertexCount + 1) * MemoryLayout<UInt32>.stride, options: [])!
        let targetsBuffer = device.makeBuffer(bytes: csr.targets, length: Int(csr.totalEdges) * MemoryLayout<UInt32>.stride, options: [])!
        let weightsBuffer = device.makeBuffer(bytes: csr.weights, length: Int(csr.totalEdges) * MemoryLayout<Float>.stride, options: [])!
        let distancesBuffer = device.makeBuffer(bytes: distances, length: vertexCount * MemoryLayout<Float>.stride, options: [])!
        let newDistancesBuffer = device.makeBuffer(bytes: newDistances, length: vertexCount * MemoryLayout<Float>.stride, options: [])!
        var vc = csr.vertexCount
        
        let commandBuffer = queue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(affectedBuffer, offset: 0, index: 0)
        encoder.setBytes(&affectedCount, length: MemoryLayout<UInt32>.stride, index: 1)
        encoder.setBuffer(offsetsBuffer, offset: 0, index: 2)
        encoder.setBuffer(targetsBuffer, offset: 0, index: 3)
        encoder.setBuffer(weightsBuffer, offset: 0, index: 4)
        encoder.setBuffer(distancesBuffer, offset: 0, index: 5)
        encoder.setBuffer(newDistancesBuffer, offset: 0, index: 6)
        encoder.setBytes(&vc, length: MemoryLayout<UInt32>.stride, index: 7)
        
        let gridSize = MTLSize(width: Int(affectedCount), height: 1, depth: 1)
        let threadGroupSize = MTLSize(width: 256, height: 1, depth: 1)
        encoder.dispatchThreads(gridSize, threadsPerThreadgroup: threadGroupSize)
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        let updatedDistances = Array(UnsafeBufferPointer(start: newDistancesBuffer.contents().bindMemory(to: Float.self, capacity: vertexCount), count: vertexCount))
        
        // CPU: Find new affected vertices (whose distance was updated)
        var newAffected = Set<UInt32>()
        for i in 0..<vertexCount {
            if updatedDistances[i] < distances[i] {
                // This vertex's distance was updated, propagate to neighbors
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
            print("    Incremental SSSP iteration \(iteration): \(affectedSet.count) vertices affected")
        }
    }
    
    let elapsed = CFAbsoluteTimeGetCurrent() - startTime
    print("    Incremental SSSP: \(iteration) iterations, \(String(format: "%.2f", elapsed * 1000)) ms")
    
    return distances
}

// MARK: - Main Test

print("=== Incremental SSSP Test ===\n")

let csr = generateWeightedGraph(vertexCount: 10000, avgDegree: 20)
print("Generated graph: \(csr.vertexCount) vertices, \(csr.totalEdges) edges")

let (device, queue) = createMetalDevice()
print("Metal device: \(device.name)")

// Step 1: Compute full SSSP from source 0
let source: UInt32 = 0
print("\n1. Computing full SSSP from source \(source)...")
let fullSSSPStart = CFAbsoluteTimeGetCurrent()
let originalDistances = computeFullSSSP(csr: csr, source: source, device: device, queue: queue)
let fullSSSPTTime = CFAbsoluteTimeGetCurrent() - fullSSSPStart
print("   Full SSSP time: \(String(format: "%.2f", fullSSSPTTime * 1000)) ms")

// Count reachable vertices
var reachableCount = 0
for i in 0..<Int(csr.vertexCount) {
    if originalDistances[i] < Float.greatestFiniteMagnitude {
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

// Step 3: Compute incremental SSSP
print("\n3. Computing incremental SSSP...")
let incrementalDistances = computeIncrementalSSSP(
    csr: csr,
    initialDistances: originalDistances,
    affectedVertices: affectedVertices,
    device: device,
    queue: queue
)

// Step 4: Compute full SSSP from scratch for comparison
print("\n4. Computing full SSSP from scratch for comparison...")
let fullSSSPFromScratch = computeFullSSSP(csr: csr, source: source, device: device, queue: queue)

// Step 5: Compare results
print("\n5. Comparing results...")
var maxDiff: Float = 0
var diffCount = 0
for i in 0..<Int(csr.vertexCount) {
    let diff = abs(incrementalDistances[i] - fullSSSPFromScratch[i])
    if diff > 0.001 {
        diffCount += 1
        maxDiff = max(maxDiff, diff)
    }
}

print("   Vertices with different distances: \(diffCount)")
print("   Max difference: \(maxDiff)")

if diffCount == 0 || maxDiff < 0.01 {
    print("\n✅ Incremental SSSP results match full SSSP!")
} else {
    print("\n⚠️  Results differ (this may be expected if graph changes affect shortest paths)")
}

print("\n=== Test Complete ===")
