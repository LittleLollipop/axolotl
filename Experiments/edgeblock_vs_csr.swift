//
// edgeblock_vs_csr.swift — EdgeBlock 格式 vs CSR 格式 GPU BFS 完整对比
//
// 基于 warp_bfs.swift 的验证过的实现
//
// 编译 & 运行：
//   swiftc -o edgeblock_vs_csr edgeblock_vs_csr.swift
//   ./edgeblock_vs_csr
//

import Metal
import Foundation

// ─────────────────────────────────────────────────────────────────────────────
// 配置
// ─────────────────────────────────────────────────────────────────────────────

let NV = 100_000
let BLOCK_CAPACITY = 32

// ─────────────────────────────────────────────────────────────────────────────
// 图数据结构
// ─────────────────────────────────────────────────────────────────────────────

struct CSRGraph {
    let va: [UInt32]  // offsets
    let ea: [UInt32]  // edges
}

//
// 生成随机图（CSR 格式）
//
func genRandomGraphCSR(n: Int, k: Int) -> CSRGraph {
    var adj = Array(repeating: [UInt32](), count: n)
    srand48(42)
    for i in 0..<n {
        var cnt = 0
        while cnt < k {
            let nb = Int(drand48() * Double(n)) % n
            if nb == i { continue }
            if adj[i].contains(UInt32(nb)) { continue }
            adj[i].append(UInt32(nb))
            cnt += 1
        }
    }
    var va = [UInt32](repeating: 0, count: n + 1)
    var ea = [UInt32]()
    for i in 0..<n {
        va[i] = UInt32(ea.count)
        ea.append(contentsOf: adj[i].sorted())
    }
    va[n] = UInt32(ea.count)
    return CSRGraph(va: va, ea: ea)
}

//
// EdgeBlock 格式图
//
struct EdgeBlockGraph {
    let vertices: [UInt32]      // 每个顶点的第一个 EdgeBlock 索引
    let blockCounts: [UInt32]   // 每个顶点的 EdgeBlock 数量
    let blocks: [UInt32]        // 扁平化的 EdgeBlock 数据
}

//
// 从 CSR 格式转换成 EdgeBlock 格式
//
func csrToEdgeBlock(csr: CSRGraph, blockCapacity: Int) -> EdgeBlockGraph {
    let n = csr.va.count - 1
    var vertices = [UInt32](repeating: 0, count: n)
    var blockCounts = [UInt32](repeating: 0, count: n)
    var blocks: [UInt32] = []
    
    for i in 0..<n {
        vertices[i] = UInt32(blocks.count / (2 + blockCapacity))
        
        let start = Int(csr.va[i])
        let end = Int(csr.va[i + 1])
        let degree = end - start
        
        if degree == 0 { continue }
        
        // 分成多个 Block
        var offset = start
        while offset < end {
            let edgesInThisBlock = min(blockCapacity, end - offset)
            
            // 添加 EdgeBlock header
            blocks.append(UInt32(i))  // ownerVertex
            blocks.append(UInt32(edgesInThisBlock))  // edgeCount
            
            // 添加边
            for j in 0..<edgesInThisBlock {
                blocks.append(csr.ea[offset + j])
            }
            
            // 填充剩余空间（GPU 期望固定大小）
            for _ in edgesInThisBlock..<blockCapacity {
                blocks.append(0)
            }
            
            offset += edgesInThisBlock
            blockCounts[i] += 1
        }
    }
    
    return EdgeBlockGraph(vertices: vertices, blockCounts: blockCounts, blocks: blocks)
}

// ─────────────────────────────────────────────────────────────────────────────
// Metal Kernel
// ─────────────────────────────────────────────────────────────────────────────

let csrKernel = """
#include <metal_stdlib>
using namespace metal;

kernel void bfs_csr(
    device const uint *va [[buffer(0)]],
    device const uint *ea [[buffer(1)]],
    device const uint *frontier [[buffer(2)]],
    constant uint &frontierSize [[buffer(3)]],
    device uint *nextFrontier [[buffer(4)]],
    device uint *nextCount [[buffer(5)]],
    device uint *visited [[buffer(6)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= frontierSize) return;
    
    uint v = frontier[gid];
    uint start = va[v];
    uint end = va[v + 1];
    
    for (uint i = start; i < end; i++) {
        uint nb = ea[i];
        if (atomic_fetch_add_explicit((device atomic_uint*)&visited[nb], 1, memory_order_relaxed) == 0) {
            uint idx = atomic_fetch_add_explicit((device atomic_uint*)nextCount, 1, memory_order_relaxed);
            nextFrontier[idx] = nb;
        }
    }
}
"""

