// src/py_bindings.rs
// Python bindings via PyO3
//
// Usage (Python):
//   import axolotl_rs
//   g = axolotl_rs.AxolotlGraph.open("data/graph.axeb")
//   g.add_vertex(42, {"name": "test", "score": 3.14})
//   pr = g.pagerank(50)
//   g.save()

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use pyo3::IntoPyObject;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::graph_db::{GraphDB, GraphMode};
use crate::gpu_edge_block::EdgeData;
use crate::PropertyValue;

// ── Python 模块 ─────────────────────────

#[pymodule]
pub fn axolotl_rs(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<AxolotlGraph>()?;
    Ok(())
}

// ── AxolotlGraph 包装 ─────────────────────────

#[pyclass(name = "AxolotlGraph")]
struct AxolotlGraph {
    db: Mutex<GraphDB>,
}

#[pymethods]
impl AxolotlGraph {
    /// 打开已有图文件（不存在则创建空库）
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        let db = GraphDB::from_file_or_new(path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(format!("{:?}", e)))?;
        Ok(AxolotlGraph { db: Mutex::new(db) })
    }

    /// 创建空图
    #[staticmethod]
    fn new() -> Self {
        AxolotlGraph {
            db: Mutex::new(GraphDB::new(GraphMode::InMemory)),
        }
    }

    // ── 统计 ──

    fn vertex_count(&self) -> usize {
        self.db.lock().unwrap().vertex_count()
    }

    fn edge_count(&self) -> usize {
        self.db.lock().unwrap().edge_count()
    }

    // ── 顶点 CRUD ──

    /// add_vertex(id, properties_dict)
    fn add_vertex(&self, id: u64, props: Option<Bound<'_, PyDict>>) -> PyResult<u64> {
        let props = dict_to_props(props);
        self.db.lock().unwrap()
            .add_vertex(id, props)
            .map_err(to_py_err)?;
        Ok(id)
    }

    /// get_vertex(id) → properties_dict or None
    fn get_vertex<'py>(&self, py: Python<'py>, id: u64) -> PyResult<Option<Bound<'py, PyDict>>> {
        let db = self.db.lock().unwrap();
        match db.get_vertex(id) {
            Some(props) => {
                let d = PyDict::new(py);
                for (k, v) in &props {
                    d.set_item(k, prop_to_py(py, v))?;
                }
                Ok(Some(d))
            }
            _ => Ok(None),
        }
    }

    /// delete_vertex(id) → removed properties or None
    fn delete_vertex<'py>(&self, py: Python<'py>, id: u64) -> PyResult<Option<Bound<'py, PyDict>>> {
        let removed = self.db.lock().unwrap().delete_vertex(id)
            .map_err(to_py_err)?;
        match removed {
            Some(props) => {
                let d = PyDict::new(py);
                for (k, v) in &props {
                    d.set_item(k, prop_to_py(py, v))?;
                }
                Ok(Some(d))
            }
            None => Ok(None),
        }
    }

    // ── 边 CRUD ──

    /// add_edge(from, to, weight=1.0, properties=None)
    #[pyo3(signature = (from_id, to_id, weight=1.0, props=None))]
    fn add_edge(
        &self, from_id: u64, to_id: u64, weight: f64,
        props: Option<Bound<'_, PyDict>>,
    ) -> PyResult<(u64, u64)> {
        let props = dict_to_props(props);
        self.db.lock().unwrap()
            .add_edge(from_id, to_id, weight, props)
            .map_err(to_py_err)?;
        Ok((from_id, to_id))
    }

    /// get_edge(from, to) → (weight, properties_dict) or None
    fn get_edge<'py>(&self, py: Python<'py>, from_id: u64, to_id: u64) -> PyResult<Option<Bound<'py, PyTuple>>> {
        match self.db.lock().unwrap().get_edge(from_id, to_id) {
            Some((w, props)) => {
                let d = PyDict::new(py);
                for (k, v) in &props {
                    d.set_item(k, prop_to_py(py, v))?;
                }
                let tup = PyTuple::new(py, [w.to_object(py), d.into_any().to_object(py)])?;
                Ok(Some(tup))
            }
            None => Ok(None),
        }
    }

    /// remove_edge(from, to)
    fn remove_edge(&self, from_id: u64, to_id: u64) -> PyResult<bool> {
        Ok(self.db.lock().unwrap().delete_edge(from_id, to_id).is_ok())
    }

    // ── 邻居查询 ──

    /// out_neighbors(id) → [neighbor_id, ...]
    fn out_neighbors(&self, id: u64) -> Vec<u64> {
        self.db.lock().unwrap().out_neighbors(id)
    }

    // ── 算法 ──

    /// pagerank(iterations=100, damping=0.85) → dict[id → score]
    fn pagerank<'py>(&self, py: Python<'py>, iterations: Option<usize>, damping: Option<f64>) -> PyResult<Bound<'py, PyDict>> {
        let iters = iterations.unwrap_or(100);
        let d = damping.unwrap_or(0.85) as f32;

        let db = self.db.lock().unwrap();
        let eb = db.edgeblock().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Graph not in InMemory mode")
        })?;

        let n = eb.vertex_count as usize;
        let mut pr = vec![1.0 / n as f32; n];

        // Find dangling vertices (out-degree == 0)
        let out_degrees: Vec<u32> = eb.compute_out_degrees();
        let dangling: Vec<usize> = (0..n).filter(|&v| out_degrees[v] == 0).collect();

        for _ in 0..iters {
            let dangling_sum: f32 = dangling.iter().map(|&v| pr[v]).sum();
            let dangling_contrib = dangling_sum / n as f32;
            let mut new_pr = vec![(1.0 - d) / n as f32; n];

            // For each vertex, sum contributions from in-neighbors
            for v_idx in 0..n {
                let mut contrib = 0.0f32;
                for src_id in eb.in_neighbors_by_idx(v_idx) {
                    if let Some(&src_idx) = eb.id_to_idx.get(&src_id) {
                        let od = out_degrees[src_idx];
                        if od > 0 {
                            contrib += pr[src_idx] / od as f32;
                        }
                    }
                }
                new_pr[v_idx] += d * (contrib + dangling_contrib);
            }
            pr = new_pr;
        }

        let result = PyDict::new(py);
        for (i, &score) in pr.iter().enumerate() {
            if i < eb.idx_to_id.len() && eb.idx_to_id[i] != u64::MAX {
                result.set_item(eb.idx_to_id[i], score)?;
            }
        }
        Ok(result)
    }

    /// bfs(source) → dict[id → distance]
    fn bfs<'py>(&self, py: Python<'py>, source: u64) -> PyResult<Bound<'py, PyDict>> {
        let db = self.db.lock().unwrap();
        let eb = db.edgeblock().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Graph not in InMemory mode")
        })?;

        use std::collections::VecDeque;
        let mut dist: HashMap<u64, u32> = HashMap::new();
        let mut queue = VecDeque::new();

        if eb.id_to_idx.contains_key(&source) {
            dist.insert(source, 0);
            queue.push_back(source);
        }

        while let Some(v) = queue.pop_front() {
            let d = dist[&v] + 1;
            for nbr in eb.out_neighbors(v) {
                if !dist.contains_key(&nbr) {
                    dist.insert(nbr, d);
                    queue.push_back(nbr);
                }
            }
        }

        let d = PyDict::new(py);
        for (v, dist_val) in &dist {
            d.set_item(*v, *dist_val)?;
        }
        Ok(d)
    }

    // ── 遍历 ──

    /// walk(start, max_depth) → [visited_vertices]
    fn walk(&self, start: u64, max_depth: usize) -> PyResult<Vec<u64>> {
        let db = self.db.lock().unwrap();
        Ok(db.walk(start, max_depth, |_, _, _| {}))
    }

    /// subgraph(seeds, max_depth) → (vertices, edges)
    /// edges = [(from, to, weight), ...]
    fn subgraph<'py>(
        &self, py: Python<'py>, seeds: Vec<u64>, max_depth: usize,
    ) -> PyResult<(Vec<u64>, Vec<Bound<'py, PyTuple>>)> {
        let (vertices, edges) = self.db.lock().unwrap().subgraph(&seeds, max_depth);
        let py_edges: Vec<_> = edges.iter().map(|(f, t, ed)| {
            let w: f64 = ed.as_ref().map(|e| e.weight as f64).unwrap_or(1.0);
            PyTuple::new(py, [f.to_object(py), t.to_object(py), w.to_object(py)])
        }).collect::<Result<Vec<_>, _>>()?;
        Ok((vertices, py_edges))
    }

    /// find_paths(path_length, max_results=100) → [[vertex_id, ...], ...]
    fn find_paths(&self, path_length: usize, max_results: Option<usize>) -> Vec<Vec<u64>> {
        let db = self.db.lock().unwrap();
        let paths = db.find_paths(
            None::<&dyn Fn(u64) -> bool>,
            None::<&dyn Fn(&EdgeData) -> bool>,
            path_length,
        );
        paths.into_iter().take(max_results.unwrap_or(100)).collect()
    }

    // ── 持久化 ──

    fn save(&self) -> PyResult<String> {
        let db = self.db.lock().unwrap();
        let path = db.file_path()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "graph.axeb".to_string());
        db.save_to(&path).map_err(to_py_err)?;
        Ok(path)
    }

    fn save_to(&self, path: &str) -> PyResult<()> {
        self.db.lock().unwrap().save_to(path).map_err(to_py_err)
    }

    fn close(&self) -> PyResult<()> {
        let db = self.db.lock().unwrap();
        if db.file_path().is_some() {
            db.save_to_file().map_err(to_py_err)?;
        }
        Ok(())
    }

    // ── 批量导入 ──

    /// batch_add_edges([(from, to, weight), ...])
    fn batch_add_edges(&self, edges: Vec<(u64, u64, f64)>) -> PyResult<usize> {
        let mut db = self.db.lock().unwrap();
        let mut count = 0;
        for (from, to, weight) in edges {
            db.add_edge(from, to, weight, HashMap::new()).map_err(to_py_err)?;
            count += 1;
        }
        Ok(count)
    }

    fn __repr__(&self) -> String {
        let db = self.db.lock().unwrap();
        format!("AxolotlGraph(vertices={}, edges={}, path={:?})",
            db.vertex_count(), db.edge_count(), db.file_path())
    }
}

