# 多平台 wheel 构建

`axolotl_rs` 现在由 GitHub Actions 为 5 个平台各产出一个 wheel。启用 `abi3-py310` 后，**每个平台只需一个 wheel 即可覆盖 Python 3.10+**，买家不必挑 Python 版本。

## 为什么需要这次改动

以前只在 macOS Apple Silicon 上本地 `maturin develop` 出过一个 `cp313-cp313-macosx_11_0_arm64` wheel。要让 Linux / Windows / Intel Mac 也能装，必须先解决两个真实缺陷：

| 问题 | 修复 |
| --- | --- |
| `Cargo.toml` 里 `memmap2` 和 `pyo3` 被误写在 `[target.'cfg(target_os = "macos")'.dependencies]` 段下面 —— 非 macOS 平台连 `pyo3` 依赖都没有，`python-bindings` feature 根本无法启用 | 把这两项移回 `[dependencies]`，把 Metal 专属的 target 段移到最后 |
| `src/lib.rs` 里 4 个算法模块（`incremental_pagerank` / `incremental_bfs` / `incremental_sssp` / `incremental_sssp_edgeblock`）无条件 `use crate::gpu::GPUAccelerator`，而 `gpu` 模块只在 macOS 编译 | 4 个 `pub mod` + 4 个 `pub use` 全部加 `#[cfg(target_os = "macos")]` |

另外新增 `prototype-rust/pyproject.toml`（maturin 配置）与 `.github/workflows/wheels.yml`。**macOS 上编译出的内容与以前完全一致**，只是把 macOS 专属部分显式门控了。

## 产物矩阵

| 平台 | runner | Rust target | wheel tag |
| --- | --- | --- | --- |
| Linux x86_64 | `ubuntu-latest` | `x86_64-unknown-linux-gnu` | `manylinux_*_x86_64` |
| Linux aarch64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | `manylinux_*_aarch64` |
| macOS Apple Silicon | `macos-15` | `aarch64-apple-darwin` | `macosx_*_arm64` |
| macOS Intel | `macos-15-intel` | `x86_64-apple-darwin` | `macosx_*_x86_64` |
| Windows x86_64 | `windows-latest` | `x86_64-pc-windows-msvc` | `win_amd64` |

全部为 `cp310-abi3`。runner 标签于 2026-11-09 核对：`macos-13` 已退役，macOS 14 镜像于 2026-11-02 退役，故 Intel 用 `macos-15-intel`、arm64 用 `macos-15`。

## 触发方式

- **自动**：push 到 `dev` / `main` 且改动落在 `prototype-rust/**` 或 workflow 文件；对应 PR 也会跑。
- **手动**：Actions → `Build multi-platform wheels` → `Run workflow`。勾选 `create_release` 并填写 `release_tag`（如 `wheels-v0.1.0`）可把全部 wheel 发成 GitHub Release。
- 每个 job 都会装回自己产出的 wheel 并 `import axolotl_rs` 做冒烟测试，然后上传 artifact（`wheels-<target>`）。
- `cpu-check` job 先在 Linux 上跑 `cargo check --features python-bindings --lib`，专门守着「CPU-only 路径不能依赖 Metal」这条约束。

## 拿到 wheel 之后

把对应平台的 `.whl` 放进 `lobster-memory/wheels/` 目录即可。`install.sh` 的查找顺序是「包内 `wheels/` → PyPI → Rust 源码构建」，命中第一个就跳过 Rust 工具链与联网编译。

## 本地验证记录（2026-11-09，离线 Cargo 缓存）

| 变体 | 命令 | 结果 |
| --- | --- | --- |
| A 模拟非 macOS（禁用 macOS cfg + 移除 Metal 依赖） | `cargo check --features python-bindings --lib` | rc=0 |
| B macOS 原生回归 | 同上 | rc=0 |
| C 非 macOS + `abi3-py310` | 同上 | rc=0 |
| 真实构建 | `maturin build --release --features python-bindings` | `axolotl_rs-0.1.0-cp310-abi3-macosx_11_0_arm64.whl`，420 KB |
| API 对比 | `dir(axolotl_rs.AxolotlGraph)` vs 旧 wheel | 新 wheel 为超集（多 `get_edges` / `in_edges` / `out_edges` / `out_edges_cols`），`in_neighbors` 等核心方法齐全 |
| 端到端 | 用新 wheel 跑 `lobster-memory/examples/quickstart.py` | 通过：写入 3 节点 3 边 → 重启后 5 节点 7 边 → 关键词与 valence 召回均命中 |

复现非 macOS 条件（在任意机器上）：

```bash
cp -R prototype-rust /tmp/axolotl-check && cd /tmp/axolotl-check
sed -i 's/#[cfg(target_os = "macos")]/#[cfg(any())]/g' src/lib.rs
# 同时删掉 Cargo.toml 里 [target.'cfg(target_os = "macos")'.dependencies] 段
cargo check --features python-bindings --lib
```

## 本地手动构建

```bash
cd prototype-rust
python -m venv .venv && source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install maturin
maturin build --release --features python-bindings --out dist
```

## 已知边界

- Metal GPU 加速（GPU PageRank / BFS / SSSP）仍是 macOS 专属；其他平台走 CPU 算法路径，功能不受影响，只是没有 GPU 提速。
- `examples/` 里的 GPU 演示程序直接引用 `axolotl_rs::gpu::*`，只在 macOS 可用。CI 只构建 lib（`maturin build`），不编译 examples / benches，因此不受影响。
- Linux aarch64 依赖 `ubuntu-24.04-arm` 原生 runner；公共仓库免费可用。私有仓库若该 runner 不可用，需改用 QEMU 交叉编译。
