#!/usr/bin/env python3
"""Louvain: axolotl-rs vs NetworkX vs iGraph vs Networkit"""

import time, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from bench_comparison import load_edges

# Build wheel first
os.chdir("/tmp/axolotl-rs/prototype-rust")
os.system("/Users/sai/.workbuddy/binaries/python/versions/3.13.12/bin/maturin build --release --features python-bindings 2>/dev/null")
os.system("~/.workbuddy/venvs/lobster-memory/bin/pip install --force-reinstall target/wheels/axolotl_rs-0.1.0-cp313-cp313-macosx_11_0_arm64.whl 2>/dev/null")

os.chdir("/Users/sai/WorkBuddy/点子/performance_test")

def bench_axolotl(name, edges, n_v):
    import axolotl_rs
    g = axolotl_rs.AxolotlGraph.new()
    for i in range(n_v): g.add_vertex(i)
    for u, v in edges:
        g.add_edge(u, v)
        g.add_edge(v, u)
    t0 = time.perf_counter()
    # Louvain not exposed via Python binding — use subprocess to call Rust binary
    elapsed = (time.perf_counter() - t0) * 1000
    return elapsed, 0

def bench_networkx(name, edges, n_v):
    import networkx as nx
    from networkx.algorithms.community import louvain_communities
    g = nx.Graph()
    g.add_nodes_from(range(n_v))
    g.add_edges_from(edges)
    t0 = time.perf_counter()
    comms = louvain_communities(g)
    elapsed = (time.perf_counter() - t0) * 1000
    return elapsed, len(comms)

def bench_igraph(name, edges, n_v):
    import igraph
    g = igraph.Graph(directed=False)
    g.add_vertices(n_v)
    g.add_edges(edges)
    t0 = time.perf_counter()
    comms = g.community_multilevel()
    elapsed = (time.perf_counter() - t0) * 1000
    return elapsed, len(comms)

def bench_networkit(name, edges, n_v):
    import networkit as nk
    g = nk.Graph(n_v, directed=False)
    for u, v in edges:
        g.addEdge(u, v)
    t0 = time.perf_counter()
    cd = nk.community.PLM(g)
    cd.run()
    elapsed = (time.perf_counter() - t0) * 1000
    return elapsed, cd.numberOfSubsets()

def main():
    # First get axolotl-rs numbers from Rust benchmark
    import subprocess
    os.chdir("/tmp/axolotl-rs/prototype-rust")
    result = subprocess.run(
        ["cargo", "run", "--release", "--example", "bench_louvain"],
        capture_output=True, text=True, timeout=300
    )
    ax_data = {}
    for line in result.stdout.split('\n'):
        if '|' in line and 'ms' in line and 'Dataset' not in line:
            parts = [p.strip() for p in line.split('|') if p.strip()]
            if len(parts) >= 5 and parts[0] != '---------':
                name = parts[0]
                time_str = parts[3].replace('ms', '')
                comms_str = parts[4]
                try:
                    ax_data[name] = (float(time_str), int(comms_str))
                except: pass

    os.chdir("/Users/sai/WorkBuddy/点子/performance_test")

    datasets = [
        ("soc-Epinions1", "datasets/soc_epinions1_100K.edgelist"),
        ("com-DBLP", "datasets/com_dblp_100K.edgelist"),
        ("RMAT scale 20", "datasets/rmat_1M_100K.edgelist"),
    ]

    print("# Louvain Community Detection — Library Comparison\n")
    print(f"**Hardware**: Apple M4\n")
    print("| Dataset | V | axolotl-rs Louvain | NetworkX | iGraph (C) | Networkit (C++) |")
    print("|---------|:---:|:---:|:---:|:---:|:---:|")

    for name, path in datasets:
        if not os.path.exists(path): continue
        edges, n_v = load_edges(path)
        v_str = f"{n_v//1000}K" if n_v < 1_000_000 else f"{n_v/1e6:.1f}M"

        t_nx, c_nx = bench_networkx(name, edges, n_v)
        t_ig, c_ig = bench_igraph(name, edges, n_v)
        t_nk, c_nk = bench_networkit(name, edges, n_v)

        ax_str = "—"
        if name in ax_data:
            t_ax, c_ax = ax_data[name]
            ax_str = f"{t_ax:.0f}ms ({c_ax})"

        print(f"| {name} | {v_str} | {ax_str} | {t_nx:.0f}ms ({c_nx}) | {t_ig:.0f}ms ({c_ig}) | {t_nk:.0f}ms ({c_nk}) |")

if __name__ == "__main__":
    main()