let edgeBlockKernel = """
#include <metal_stdlib>
using namespace metal;

struct EdgeBlock {
    uint ownerVertex;
    uint edgeCount;
    uint edges[32];
};

kernel void bfs_edgeblock(
    device const uint *vertices [[buffer(0)]],
    device const uint *blockCounts [[buffer(1)]],
    device const EdgeBlock *blocks [[buffer(2)]],
    device const uint *frontier [[buffer(3)]],
    constant uint &frontierSize [[buffer(4)]],
    device uint *nextFrontier [[buffer(5)]],
    device uint *nextCount [[buffer(6)]],
    device uint *visited [[buffer(7)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= frontierSize) return;
    
    uint v = frontier[gid];
    uint blockStart = vertices[v];
    uint blockCount = blockCounts[v];
    
    for (uint i = 0; i < blockCount; i++) {
        EdgeBlock block = blocks[blockStart + i];
        for (uint j = 0; j < block.edgeCount; j++) {
            uint nb = block.edges[j];
            if (atomic_fetch_add_explicit((device atomic_uint*)&visited[nb], 1, memory_order_relaxed) == 0) {
                uint idx = atomic_fetch_add_explicit((device atomic_uint*)nextCount, 1, memory_order_relaxed);
                nextFrontier[idx] = nb;
            }
        }
    }
}
"""

// ─────────────────────────────────────────────────────────────────────────────
// BFS 执行函数
// ─────────────────────────────────────────────────────────────────────────────

func runBFS(device: MTLDevice, pipeline: MTLComputePipelineState, 
           graphBuffers: [MTLBuffer], frontierIdx: Int, vertexCount: Int, maxFrontier: Int) -> (Int, Double) {
    
    let queue = device.makeCommandQueue()!
    
    var visited = [UInt32](repeating: 0, count: vertexCount)
    var frontier = [UInt32](repeating: 0, count: maxFrontier)
    frontier[0] = 0
    var frontierCount = 1
    var visitedCount = 0
    
    let startTime = CFAbsoluteTimeGetCurrent()
    
    while frontierCount > 0 {
        // 创建 command buffer
        let commandBuffer = queue.makeCommandBuffer()!
        let encoder = commandBuffer.makeComputeCommandEncoder()!
        
        encoder.setComputePipelineState(pipeline)
        
        // 设置图数据 buffers
        for (i, buffer) in graphBuffers.enumerated() {
            encoder.setBuffer(buffer, offset: 0, index: i)
        }
        
        // 设置 frontier buffer (index = frontierIdx)
        let frontierBuffer = device.makeBuffer(bytes: frontier, length: frontierCount * MemoryLayout<UInt32>.size, options: [])!
        encoder.setBuffer(frontierBuffer, offset: 0, index: frontierIdx)
        
        // 设置 frontierSize (index = frontierIdx + 1)
        var fs = UInt32(frontierCount)
        encoder.setBytes(&fs, length: 4, index: frontierIdx + 1)
        
        // 设置 nextFrontier, nextCount, visited (index = frontierIdx + 2, 3, 4)
        let nextFrontierBuffer = device.makeBuffer(length: maxFrontier * MemoryLayout<UInt32>.size, options: [])!
        encoder.setBuffer(nextFrontierBuffer, offset: 0, index: frontierIdx + 2)
        
        var nextCount: UInt32 = 0
        let nextCountBuffer = device.makeBuffer(bytes: &nextCount, length: 4, options: [])!
        encoder.setBuffer(nextCountBuffer, offset: 0, index: frontierIdx + 3)
        
        let visitedBuffer = device.makeBuffer(bytes: visited, length: visited.count * MemoryLayout<UInt32>.size, options: [])!
        encoder.setBuffer(visitedBuffer, offset: 0, index: frontierIdx + 4)
        
        // dispatch
        let threadsPerThreadgroup = 256
        let threadgroups = (frontierCount + threadsPerThreadgroup - 1) / threadsPerThreadgroup
        encoder.dispatchThreadgroups(MTLSize(width: threadgroups, height: 1, depth: 1),
                                     threadsPerThreadgroup: MTLSize(width: threadsPerThreadgroup, height: 1, depth: 1))
        encoder.endEncoding()
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        
        // 读取结果
        let nextCountPtr = nextCountBuffer.contents().bindMemory(to: UInt32.self, capacity: 1)
        frontierCount = Int(nextCountPtr[0])
        
        let nextFrontierPtr = nextFrontierBuffer.contents().bindMemory(to: UInt32.self, capacity: maxFrontier)
        for i in 0..<frontierCount {
            frontier[i] = nextFrontierPtr[i]
        }
        
        let visitedPtr = visitedBuffer.contents().bindMemory(to: UInt32.self, capacity: visited.count)
        for i in 0..<visited.count {
            visited[i] = visitedPtr[i]
        }
        
        // 统计 visited
        visitedCount = 0
        for v in visited {
            if v > 0 { visitedCount += 1 }
        }
    }
    
    let elapsed = CFAbsoluteTimeGetCurrent() - startTime
    return (visitedCount, elapsed)
}