// ── 类型转换 ─────────────────────────

fn dict_to_props(dict: Option<Bound<'_, PyDict>>) -> HashMap<String, PropertyValue> {
    let mut props = HashMap::new();
    if let Some(d) = dict {
        for (key, val) in d.iter() {
            if let Ok(k) = key.extract::<String>() {
                props.insert(k, py_to_prop(&val));
            }
        }
    }
    props
}

fn py_to_prop(val: &Bound<'_, PyAny>) -> PropertyValue {
    if let Ok(i) = val.extract::<i64>() { return PropertyValue::Int(i); }
    if let Ok(f) = val.extract::<f64>() { return PropertyValue::Double(f); }
    if let Ok(s) = val.extract::<String>() { return PropertyValue::String(s); }
    if let Ok(b) = val.extract::<bool>() { return PropertyValue::Bool(b); }
    PropertyValue::Null
}

fn prop_to_py(py: Python<'_>, val: &PropertyValue) -> PyObject {
    match val {
        PropertyValue::Int(i)     => i.into_py(py),
        PropertyValue::Double(f)  => f.into_py(py),
        PropertyValue::String(s)  => s.clone().into_py(py),
        PropertyValue::Bool(b)    => b.into_py(py),
        PropertyValue::Null       => py.None(),
    }
}

fn to_py_err(e: impl std::fmt::Debug) -> PyErr {
    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{:?}", e))
}
