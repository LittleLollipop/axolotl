#!/bin/bash
# 测试增量 PageRank 的正确性

echo "=== 编译测试程序 ==="
cargo build --example test_incremental_pr

echo ""
echo "=== 运行测试程序 ==="
cargo run --example test_incremental_pr

echo ""
echo "=== 运行单元测试 ==="
cargo test test_incremental_pagerank -- --nocapture
