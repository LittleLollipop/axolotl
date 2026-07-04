import Foundation
import Metal

// MARK: - Metal Kernel 代码

let metalCode = """
#include <metal_stdlib>
using namespace metal;

struct EdgeBlock {
    uint ownerVertex;
    uint edgeCount;
    uint edges[32];
};

// 辅助函数：float 原子加法（使用 CAS 循环）
inline void atomic_add_float(device float* address, float val) {
    uint oldBits = as_type<uint>(*address);
    uint newBits;
    do {
        float oldVal = as_type<float>(oldBits);
        float newVal = oldVal + val;
        newBits = as_type<uint>(newVal);
    } while (!atomic_compare_exchange_weak_explicit(
        (device atomic_uint*)address,
        &oldBits,
        newBits,
        memory_order_relaxed,
        memory_order_relaxed));
}

// CSR 格式 PageRank
kernel void pagerank_csr(
    device const uint *va [[buffer(0)]],
    device const uint *ea [[buffer(1)]],
    device const float *rank [[buffer(2)]],
    device float *newRank [[buffer(3)]],
    device const uint *nv [[buffer(4)]],
    device const float *d [[buffer(5)]],
    uint tid [[thread_position_in_grid]]
) {
    if (tid >= *nv) return;
    
    uint start = va[tid];
    uint end = va[tid + 1];
    uint degree = end - start;
    if (degree == 0) return;
    
    float contribution = rank[tid] * (*d) / float(degree);
    
    for (uint i = start; i < end; i++) {
        uint v = ea[i];
        atomic_add_float(&newRank[v], contribution);
    }
}

// EdgeBlock 格式 PageRank
kernel void pagerank_edgeblock(
    device const uint *vertices [[buffer(0)]],
    device const uint *blockCounts [[buffer(1)]],
    device const EdgeBlock *blocks [[buffer(2)]],
    device const float *rank [[buffer(3)]],
    device float *newRank [[buffer(4)]],
    device const uint *nv [[buffer(5)]],
    device const float *d [[buffer(6)]],
    uint tid [[thread_position_in_grid]]
) {
    if (tid >= *nv) return;
    
    uint blockStart = vertices[tid];
    uint blockEnd = blockStart + blockCounts[tid];
    
    // 计算度数
    uint degree = 0;
    for (uint b = blockStart; b < blockEnd; b++) {
        degree += blocks[b].edgeCount;
    }
    if (degree == 0) return;
    
    float contribution = rank[tid] * (*d) / float(degree);
    
    // 遍历所有边
    for (uint b = blockStart; b < blockEnd; b++) {
        uint edgeCount = blocks[b].edgeCount;
        for (uint j = 0; j < edgeCount; j++) {
            uint v = blocks[b].edges[j];
            atomic_add_float(&newRank[v], contribution);
        }
    }
}
"""

// MARK: - 辅助函数

func createMetalDevice() -> (MTLDevice, MTLLibrary, MTLCommandQueue) {
    let device = MTLCreateSystemDefaultDevice()!
    let options = MTLCompileOptions()
    let library = try! device.makeLibrary(source: metalCode, options: options)
    let queue = device.makeCommandQueue()!
    return (device, library, queue)
}

// MARK: - PageRank CPU 实现（验证用）

func pagerankCPU(va: [UInt32], ea: [UInt32], nv: Int, iterations: Int = 20, damping: Float = 0.85) -> [Float] {
    let n = nv
    var rank = [Float](repeating: 1.0 / Float(n), count: n)
    var newRank = [Float](repeating: 0.0, count: n)
    let d = damping
    let oneMinusDOverN = (1.0 - d) / Float(n)
    
    for _ in 0..<iterations {
        for i in 0..<n {
            newRank[i] = oneMinusDOverN
        }
        
        for u in 0..<n {
            let start = Int(va[u])
            let end = Int(va[u + 1])
            let degree = Float(end - start)
            if degree == 0 { continue }
            let contribution = rank[u] * d / degree
            
            for i in start..<end {
                let v = Int(ea[i])
                newRank[v] += contribution
            }
        }
        
        swap(&rank, &newRank)
    }
    
    return rank
}

