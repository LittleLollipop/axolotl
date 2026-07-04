import Metal
import Foundation
import Accelerate

// MARK: - Graph Structures

struct EdgeBlock {
    let edges: [UInt32]
    let targets: [UInt32]
    let blockStart: [UInt32]
    let blockCount: [UInt32]
    let vertexCount: UInt32
    let totalEdges: UInt32
    
    init(edges: [UInt32], targets: [UInt32], blockStart: [UInt32], blockCount: [UInt32], 
         vertexCount: UInt32, totalEdges: UInt32) {
        self.edges = edges
        self.targets = targets
        self.blockStart = blockStart
        self.blockCount = blockCount
        self.vertexCount = vertexCount
        self.totalEdges = totalEdges
    }
}

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

func generatePowerLawGraph(vertexCount: UInt32, edgeCount: UInt32) -> (EdgeBlock, CSRGraph) {
    var edges: [UInt32] = []
    var adjacency = Array(repeating: Set<UInt32>(), count: Int(vertexCount))
    
    var actualEdges: UInt32 = 0
    let maxAttempts = edgeCount * 10
    var attempts = 0
    
    while actualEdges < edgeCount && attempts < maxAttempts {
        attempts += 1
        let u = UInt32.random(in: 0..<vertexCount)
        var v = UInt32.random(in: 0..<vertexCount)
        while v == u {
            v = UInt32.random(in: 0..<vertexCount)
        }
        
        if !adjacency[Int(u)].contains(v) {
            adjacency[Int(u)].insert(v)
            edges.append(u)
            edges.append(v)
            actualEdges += 1
        }
    }
    
    let finalEdges = actualEdges
    var sortedAdjacency = Array(repeating: [UInt32](), count: Int(vertexCount))
    for i in 0..<Int(vertexCount) {
        sortedAdjacency[i] = Array(adjacency[i]).sorted()
    }
    
    // Build EdgeBlock
    let blockSize = 32
    var blockEdges: [UInt32] = []
    var blockTargets: [UInt32] = []
    var blockStart = [UInt32](repeating: 0, count: Int(vertexCount) + 1)
    var blockCount = [UInt32](repeating: 0, count: Int(vertexCount))
    
    for i in 0..<Int(vertexCount) {
        let neighbors = sortedAdjacency[i]
        if neighbors.isEmpty { continue }
        
        blockStart[i] = UInt32(blockEdges.count / blockSize)
        let blocksNeeded = (neighbors.count + blockSize - 1) / blockSize
        
        for blockIdx in 0..<blocksNeeded {
            let start = blockIdx * blockSize
            let end = min(start + blockSize, neighbors.count)
            
            for j in 0..<blockSize {
                if j < end {
                    blockEdges.append(UInt32(i))
                    blockTargets.append(neighbors[start + j])
                } else {
                    blockEdges.append(0)
                    blockTargets.append(0)
                }
            }
        }
        
        blockCount[i] = UInt32(blocksNeeded)
    }
    
    blockStart[Int(vertexCount)] = UInt32(blockEdges.count / blockSize)
    
    let edgeBlock = EdgeBlock(
        edges: blockEdges, targets: blockTargets,
        blockStart: blockStart, blockCount: blockCount,
        vertexCount: vertexCount, totalEdges: finalEdges
    )
    
    // Build CSR
    var csrOffsets = [UInt32](repeating: 0, count: Int(vertexCount) + 1)
    var csrTargets: [UInt32] = []
    
    var offset: UInt32 = 0
    for i in 0..<Int(vertexCount) {
        csrOffsets[i] = offset
        csrTargets.append(contentsOf: sortedAdjacency[i])
        offset += UInt32(sortedAdjacency[i].count)
    }
    csrOffsets[Int(vertexCount)] = offset
    
    let csr = CSRGraph(offsets: csrOffsets, targets: csrTargets, vertexCount: vertexCount, totalEdges: finalEdges)
    
    return (edgeBlock, csr)
}

// MARK: - Metal Setup

func createMetalDevice() -> (MTLDevice, MTLCommandQueue) {
    guard let device = MTLCreateSystemDefaultDevice() else {
        fatalError("No Metal device found")
    }
    let queue = device.makeCommandQueue()!
    return (device, queue)
}