// ─────────────────────────────────────────────────────────────────────────────
// 主函数
// ─────────────────────────────────────────────────────────────────────────────

func main() {
    print("=== EdgeBlock vs CSR GPU BFS 对比 ===\n")
    print("顶点数: \(NV), 每个顶点约 10 条边\n")
    
    let device = MTLCreateSystemDefaultDevice()!
    print("Metal device: \(device.name)\n")
    
    // 生成 CSR 格式的图
    print("生成 CSR 格式的图...")
    let csr = genRandomGraphCSR(n: NV, k: 10)
    print("  边数: \(csr.ea.count)\n")
    
    // 转换成 EdgeBlock 格式
    print("转换成 EdgeBlock 格式...")
    let eb = csrToEdgeBlock(csr: csr, blockCapacity: BLOCK_CAPACITY)
    print("  EdgeBlock 数量: \(eb.blocks.count / (2 + BLOCK_CAPACITY))")
    print("  数据大小: \(eb.blocks.count * 4) bytes\n")
    
    // 上传 CSR 数据到 GPU
    print("上传 CSR 数据到 GPU...")
    let csrVaBuffer = device.makeBuffer(bytes: csr.va, length: csr.va.count * 4, options: [])!
    let csrEaBuffer = device.makeBuffer(bytes: csr.ea, length: csr.ea.count * 4, options: [])!
    print("  完成\n")
    
    // 上传 EdgeBlock 数据到 GPU
    print("上传 EdgeBlock 数据到 GPU...")
    let ebVerticesBuffer = device.makeBuffer(bytes: eb.vertices, length: eb.vertices.count * 4, options: [])!
    let ebBlockCountsBuffer = device.makeBuffer(bytes: eb.blockCounts, length: eb.blockCounts.count * 4, options: [])!
    let ebBlocksBuffer = device.makeBuffer(bytes: eb.blocks, length: eb.blocks.count * 4, options: [])!
    print("  完成\n")
    
    // 编译 CSR kernel
    print("编译 CSR BFS kernel...")
    let csrLib = try! device.makeLibrary(source: csrKernel, options: nil)
    let csrPipeline = try! device.makeComputePipelineState(function: csrLib.makeFunction(name: "bfs_csr")!)
    print("  完成\n")
    
    // 编译 EdgeBlock kernel
    print("编译 EdgeBlock BFS kernel...")
    let ebLib = try! device.makeLibrary(source: edgeBlockKernel, options: nil)
    let ebPipeline = try! device.makeComputePipelineState(function: ebLib.makeFunction(name: "bfs_edgeblock")!)
    print("  完成\n")
    
    // 运行 CSR BFS
    print("运行 CSR BFS...")
    let (csrCount, csrTime) = runBFS(device: device, pipeline: csrPipeline, 
                                     graphBuffers: [csrVaBuffer, csrEaBuffer], 
                                     frontierIdx: 2,
                                     vertexCount: NV, maxFrontier: NV)
    print("  访问了 \(csrCount) 个顶点")
    print("  耗时: \(String(format: "%.6f", csrTime)) 秒\n")
    
    // 运行 EdgeBlock BFS
    print("运行 EdgeBlock BFS...")
    let (ebCount, ebTime) = runBFS(device: device, pipeline: ebPipeline,
                                   graphBuffers: [ebVerticesBuffer, ebBlockCountsBuffer, ebBlocksBuffer],
                                   frontierIdx: 3,
                                   vertexCount: NV, maxFrontier: NV)
    print("  访问了 \(ebCount) 个顶点")
    print("  耗时: \(String(format: "%.6f", ebTime)) 秒\n")
    
    // 对比
    print("=== 性能对比 ===")
    print("CSR BFS: \(String(format: "%.6f", csrTime)) 秒")
    print("EdgeBlock BFS: \(String(format: "%.6f", ebTime)) 秒")
    let ratio = Double(csrTime) / Double(ebTime)
    print("比值 (CSR / EdgeBlock): \(String(format: "%.2f", ratio))x")
}

main()