// MARK: - PageRank GPU 实现（CSR 格式）

func pagerankGPU_CSR(device: MTLDevice, library: MTLLibrary, commandQueue: MTLCommandQueue,
                     va: [UInt32], ea: [UInt32], nv: Int, 
                     iterations: Int = 20, damping: Float = 0.85) -> (rank: [Float], time: Double) {
    let function = library.makeFunction(name: "pagerank_csr")!
    let pipeline = try! device.makeComputePipelineState(function: function)
    
    let vaBuffer = device.makeBuffer(bytes: va, length: va.count * MemoryLayout<UInt32>.stride)!
    let eaBuffer = device.makeBuffer(bytes: ea, length: ea.count * MemoryLayout<UInt32>.stride)!
    
    var rank = [Float](repeating: 1.0 / Float(nv), count: nv)
    var newRank = [Float](repeating: 0.0, count: nv)
    
    let d = damping
    let oneMinusDOverN = (1.0 - d) / Float(nv)
    
    let startTime = CFAbsoluteTimeGetCurrent()
    
    for _ in 0..<iterations {
        // 清空 newRank
        let newRankPtr = UnsafeMutablePointer<Float>.allocate(capacity: nv)
        for i in 0..<nv {
            newRankPtr[i] = 0.0
        }
        let newRankBuffer = device.makeBuffer(bytes: newRankPtr, length: nv * MemoryLayout<Float>.stride)!
        newRankPtr.deallocate()
        
        let rankBuffer = device.makeBuffer(bytes: rank, length: rank.count * MemoryLayout<Float>.stride)!
        
        let commandBuffer = commandQueue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(vaBuffer, offset: 0, index: 0)
        encoder.setBuffer(eaBuffer, offset: 0, index: 1)
        encoder.setBuffer(rankBuffer, offset: 0, index: 2)
        encoder.setBuffer(newRankBuffer, offset: 0, index: 3)
        
        var nvUint = UInt32(nv)
        encoder.setBytes(&nvUint, length: 1, index: 4)
        var dFloat = d
        encoder.setBytes(&dFloat, length: 1, index: 5)
        
        let threadsPerGroup = MTLSize(width: 256, height: 1, depth: 1)
        let numGroups = MTLSize(width: (nv + 255) / 256, height: 1, depth: 1)
        encoder.dispatchThreadgroups(numGroups, threadsPerThreadgroup: threadsPerGroup)
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        // 读取 newRank 并添加 (1-d)/n
        let resultPtr = newRankBuffer.contents().bindMemory(to: Float.self, capacity: nv)
        for i in 0..<nv {
            newRank[i] = resultPtr[i] + oneMinusDOverN
        }
        
        swap(&rank, &newRank)
    }
    
    let endTime = CFAbsoluteTimeGetCurrent()
    
    return (rank, endTime - startTime)
}

// MARK: - PageRank GPU 实现（EdgeBlock 格式）

