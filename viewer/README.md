# Axolotl Viewer — 图库查看器

macOS 桌面应用，用于**只读浏览** Axolotl 图库（`.axeb`）内容：统计概览、节点列表与搜索、力导向图可视化、节点属性与邻居详情。

技术栈：**Tauri 2**（Rust 后端直接 link `axolotl-rs` 引擎，无 HTTP 层）+ WebView 前端（cytoscape.js 图可视化，已本地 vendor，离线可用）。

## 快速开始

```bash
# 1. 构建引擎（首次较慢，含 Metal/算法模块）
cd ../prototype-rust && cargo build

# 2. 开发模式（热更新）
cd .. && cargo tauri dev

# 3. 打包 .app
cargo tauri build
```

产物：`target/release/bundle/macos/Axolotl Viewer.app`，双击即用。

## 使用

- 点「打开图库…」选择 `.axeb` 文件，或直接把文件拖进窗口
- 启动视图：`lobster_root` 及其 2 跳子图（无 root 则取入度最高的节点）
- 单击节点：右侧查看属性与出/入邻居；双击节点：展开其 1 跳邻居
- 左栏搜索：按 `id` / `label` 子串过滤，可叠加 domain/type 过滤；点列表项定位到图
- 全部只读：打开使用 `open_without_recovery`，不触发任何写入/清理

## 结构

```
viewer/
├── src/lib.rs        # Tauri 命令：open_graph（全量快照导出）
├── ui/               # 前端（纯静态，无构建步骤）
│   ├── index.html
│   ├── main.js
│   ├── style.css
│   └── vendor/cytoscape.min.js   # 本地 vendored，离线可用
└── tauri.conf.json
```

## 引擎贡献

修复了 `prototype-rust/src/graph_db.rs` 中 `iter_edges()` 未返回边权重/属性的问题（现在从 `edge_data` 读取真实值），viewer 与导出场景依赖此修复。