// MARK: - Full PageRank (GPU only)

let fullPageRankKernel = """
#include <metal_stdlib>
using namespace metal;

inline void atomic_add_float(device float* address, float val) {
    device atomic_uint* atomicAddr = (device atomic_uint*)address;
    uint oldBits = atomic_load_explicit(atomicAddr, memory_order_relaxed);
    uint newBits;
    do {
        float oldVal = as_type<float>(oldBits);
        float newVal = oldVal + val;
        newBits = as_type<uint>(newVal);
    } while (!atomic_compare_exchange_weak_explicit(
        atomicAddr, &oldBits, newBits,
        memory_order_relaxed, memory_order_relaxed));
}

kernel void pagerank_csr(
    device const uint *offsets [[buffer(0)]],
    device const uint *targets [[buffer(1)]],
    device const float *pr [[buffer(2)]],
    device float *new_pr [[buffer(3)]],
    constant uint &vertex_count [[buffer(4)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= vertex_count) return;
    
    float contribution = 0.0f;
    uint start = offsets[gid];
    uint end = offsets[gid + 1];
    
    for (uint i = start; i < end; i++) {
        uint neighbor = targets[i];
        uint neighbor_start = offsets[neighbor];
        uint neighbor_end = offsets[neighbor + 1];
        uint out_degree = neighbor_end - neighbor_start;
        if (out_degree > 0) {
            contribution += pr[neighbor] / float(out_degree);
        }
    }
    
    float damping = 0.85f;
    float new_value = (1.0f - damping) / float(vertex_count) + damping * contribution;
    atomic_add_float(&new_pr[gid], new_value);
}
"""

func computeFullPageRank(csr: CSRGraph, iterations: Int, device: MTLDevice, queue: MTLCommandQueue) -> [Float] {
    let library = try! device.makeLibrary(source: fullPageRankKernel, options: nil)
    let kernel = library.makeFunction(name: "pagerank_csr")!
    let pipeline = try! device.makeComputePipelineState(function: kernel)
    
    var pr = [Float](repeating: 1.0 / Float(csr.vertexCount), count: Int(csr.vertexCount))
    
    for iter in 0..<iterations {
        let newPr = [Float](repeating: 0.0, count: Int(csr.vertexCount))
        
        let offsetsBuffer = device.makeBuffer(bytes: csr.offsets, length: (Int(csr.vertexCount) + 1) * MemoryLayout<UInt32>.stride, options: [])!
        let targetsBuffer = device.makeBuffer(bytes: csr.targets, length: Int(csr.totalEdges) * MemoryLayout<UInt32>.stride, options: [])!
        let prBuffer = device.makeBuffer(bytes: pr, length: Int(csr.vertexCount) * MemoryLayout<Float>.stride, options: [])!
        let newPrBuffer = device.makeBuffer(bytes: newPr, length: Int(csr.vertexCount) * MemoryLayout<Float>.stride, options: [])!
        var vertexCount = csr.vertexCount
        
        let commandBuffer = queue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(offsetsBuffer, offset: 0, index: 0)
        encoder.setBuffer(targetsBuffer, offset: 0, index: 1)
        encoder.setBuffer(prBuffer, offset: 0, index: 2)
        encoder.setBuffer(newPrBuffer, offset: 0, index: 3)
        encoder.setBytes(&vertexCount, length: MemoryLayout<UInt32>.stride, index: 4)
        
        let gridSize = MTLSize(width: Int(vertexCount), height: 1, depth: 1)
        let threadGroupSize = MTLSize(width: 256, height: 1, depth: 1)
        encoder.dispatchThreads(gridSize, threadsPerThreadgroup: threadGroupSize)
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        pr = Array(UnsafeBufferPointer(start: newPrBuffer.contents().bindMemory(to: Float.self, capacity: Int(csr.vertexCount)), count: Int(csr.vertexCount)))
        
        if iter % 5 == 0 {
            print("    Iteration \(iter): PR sum = \(pr.reduce(0, +))")
        }
    }
    
    return pr
}

// MARK: - Incremental PageRank (CPU + GPU)