func pagerankGPU_EdgeBlock(device: MTLDevice, library: MTLLibrary, commandQueue: MTLCommandQueue,
                           vertices: [UInt32], blockCounts: [UInt32], blocks: [UInt32], nv: Int,
                           iterations: Int = 20, damping: Float = 0.85) -> (rank: [Float], time: Double) {
    let function = library.makeFunction(name: "pagerank_edgeblock")!
    let pipeline = try! device.makeComputePipelineState(function: function)
    
    let blocksBuffer = device.makeBuffer(bytes: blocks, length: blocks.count * MemoryLayout<UInt32>.stride)!
    let verticesBuffer = device.makeBuffer(bytes: vertices, length: vertices.count * MemoryLayout<UInt32>.stride)!
    let blockCountsBuffer = device.makeBuffer(bytes: blockCounts, length: blockCounts.count * MemoryLayout<UInt32>.stride)!
    
    var rank = [Float](repeating: 1.0 / Float(nv), count: nv)
    var newRank = [Float](repeating: 0.0, count: nv)
    
    let d = damping
    let oneMinusDOverN = (1.0 - d) / Float(nv)
    
    let startTime = CFAbsoluteTimeGetCurrent()
    
    for _ in 0..<iterations {
        // 清空 newRank
        let newRankPtr = UnsafeMutablePointer<Float>.allocate(capacity: nv)
        for i in 0..<nv {
            newRankPtr[i] = 0.0
        }
        let newRankBuffer = device.makeBuffer(bytes: newRankPtr, length: nv * MemoryLayout<Float>.stride)!
        newRankPtr.deallocate()
        
        let rankBuffer = device.makeBuffer(bytes: rank, length: rank.count * MemoryLayout<Float>.stride)!
        
        let commandBuffer = commandQueue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(verticesBuffer, offset: 0, index: 0)
        encoder.setBuffer(blockCountsBuffer, offset: 0, index: 1)
        encoder.setBuffer(blocksBuffer, offset: 0, index: 2)
        encoder.setBuffer(rankBuffer, offset: 0, index: 3)
        encoder.setBuffer(newRankBuffer, offset: 0, index: 4)
        
        var nvUint = UInt32(nv)
        encoder.setBytes(&nvUint, length: 1, index: 5)
        var dFloat = d
        encoder.setBytes(&dFloat, length: 1, index: 6)
        
        let threadsPerGroup = MTLSize(width: 256, height: 1, depth: 1)
        let numGroups = MTLSize(width: (nv + 255) / 256, height: 1, depth: 1)
        encoder.dispatchThreadgroups(numGroups, threadsPerThreadgroup: threadsPerGroup)
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        // 读取 newRank 并添加 (1-d)/n
        let resultPtr = newRankBuffer.contents().bindMemory(to: Float.self, capacity: nv)
        for i in 0..<nv {
            newRank[i] = resultPtr[i] + oneMinusDOverN
        }
        
        swap(&rank, &newRank)
    }
    
    let endTime = CFAbsoluteTimeGetCurrent()
    
    return (rank, endTime - startTime)
}

// MARK: - 图生成（幂律图）

func generatePowerLawGraph(nv: Int, avgDegree: Int) -> (va: [UInt32], ea: [UInt32]) {
    var va = [UInt32](repeating: 0, count: nv + 1)
    var edges: [UInt32] = []
    
    // 生成幂律分布的度数
    var degrees = [Int](repeating: 0, count: nv)
    for i in 0..<nv {
        let alpha: Double = 2.5
        let rank = Double(i + 1)
        let degree = Int(pow(rank, -1.0/alpha) * Double(avgDegree) * Double(nv) / 10.0)
        degrees[i] = max(1, min(degree, nv - 1))
    }
    
    // 调整总边数
    let totalEdges = degrees.reduce(0, +)
    let targetEdges = nv * avgDegree
    if totalEdges < targetEdges {
        for i in 0..<nv {
            degrees[i] = Int(Double(degrees[i]) * Double(targetEdges) / Double(totalEdges)) + 1
        }
    }
    
    // 生成边
    for u in 0..<nv {
        let degree = degrees[u]
        var neighbors = Set<UInt32>()
        while neighbors.count < degree {
            let v = UInt32(Int.random(in: 0..<nv))
            if v != UInt32(u) {
                neighbors.insert(v)
            }
        }
        for v in neighbors {
            edges.append(v)
        }
        va[u + 1] = va[u] + UInt32(degree)
    }
    
    // 排序 edges
    var sortedEdges: [UInt32] = []
    var offset = 0
    for u in 0..<nv {
        let degree = Int(va[u + 1] - va[u])
        let start = offset
        let end = offset + degree
        let vertexEdges = Array(edges[start..<end]).sorted()
        sortedEdges.append(contentsOf: vertexEdges)
        offset = end
    }
    
    // 重新计算 va
    var idx: UInt32 = 0
    for u in 0..<nv {
        va[Int(u)] = idx
        idx += UInt32(degrees[Int(u)])
    }
    va[nv] = idx
    
    return (va, sortedEdges)
}

