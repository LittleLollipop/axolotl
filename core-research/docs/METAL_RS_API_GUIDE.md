# metal-rs API 使用指南

## 关键发现（2026-07-05）

### 问题：程序崩溃（exit code 139）

**原因**：忘记在命令编码器中设置计算管线状态。

**正确的 API 使用顺序**：

```rust
// 1. 创建 Metal 设备
let device = Device::system_default().expect("No Metal device found");

// 2. 创建命令队列
let queue = device.new_command_queue();

// 3. 创建 GPU 内核
let kernel_source = "..."; // Metal 着色器代码
let options = CompileOptions::new();
let library = device.new_library_with_source(kernel_source, &options).unwrap();
let kernel = library.get_function("kernel_name", None).unwrap();
let pipeline = device.new_compute_pipeline_state_with_function(&kernel).unwrap();

// 4. 创建缓冲区
let buffer = device.new_buffer_with_data(
    data.as_ptr() as *const c_void,
    data_len,
    MTLResourceOptions::StorageModeShared, // 适用于 Apple 的 Unified Memory
);

// 5. 编码命令（重要：顺序不能错！）
let command_buffer = queue.new_command_buffer();
let encoder = command_buffer.new_compute_command_encoder();

// ⚠️ 关键：必须设置计算管线状态！
encoder.set_compute_pipeline_state(&pipeline);

// 然后设置缓冲区
encoder.set_buffer(0, Some(&buffer), 0);

// 调度线程
let grid_size = MTLSize::new(n, 1, 1);
let threadgroup_size = MTLSize::new(32, 1, 1); // 线程执行宽度
encoder.dispatch_threads(grid_size, threadgroup_size);

encoder.end_encoding();

// 6. 执行
command_buffer.commit();
command_buffer.wait_until_completed();

// 7. 读取结果
let result_ptr = buffer.contents() as *const f32;
let result_slice = unsafe { std::slice::from_raw_parts(result_ptr, n) };
```

### 常见错误

1. **忘记设置计算管线状态** ❌
   ```rust
   // 错误：没有设置管线状态
   encoder.set_buffer(0, Some(&buffer), 0);
   encoder.dispatch_threads(grid_size, threadgroup_size); // 崩溃！
   ```
   
   ✅ **正确**：
   ```rust
   encoder.set_compute_pipeline_state(&pipeline); // 必须先设置
   encoder.set_buffer(0, Some(&buffer), 0);
   encoder.dispatch_threads(grid_size, threadgroup_size);
   ```

2. **使用错误的缓冲区选项** ❌
   ```rust
   // 错误：使用 CPUCacheModeDefaultCache（可能导致 GPU 无法写入）
   let buffer = device.new_buffer_with_data(..., MTLResourceOptions::CPUCacheModeDefaultCache);
   ```
   
   ✅ **正确**（适用于 Apple 的 Unified Memory）：
   ```rust
   let buffer = device.new_buffer_with_data(..., MTLResourceOptions::StorageModeShared);
   ```

3. **线程调度参数错误** ❌
   ```rust
   // 错误：threadgroup_size 不是线程执行宽度的倍数
   let threadgroup_size = MTLSize::new(10, 1, 1); // 10 不是 32 的倍数
   ```
   
   ✅ **正确**：
   ```rust
   let threadgroup_size = MTLSize::new(pipeline.thread_execution_width() as u64, 1, 1);
   ```

### 正确的 API 调用

| 步骤 | 正确的 API | 说明 |
|------|-----------|------|
| 创建设备 | `Device::system_default()` | 返回 `Option<Device>` |
| 创建队列 | `device.new_command_queue()` | 返回 `CommandQueue` |
| 创建库 | `device.new_library_with_source(source, &options)` | 需要 `CompileOptions` |
| 获取函数 | `library.get_function("name", None)` | 返回 `Result<Function, NSError>` |
| 创建管线 | `device.new_compute_pipeline_state_with_function(&function)` | 返回 `Result<ComputePipelineState, NSError>` |
| 创建缓冲区 | `device.new_buffer_with_data(data, length, options)` | 使用 `StorageModeShared` |
| 设置管线 | `encoder.set_compute_pipeline_state(&pipeline)` | **必须在设置缓冲区之前** |
| 调度线程 | `encoder.dispatch_threads(grid_size, threadgroup_size)` | macOS 10.15+ |

### 测试代码

最小可工作例子：`/tmp/test_metal/src/main.rs`

运行方法：
```bash
cd /tmp/test_metal
cargo run
```

### 应用到 axolotl-rs

需要修复的文件：
1. `src/gpu/mod.rs` - 添加 `encoder.set_compute_pipeline_state(&pipeline);`
2. `src/incremental_pagerank.rs` - 调用 GPU 加速器

### 参考资料

- `metal-rs` 例子：`~/.cargo/registry/src/*/metal-0.29.0/examples/mps/main.rs`
- Apple Metal 文档：https://developer.apple.com/documentation/metal