let incrementalPageRankKernel = """
#include <metal_stdlib>
using namespace metal;

// GPU kernel: Update PageRank for affected vertices only
kernel void incremental_pagerank(
    device const uint *affected_vertices [[buffer(0)]],
    device const uint &affected_count [[buffer(1)]],
    device const uint *offsets [[buffer(2)]],
    device const uint *targets [[buffer(3)]],
    device const float *pr [[buffer(4)]],
    device float *new_pr [[buffer(5)]],
    constant uint &vertex_count [[buffer(6)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= affected_count) return;
    
    uint v = affected_vertices[gid];
    if (v >= vertex_count) return;
    
    float contribution = 0.0f;
    uint start = offsets[v];
    uint end = offsets[v + 1];
    
    for (uint i = start; i < end; i++) {
        uint neighbor = targets[i];
        uint neighbor_start = offsets[neighbor];
        uint neighbor_end = offsets[neighbor + 1];
        uint out_degree = neighbor_end - neighbor_start;
        if (out_degree > 0) {
            contribution += pr[neighbor] / float(out_degree);
        }
    }
    
    float damping = 0.85f;
    float new_value = (1.0f - damping) / float(vertex_count) + damping * contribution;
    
    // Update this vertex's score
    new_pr[v] = new_value;
}
"""

// CPU+GPU Incremental PageRank
func computeIncrementalPageRank(
    csr: CSRGraph,
    initialPR: [Float],
    affectedVertices: [UInt32],
    maxIterations: Int,
    tolerance: Float,
    device: MTLDevice,
    queue: MTLCommandQueue
) -> [Float] {
    let library = try! device.makeLibrary(source: incrementalPageRankKernel, options: nil)
    let kernel = library.makeFunction(name: "incremental_pagerank")!
    let pipeline = try! device.makeComputePipelineState(function: kernel)
    
    var pr = initialPR
    let vertexCount = Int(csr.vertexCount)
    
    // Build reverse adjacency list for propagation
    var reverseAdjacency = Array(repeating: [UInt32](), count: vertexCount)
    for u in 0..<vertexCount {
        let start = Int(csr.offsets[u])
        let end = Int(csr.offsets[u + 1])
        for i in start..<end {
            let v = Int(csr.targets[i])
            reverseAdjacency[v].append(UInt32(u))
        }
    }
    
    // CPU: Manage affected vertex queue
    var affectedSet = Set<UInt32>(affectedVertices)
    var iteration = 0
    
    let startTime = CFAbsoluteTimeGetCurrent()
    
    while !affectedSet.isEmpty && iteration < maxIterations {
        // Convert set to array for GPU
        let affectedArray = Array(affectedSet)
        var affectedCount = UInt32(affectedArray.count)
        
        // GPU: Update PageRank for affected vertices
        var newPr = pr  // Copy current values
        
        let affectedBuffer = device.makeBuffer(bytes: affectedArray, length: Int(affectedCount) * MemoryLayout<UInt32>.stride, options: [])!
        let offsetsBuffer = device.makeBuffer(bytes: csr.offsets, length: (vertexCount + 1) * MemoryLayout<UInt32>.stride, options: [])!
        let targetsBuffer = device.makeBuffer(bytes: csr.targets, length: Int(csr.totalEdges) * MemoryLayout<UInt32>.stride, options: [])!
        let prBuffer = device.makeBuffer(bytes: pr, length: vertexCount * MemoryLayout<Float>.stride, options: [])!
        let newPrBuffer = device.makeBuffer(bytes: newPr, length: vertexCount * MemoryLayout<Float>.stride, options: [])!
        var vc = csr.vertexCount
        
        let commandBuffer = queue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(affectedBuffer, offset: 0, index: 0)
        encoder.setBytes(&affectedCount, length: MemoryLayout<UInt32>.stride, index: 1)
        encoder.setBuffer(offsetsBuffer, offset: 0, index: 2)
        encoder.setBuffer(targetsBuffer, offset: 0, index: 3)
        encoder.setBuffer(prBuffer, offset: 0, index: 4)
        encoder.setBuffer(newPrBuffer, offset: 0, index: 5)
        encoder.setBytes(&vc, length: MemoryLayout<UInt32>.stride, index: 6)
        
        let gridSize = MTLSize(width: Int(affectedCount), height: 1, depth: 1)
        let threadGroupSize = MTLSize(width: 256, height: 1, depth: 1)
        encoder.dispatchThreads(gridSize, threadsPerThreadgroup: threadGroupSize)
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        // Get updated scores
        let updatedPr = Array(UnsafeBufferPointer(start: newPrBuffer.contents().bindMemory(to: Float.self, capacity: vertexCount), count: vertexCount))
        
        // CPU: Check convergence and find new affected vertices
        var newAffected = Set<UInt32>()
        for i in 0..<vertexCount {
            let diff = abs(updatedPr[i] - pr[i])
            if diff > tolerance {
                // This vertex changed significantly, need to propagate to neighbors
                for neighbor in reverseAdjacency[i] {
                    newAffected.insert(neighbor)
                }
            }
        }
        
        pr = updatedPr
        affectedSet = newAffected
        iteration += 1
        
        if iteration % 5 == 0 {
            print("    Incremental iteration \(iteration): \(affectedSet.count) vertices affected, PR sum = \(pr.reduce(0, +))")
        }
    }
    
    let elapsed = CFAbsoluteTimeGetCurrent() - startTime
    print("    Incremental PageRank: \(iteration) iterations, \(String(format: "%.2f", elapsed * 1000)) ms")
    
    return pr
}

