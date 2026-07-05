import Foundation
import Metal
import Dispatch

// MARK: - Metal Kernel（EdgeBlock 格式，支持指定顶点范围）

let metalCode = """
#include <metal_stdlib>
using namespace metal;

struct EdgeBlock {
    uint ownerVertex;
    uint edgeCount;
    uint edges[32];
};

// GPU BFS（只处理 frontier 中的一部分顶点）
kernel void bfs_gpu_partial(
    device const uint *vertices [[buffer(0)]],
    device const uint *blockCounts [[buffer(1)]],
    device const EdgeBlock *blocks [[buffer(2)]],
    device const uint *frontier [[buffer(3)]],
    device uint *nextFrontier [[buffer(4)]],
    device uint *nextCount [[buffer(5)]],
    device uint *visited [[buffer(6)]],
    device const uint *frontierSize [[buffer(7)]],
    device const uint *startIdx [[buffer(8)]],  // 处理的起始索引
    device const uint *endIdx [[buffer(9)]],    // 处理的结束索引
    uint tid [[thread_position_in_grid]]
) {
    if (tid >= (*endIdx - *startIdx)) return;
    
    uint frontierIdx = *startIdx + tid;
    if (frontierIdx >= *endIdx) return;
    
    uint u = frontier[frontierIdx];
    
    uint blockStart = vertices[u];
    uint blockEnd = blockStart + blockCounts[u];
    
    for (uint b = blockStart; b < blockEnd; b++) {
        uint edgeCount = blocks[b].edgeCount;
        for (uint j = 0; j < edgeCount; j++) {
            uint v = blocks[b].edges[j];
            
            if (atomic_fetch_or_explicit((device atomic_uint*)&visited[v], 1, memory_order_relaxed) == 0) {
                uint pos = atomic_fetch_add_explicit((device atomic_uint*)nextCount, 1, memory_order_relaxed);
                nextFrontier[pos] = v;
            }
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

// MARK: - 图生成（幂律图）

func generatePowerLawGraph(nv: Int, avgDegree: Int) -> (va: [UInt32], ea: [UInt32]) {
    var va = [UInt32](repeating: 0, count: nv + 1)
    var edges: [UInt32] = []
    
    var degrees = [Int](repeating: 0, count: nv)
    for i in 0..<nv {
        let alpha: Double = 2.5
        let rank = Double(i + 1)
        let degree = Int(pow(rank, -1.0/alpha) * Double(avgDegree) * Double(nv) / 10.0)
        degrees[i] = max(1, min(degree, nv - 1))
    }
    
    let totalEdges = degrees.reduce(0, +)
    let targetEdges = nv * avgDegree
    if totalEdges < targetEdges {
        for i in 0..<nv {
            degrees[i] = Int(Double(degrees[i]) * Double(targetEdges) / Double(totalEdges)) + 1
        }
    }
    
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
        vertices[i] = UInt32(blocks.count / 34)
        let start = Int(va[i]), end = Int(va[i + 1])
        var offset = start
        while offset < end {
            let edgesInThisBlock = min(Int(BLOCK_CAPACITY), end - offset)
            blocks.append(UInt32(i))
            blocks.append(UInt32(edgesInThisBlock))
            for j in 0..<Int(BLOCK_CAPACITY) {
                if j < edgesInThisBlock {
                    blocks.append(ea[offset + j])
                } else {
                    blocks.append(UInt32.max)
                }
            }
            offset += edgesInThisBlock
            blockCounts[i] += 1
        }
    }
    return (vertices, blockCounts, blocks)
}

// MARK: - 计算顶点 degree

func computeDegrees(va: [UInt32], nv: Int) -> [Int] {
    var degrees = [Int](repeating: 0, count: nv)
    for i in 0..<nv {
        degrees[i] = Int(va[i + 1] - va[i])
    }
    return degrees
}

// MARK: - Heterogeneous BFS

func heterogeneousBFS(
    device: MTLDevice,
    library: MTLLibrary,
    commandQueue: MTLCommandQueue,
    vertices: [UInt32],
    blockCounts: [UInt32],
    blocks: [UInt32],
    va: [UInt32],  // 用于 CPU 获取邻居
    ea: [UInt32],  // 用于 CPU 获取邻居
    nv: Int,
    source: Int,
    degreeThreshold: Int
) -> (dist: [Int], time: Double) {
    
    // 计算顶点 degree 并分区
    let degrees = computeDegrees(va: va, nv: nv)
    var vertexPartition = [Int](repeating: 1, count: nv)  // 1 = GPU, 0 = CPU
    for i in 0..<nv {
        if degrees[i] > degreeThreshold {
            vertexPartition[i] = 0  // CPU
        }
    }
    
    print("  分区: CPU 顶点数 = \(vertexPartition.filter { $0 == 0 }.count), GPU 顶点数 = \(vertexPartition.filter { $0 == 1 }.count)")
    
    // 创建 GPU buffer
    let verticesBuffer = device.makeBuffer(bytes: vertices, length: vertices.count * MemoryLayout<UInt32>.stride)!
    let blockCountsBuffer = device.makeBuffer(bytes: blockCounts, length: blockCounts.count * MemoryLayout<UInt32>.stride)!
    
    // 创建 EdgeBlock 结构体 buffer
    var edgeBlockData: [UInt32] = []
    let blockCount = vertices.count > 0 ? (blocks.count / 34) : 0
    for b in 0..<blockCount {
        edgeBlockData.append(blocks[b * 34])      // ownerVertex
        edgeBlockData.append(blocks[b * 34 + 1])  // edgeCount
        for j in 0..<32 {
            edgeBlockData.append(blocks[b * 34 + 2 + j])  // edges
        }
    }
    let blocksBuffer = device.makeBuffer(bytes: edgeBlockData, length: edgeBlockData.count * MemoryLayout<UInt32>.stride)!
    
    // BFS 初始化
    var dist = [Int](repeating: -1, count: nv)
    var frontier: [UInt32] = [UInt32(source)]
    dist[source] = 0
    
    let startTime = CFAbsoluteTimeGetCurrent()
    var level = 0
    
    while !frontier.isEmpty {
        level += 1
        
        // 分区 frontier
        var cpuFrontier: [UInt32] = []
        var gpuFrontier: [UInt32] = []
        
        for u in frontier {
            if vertexPartition[Int(u)] == 0 {
                cpuFrontier.append(u)
            } else {
                gpuFrontier.append(u)
            }
        }
        
        var nextFrontier: [UInt32] = []
        let nextFrontierLock = NSLock()
        
        // CPU 处理（使用 DispatchQueue 并行）
        let cpuGroup = DispatchGroup()
        
        if !cpuFrontier.isEmpty {
            DispatchQueue.global(qos: .userInteractive).async(group: cpuGroup) {
                for u in cpuFrontier {
                    let start = Int(va[Int(u)])
                    let end = Int(va[Int(u) + 1])
                    for i in start..<end {
                        let v = Int(ea[i])
                        if dist[v] == -1 {
                            dist[v] = level
                            nextFrontierLock.lock()
                            nextFrontier.append(UInt32(v))
                            nextFrontierLock.unlock()
                        }
                    }
                }
            }
        }
        
        // GPU 处理
        var gpuNextFrontier: [UInt32] = []
        if !gpuFrontier.isEmpty {
            let function = library.makeFunction(name: "bfs_gpu_partial")!
            let pipeline = try! device.makeComputePipelineState(function: function)
            
            let frontierBuffer = device.makeBuffer(bytes: gpuFrontier, length: gpuFrontier.count * MemoryLayout<UInt32>.stride)!
            let nextFrontierBuffer = device.makeBuffer(length: nv * MemoryLayout<UInt32>.stride)!
            let nextCountBuffer = device.makeBuffer(length: MemoryLayout<UInt32>.stride)!
            
            // visited 数组（用 UInt32 表示）
            var visited = [UInt32](repeating: 0, count: nv)
            for i in 0..<nv {
                if dist[i] != -1 {
                    visited[i] = 1
                }
            }
            let visitedBuffer = device.makeBuffer(bytes: visited, length: visited.count * MemoryLayout<UInt32>.stride)!
            
            var startIdx: UInt32 = 0
            var endIdx = UInt32(gpuFrontier.count)
            let startIdxBuffer = device.makeBuffer(bytes: &startIdx, length: MemoryLayout<UInt32>.stride)!
            let endIdxBuffer = device.makeBuffer(bytes: &endIdx, length: MemoryLayout<UInt32>.stride)!
            
            let commandBuffer = commandQueue.makeCommandBuffer()!
            let encoder = commandBuffer.makeComputeCommandEncoder()!
            encoder.setComputePipelineState(pipeline)
            encoder.setBuffer(verticesBuffer, offset: 0, index: 0)
            encoder.setBuffer(blockCountsBuffer, offset: 0, index: 1)
            encoder.setBuffer(blocksBuffer, offset: 0, index: 2)
            encoder.setBuffer(frontierBuffer, offset: 0, index: 3)
            encoder.setBuffer(nextFrontierBuffer, offset: 0, index: 4)
            encoder.setBuffer(nextCountBuffer, offset: 0, index: 5)
            encoder.setBuffer(visitedBuffer, offset: 0, index: 6)
            encoder.setBuffer(startIdxBuffer, offset: 0, index: 8)
            encoder.setBuffer(endIdxBuffer, offset: 0, index: 9)
            
            let threadsPerGroup = MTLSize(width: 256, height: 1, depth: 1)
            let numGroups = MTLSize(width: (gpuFrontier.count + 255) / 256, height: 1, depth: 1)
            encoder.dispatchThreadgroups(numGroups, threadsPerThreadgroup: threadsPerGroup)
            encoder.endEncoding()
            commandBuffer.commit()
            commandBuffer.waitUntilCompleted()
            
            // 读取 GPU 结果
            let nextCountPtr = nextCountBuffer.contents().bindMemory(to: UInt32.self, capacity: 1)
            let gpuNextCount = Int(nextCountPtr[0])
            
            let nextFrontierPtr = nextFrontierBuffer.contents().bindMemory(to: UInt32.self, capacity: nv)
            for i in 0..<gpuNextCount {
                let v = Int(nextFrontierPtr[i])
                if dist[v] == -1 {
                    dist[v] = level
                    gpuNextFrontier.append(UInt32(v))
                }
            }
        }
        
        // 等待 CPU 完成
        cpuGroup.wait()
        
        // 合并结果
        nextFrontier.append(contentsOf: gpuNextFrontier)
        frontier = nextFrontier
    }
    
    let endTime = CFAbsoluteTimeGetCurrent()
    
    return (dist, endTime - startTime)
}

// MARK: - 普通 GPU BFS（用于对比）

func gpuBFS(
    device: MTLDevice,
    library: MTLLibrary,
    commandQueue: MTLCommandQueue,
    vertices: [UInt32],
    blockCounts: [UInt32],
    blocks: [UInt32],
    nv: Int,
    source: Int
) -> (dist: [Int], time: Double) {
    
    let function = library.makeFunction(name: "bfs_gpu_partial")!
    let pipeline = try! device.makeComputePipelineState(function: function)
    
    let verticesBuffer = device.makeBuffer(bytes: vertices, length: vertices.count * MemoryLayout<UInt32>.stride)!
    let blockCountsBuffer = device.makeBuffer(bytes: blockCounts, length: blockCounts.count * MemoryLayout<UInt32>.stride)!
    
    var edgeBlockData: [UInt32] = []
    let blockCount = vertices.count > 0 ? (blocks.count / 34) : 0
    for b in 0..<blockCount {
        edgeBlockData.append(blocks[b * 34])
        edgeBlockData.append(blocks[b * 34 + 1])
        for j in 0..<32 {
            edgeBlockData.append(blocks[b * 34 + 2 + j])
        }
    }
    let blocksBuffer = device.makeBuffer(bytes: edgeBlockData, length: edgeBlockData.count * MemoryLayout<UInt32>.stride)!
    
    var dist = [Int](repeating: -1, count: nv)
    var frontier: [UInt32] = [UInt32(source)]
    dist[source] = 0
    
    let startTime = CFAbsoluteTimeGetCurrent()
    var level = 0
    
    while !frontier.isEmpty {
        level += 1
        
        let frontierBuffer = device.makeBuffer(bytes: frontier, length: frontier.count * MemoryLayout<UInt32>.stride)!
        let nextFrontierBuffer = device.makeBuffer(length: nv * MemoryLayout<UInt32>.stride)!
        let nextCountBuffer = device.makeBuffer(length: MemoryLayout<UInt32>.stride)!
        
        var visited = [UInt32](repeating: 0, count: nv)
        for i in 0..<nv {
            if dist[i] != -1 {
                visited[i] = 1
            }
        }
        let visitedBuffer = device.makeBuffer(bytes: visited, length: visited.count * MemoryLayout<UInt32>.stride)!
        
        var startIdx: UInt32 = 0
        var endIdx = UInt32(frontier.count)
        let startIdxBuffer = device.makeBuffer(bytes: &startIdx, length: MemoryLayout<UInt32>.stride)!
        let endIdxBuffer = device.makeBuffer(bytes: &endIdx, length: MemoryLayout<UInt32>.stride)!
        
        let commandBuffer = commandQueue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        encoder.setComputePipelineState(pipeline)
        encoder.setBuffer(verticesBuffer, offset: 0, index: 0)
        encoder.setBuffer(blockCountsBuffer, offset: 0, index: 1)
        encoder.setBuffer(blocksBuffer, offset: 0, index: 2)
        encoder.setBuffer(frontierBuffer, offset: 0, index: 3)
        encoder.setBuffer(nextFrontierBuffer, offset: 0, index: 4)
        encoder.setBuffer(nextCountBuffer, offset: 0, index: 5)
        encoder.setBuffer(visitedBuffer, offset: 0, index: 6)
        encoder.setBuffer(startIdxBuffer, offset: 0, index: 8)
        encoder.setBuffer(endIdxBuffer, offset: 0, index: 9)
        
        let threadsPerGroup = MTLSize(width: 256, height: 1, depth: 1)
        let numGroups = MTLSize(width: (frontier.count + 255) / 256, height: 1, depth: 1)
        encoder.dispatchThreadgroups(numGroups, threadsPerThreadgroup: threadsPerGroup)
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        let nextCountPtr = nextCountBuffer.contents().bindMemory(to: UInt32.self, capacity: 1)
        let gpuNextCount = Int(nextCountPtr[0])
        
        let nextFrontierPtr = nextFrontierBuffer.contents().bindMemory(to: UInt32.self, capacity: nv)
        var nextFrontier: [UInt32] = []
        for i in 0..<gpuNextCount {
            let v = Int(nextFrontierPtr[i])
            if dist[v] == -1 {
                dist[v] = level
                nextFrontier.append(UInt32(v))
            }
        }
        
        frontier = nextFrontier
    }
    
    let endTime = CFAbsoluteTimeGetCurrent()
    
    return (dist, endTime - startTime)
}

// MARK: - 主函数

func runHeterogeneousBFSComparison() {
    print("=== Heterogeneous BFS 测试 ===")
    print()
    
    let (device, library, commandQueue) = createMetalDevice()
    print("GPU: \(device.name)")
    print()
    
    let nv = 100000
    let avgDegree = 10
    let source = 0
    
    print("生成图: \(nv) 顶点, 平均度数 \(avgDegree)")
    let (va, ea) = generatePowerLawGraph(nv: nv, avgDegree: avgDegree)
    let (vertices, blockCounts, blocks) = csrToEdgeBlock(va: va, ea: ea, nv: nv)
    print("图生成完成")
    print()
    
    // 测试不同 degree 阈值的 Heterogeneous BFS
    let thresholds = [10, 50, 100, 500]
    
    for threshold in thresholds {
        print("测试: degree 阈值 = \(threshold)")
        
        let (_, time) = heterogeneousBFS(
            device: device,
            library: library,
            commandQueue: commandQueue,
            vertices: vertices,
            blockCounts: blockCounts,
            blocks: blocks,
            va: va,
            ea: ea,
            nv: nv,
            source: source,
            degreeThreshold: threshold
        )
        
        print("  Heterogeneous BFS: \(time * 1000) ms")
        print()
    }
    
    // 对比：普通 GPU BFS
    print("对比: 普通 GPU BFS")
    let (_, gpuTime) = gpuBFS(
        device: device,
        library: library,
        commandQueue: commandQueue,
        vertices: vertices,
        blockCounts: blockCounts,
        blocks: blocks,
        nv: nv,
        source: source
    )
    print("  GPU BFS: \(gpuTime * 1000) ms")
    print()
}

runHeterogeneousBFSComparison()