// MARK: - CSR 转 EdgeBlock

func csrToEdgeBlock(va: [UInt32], ea: [UInt32], nv: Int) -> (vertices: [UInt32], blockCounts: [UInt32], blocks: [UInt32]) {
    let BLOCK_CAPACITY: UInt32 = 32
    var vertices = [UInt32](repeating: 0, count: nv)
    var blockCounts = [UInt32](repeating: 0, count: nv)
    var blocks: [UInt32] = []
    
    for i in 0..<nv {
        vertices[i] = UInt32(blocks.count / (2 + Int(BLOCK_CAPACITY)))
        let start = Int(va[i]), end = Int(va[i + 1])
        var offset = start
        while offset < end {
            let edgesInThisBlock = min(Int(BLOCK_CAPACITY), end - offset)
            blocks.append(UInt32(i))  // ownerVertex
            blocks.append(UInt32(edgesInThisBlock))  // edgeCount
            for j in 0..<Int(BLOCK_CAPACITY) {
                if j < edgesInThisBlock {
                    blocks.append(ea[offset + j])
                } else {
                    blocks.append(UInt32.max)  // PADDING
                }
            }
            offset += edgesInThisBlock
            blockCounts[i] += 1
        }
    }
    return (vertices, blockCounts, blocks)
}

// MARK: - 主函数

func runPageRankComparison() {
    print("=== PageRank 测试（EdgeBlock vs CSR）===")
    print()
    
    let (device, library, commandQueue) = createMetalDevice()
    print("GPU: \(device.name)")
    print()
    
    let iterations = 20
    
    // 测试不同规模的幂律图
    let testCases = [
        (nv: 10000, avgDegree: 10),
        (nv: 50000, avgDegree: 10),
        (nv: 100000, avgDegree: 10)
    ]
    
    for (nv, avgDegree) in testCases {
        print("测试: \(nv) 顶点, 平均度数 \(avgDegree)")
        
        // 生成幂律图
        let (va, ea) = generatePowerLawGraph(nv: nv, avgDegree: avgDegree)
        let (vertices, blockCounts, blocks) = csrToEdgeBlock(va: va, ea: ea, nv: nv)
        
        // CPU PageRank（验证用）
        let cpuStart = CFAbsoluteTimeGetCurrent()
        let cpuRank = pagerankCPU(va: va, ea: ea, nv: nv, iterations: iterations)
        let cpuTime = CFAbsoluteTimeGetCurrent() - cpuStart
        print("  CPU PageRank: \(String(format: "%.2f", cpuTime * 1000)) ms")
        
        // GPU CSR
        let (csrRank, csrTime) = pagerankGPU_CSR(device: device, library: library, commandQueue: commandQueue,
                                                  va: va, ea: ea, nv: nv, iterations: iterations)
        print("  GPU CSR: \(String(format: "%.2f", csrTime * 1000)) ms")
        
        // 验证 CSR 结果
        var csrError: Float = 0.0
        for i in 0..<nv {
            let error = abs(cpuRank[i] - csrRank[i])
            if error > csrError {
                csrError = error
            }
        }
        print("    CSR 最大误差: \(csrError)")
        
        // GPU EdgeBlock
        let (ebRank, ebTime) = pagerankGPU_EdgeBlock(device: device, library: library, commandQueue: commandQueue,
                                                      vertices: vertices, blockCounts: blockCounts, blocks: blocks, nv: nv,
                                                      iterations: iterations)
        print("  GPU EdgeBlock: \(String(format: "%.2f", ebTime * 1000)) ms")
        
        // 验证 EdgeBlock 结果
        var ebError: Float = 0.0
        for i in 0..<nv {
            let error = abs(cpuRank[i] - ebRank[i])
            if error > ebError {
                ebError = error
            }
        }
        print("    EdgeBlock 最大误差: \(ebError)")
        
        // 比较性能
        let speedup = csrTime / ebTime
        print("  加速比 (CSR/EdgeBlock): \(String(format: "%.2f", speedup))x")
        
        print()
    }
}

runPageRankComparison()