// MARK: - Main Test

print("=== Incremental PageRank Test ===\n")

let (edgeBlock, csr) = generatePowerLawGraph(vertexCount: 10000, edgeCount: 50000)
print("Generated graph: \(csr.vertexCount) vertices, \(csr.totalEdges) edges")

let (device, queue) = createMetalDevice()
print("Metal device: \(device.name)")

// Step 1: Compute full PageRank on original graph
print("\n1. Computing full PageRank on original graph (10 iterations)...")
let fullPrStart = CFAbsoluteTimeGetCurrent()
let originalPR = computeFullPageRank(csr: csr, iterations: 10, device: device, queue: queue)
let fullPrTime = CFAbsoluteTimeGetCurrent() - fullPrStart
print("   Full PageRank time: \(String(format: "%.2f", fullPrTime * 1000)) ms")
print("   PR sum: \(originalPR.reduce(0, +))")

// Step 2: Simulate graph change (add some edges)
print("\n2. Simulating graph change (adding 10 edges)...")
var affectedVertices: [UInt32] = []
for _ in 0..<10 {
    let u = UInt32.random(in: 0..<csr.vertexCount)
    let v = UInt32.random(in: 0..<csr.vertexCount)
    affectedVertices.append(u)
    affectedVertices.append(v)
}
affectedVertices = Array(Set(affectedVertices))  // Remove duplicates
print("   Affected vertices: \(affectedVertices.count)")

// Step 3: Compute incremental PageRank
print("\n3. Computing incremental PageRank (tolerance = 1e-6)...")
let incrementalPr = computeIncrementalPageRank(
    csr: csr,
    initialPR: originalPR,
    affectedVertices: affectedVertices,
    maxIterations: 50,
    tolerance: 1e-6,
    device: device,
    queue: queue
)

// Step 4: Compute full PageRank from scratch for comparison
print("\n4. Computing full PageRank from scratch for comparison...")
let fullPrFromScratch = computeFullPageRank(csr: csr, iterations: 10, device: device, queue: queue)

// Step 5: Compare results
print("\n5. Comparing results...")
var maxDiff: Float = 0
var sumDiff: Float = 0
for i in 0..<Int(csr.vertexCount) {
    let diff = abs(incrementalPr[i] - fullPrFromScratch[i])
    maxDiff = max(maxDiff, diff)
    sumDiff += diff
}
let avgDiff = sumDiff / Float(csr.vertexCount)

print("   Max difference: \(maxDiff)")
print("   Average difference: \(avgDiff)")
print("   PR sum (incremental): \(incrementalPr.reduce(0, +))")
print("   PR sum (full): \(fullPrFromScratch.reduce(0, +))")

if maxDiff < 0.001 {
    print("\n✅ Incremental PageRank results match full PageRank!")
} else {
    print("\n⚠️  Results differ significantly")
}

print("\n=== Test Complete ===")
