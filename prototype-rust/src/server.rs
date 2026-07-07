// src/server.rs
// Axolotl 图数据库 REST API 服务器
//
// 零外部依赖：纯 std::net + serde_json（已有依赖）。
// 支持 CRUD（顶点/边）、邻居查询、PageRank/BFS 算法。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crate::graph_db::{GraphDB, GraphMode};
use crate::PropertyValue;

// ── 线程池（微型，std-only）─────────────────

const POOL_SIZE: usize = 16;

struct ThreadPool {
    workers: Vec<thread::JoinHandle<()>>,
    sender: std::sync::mpsc::Sender<Box<dyn FnOnce() + Send + 'static>>,
}

impl ThreadPool {
    fn new(size: usize) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<Box<dyn FnOnce() + Send + 'static>>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            let receiver = Arc::clone(&receiver);
            workers.push(thread::spawn(move || {
                loop {
                    let job = {
                        let rx = receiver.lock().unwrap();
                        rx.recv()
                    };
                    match job {
                        Ok(job) => job(),
                        Err(_) => break,
                    }
                }
            }));
        }

        ThreadPool { workers, sender }
    }

    fn execute<F>(&self, f: F)
    where F: FnOnce() + Send + 'static
    {
        let _ = self.sender.send(Box::new(f));
    }
}

// ── 共享状态 ─────────────────────────

pub type SharedGraph = Arc<RwLock<GraphDB>>;

/// 全局 shutdown 标志
static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);

// ── HTTP 框架（微型）────────────────────

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn parse_request(stream: &mut TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut req_line = String::new();
    reader.read_line(&mut req_line).ok()?;
    let parts: Vec<&str> = req_line.trim().split_whitespace().collect();
    if parts.len() < 2 { return None; }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    // 读 headers 直到空行
    let mut content_len = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let trimmed = line.trim();
        if trimmed.is_empty() { break; }
        if let Some(v) = trimmed.strip_prefix("Content-Length:").or_else(|| trimmed.strip_prefix("content-length:")) {
            content_len = v.trim().parse().unwrap_or(0);
        }
    }

    // 读 body
    let mut body = vec![0u8; content_len];
    if content_len > 0 {
        reader.read_exact(&mut body).ok()?;
    }

    Some(Request { method, path, body })
}

fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
        status, status_text(status), content_type, body.len(), body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

fn json_ok(status: u16, json: serde_json::Value) -> String {
    json.to_string()
}

// ── 路由 ─────────────────────────

fn handle_request(req: Request, state: &SharedGraph) -> (u16, String) {
    // CORS preflight
    if req.method == "OPTIONS" {
        return (200, "{}".to_string());
    }

    let path = req.path.clone();
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    match (req.method.as_str(), segments.as_slice()) {
        ("GET", ["health"]) => (200, r#"{"status":"ok","name":"Axolotl GraphDB"}"#.to_string()),
        ("GET", ["stats"]) => handle_stats(state),

        ("POST", ["vertices"]) => handle_add_vertex(&req.body, state),
        ("GET", ["vertices", id]) => handle_get_vertex(id, state),
        ("DELETE", ["vertices", id]) => handle_delete_vertex(id, state),

        ("POST", ["edges"]) => handle_add_edge(&req.body, state),
        ("DELETE", ["edges", from, to]) => handle_delete_edge(from, to, state),

        ("GET", ["neighbors", id]) => handle_out_neighbors(id, state),
        ("GET", ["neighbors", id, "in"]) => handle_in_neighbors(id, state),

        ("POST", ["algorithms", "pagerank"]) => handle_pagerank(&req.body, state),
        ("POST", ["algorithms", "bfs"]) => handle_bfs(&req.body, state),

        ("POST", ["traverse", "walk"]) => handle_walk(&req.body, state),
        ("POST", ["traverse", "subgraph"]) => handle_subgraph(&req.body, state),
        ("POST", ["traverse", "find_paths"]) => handle_find_paths(&req.body, state),

        ("POST", ["admin", "save"]) => handle_admin_save(state),
        ("POST", ["admin", "shutdown"]) => handle_admin_shutdown(state),

        _ => (404, json_err("not found")),
    }
}

fn json_err(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}
fn json_err_fmt<E: std::fmt::Debug>(e: E) -> String {
    serde_json::json!({ "error": format!("{:?}", e) }).to_string()
}

fn parse_id(s: &str) -> Result<u64, String> {
    s.parse().map_err(|_| "invalid id".to_string())
}

// ── Stats ─────────────────────────

fn handle_stats(state: &SharedGraph) -> (u16, String) {
    let g = state.read().unwrap();
    let mode = match g.mode() {
        GraphMode::InMemory => "in_memory",
        GraphMode::Mmap => "mmap",
    };
    (200, serde_json::json!({
        "vertex_count": g.vertex_count(),
        "edge_count": g.edge_count(),
        "mode": mode
    }).to_string())
}

// ── 顶点 ─────────────────────────

fn handle_add_vertex(body: &[u8], state: &SharedGraph) -> (u16, String) {
    let v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return (400, json_err("invalid JSON")),
    };
    let id = match v.get("id").and_then(|i| i.as_u64()) {
        Some(id) => id,
        None => return (400, json_err("missing 'id' field")),
    };
    let mut props = HashMap::new();
    if let Some(obj) = v.get("properties").and_then(|p| p.as_object()) {
        for (k, val) in obj {
            props.insert(k.clone(), json_to_property(val));
        }
    }
    match state.write().unwrap().add_vertex(id, props) {
        Ok(()) => (201, serde_json::json!({ "id": id, "status": "created" }).to_string()),
        Err(e) => (400, json_err_fmt(e)),
    }
}

fn handle_get_vertex(id_str: &str, state: &SharedGraph) -> (u16, String) {
    let id = match parse_id(id_str) { Ok(i) => i, Err(e) => return (400, json_err(&e)) };
    let g = state.read().unwrap();
    match g.get_vertex(id) {
        Some(props) => {
            let json_props: serde_json::Map<String, serde_json::Value> = props.into_iter()
                .map(|(k, v)| (k, property_to_json(v)))
                .collect();
            (200, serde_json::json!({ "id": id, "properties": json_props }).to_string())
        }
        None => (404, json_err(&format!("vertex {} not found", id))),
    }
}

fn handle_delete_vertex(id_str: &str, state: &SharedGraph) -> (u16, String) {
    let id = match parse_id(id_str) { Ok(i) => i, Err(e) => return (400, json_err(&e)) };
    match state.write().unwrap().delete_vertex(id) {
        Ok(Some(_)) => (200, serde_json::json!({ "id": id, "status": "deleted" }).to_string()),
        Ok(None) => (404, json_err(&format!("vertex {} not found", id))),
        Err(e) => (400, json_err(&format!("{:?}", e))),
    }
}

// ── 边 ─────────────────────────

fn handle_add_edge(body: &[u8], state: &SharedGraph) -> (u16, String) {
    let v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return (400, json_err("invalid JSON")),
    };
    let from = match v.get("from").and_then(|f| f.as_u64()) {
        Some(f) => f, None => return (400, json_err("missing 'from'")),
    };
    let to = match v.get("to").and_then(|t| t.as_u64()) {
        Some(t) => t, None => return (400, json_err("missing 'to'")),
    };
    let weight = v.get("weight").and_then(|w| w.as_f64()).unwrap_or(1.0);

    match state.write().unwrap().add_edge(from, to, weight, HashMap::new()) {
        Ok(()) => (201, serde_json::json!({ "from": from, "to": to, "status": "created" }).to_string()),
        Err(e) => (400, json_err(&format!("{:?}", e))),
    }
}

fn handle_delete_edge(from_str: &str, to_str: &str, state: &SharedGraph) -> (u16, String) {
    let from = match parse_id(from_str) { Ok(i) => i, Err(e) => return (400, json_err(&e)) };
    let to = match parse_id(to_str) { Ok(i) => i, Err(e) => return (400, json_err(&e)) };
    match state.write().unwrap().delete_edge(from, to) {
        Ok(Some(_)) => (200, serde_json::json!({ "from": from, "to": to, "status": "deleted" }).to_string()),
        Ok(None) => (404, json_err(&format!("edge {}->{} not found", from, to))),
        Err(e) => (400, json_err(&format!("{:?}", e))),
    }
}

// ── 邻居 ─────────────────────────

fn handle_out_neighbors(id_str: &str, state: &SharedGraph) -> (u16, String) {
    let id = match parse_id(id_str) { Ok(i) => i, Err(e) => return (400, json_err(&e)) };
    let g = state.read().unwrap();
    let neighbors = g.out_neighbors(id);
    (200, serde_json::json!({ "vertex": id, "neighbors": neighbors }).to_string())
}

fn handle_in_neighbors(id_str: &str, state: &SharedGraph) -> (u16, String) {
    let id = match parse_id(id_str) { Ok(i) => i, Err(e) => return (400, json_err(&e)) };
    let g = state.read().unwrap();
    let neighbors = g.edgeblock()
        .map(|eb| eb.in_neighbors(id))
        .unwrap_or_default();
    (200, serde_json::json!({ "vertex": id, "neighbors": neighbors }).to_string())
}

// ── PageRank ─────────────────────────

fn handle_pagerank(body: &[u8], state: &SharedGraph) -> (u16, String) {
    let mut iterations = 100usize;
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(body) {
        if let Some(i) = v.get("iterations").and_then(|i| i.as_u64()) {
            iterations = i as usize;
        }
    }
    let csr = {
        let g = state.read().unwrap();
        g.to_csr()
    }; // 释放锁，算法在锁外计算
    let pr = crate::pagerank_correct::compute_pagerank_cpu(&csr, iterations);
    let sum: f32 = pr.iter().sum();
    let scores: Vec<serde_json::Value> = pr.iter().enumerate()
        .take(20) // top 20 only
        .map(|(i, &s)| serde_json::json!({ "vertex": i, "score": s }))
        .collect();

    (200, serde_json::json!({
        "iterations": iterations,
        "sum": sum,
        "top20": scores
    }).to_string())
}

// ── BFS ─────────────────────────

fn handle_bfs(body: &[u8], state: &SharedGraph) -> (u16, String) {
    let source = if let Ok(v) = serde_json::from_slice::<serde_json::Value>(body) {
        v.get("source").and_then(|s| s.as_u64()).unwrap_or(0)
    } else { 0u64 };

    use std::collections::VecDeque;
    let g = state.read().unwrap();
    let eb = match g.edgeblock() {
        Some(eb) => eb,
        None => return (400, json_err("BFS requires InMemory mode")),
    };

    let n = eb.vertex_count as usize;
    let mut dist = vec![u32::MAX; n];
    let mut queue = VecDeque::new();

    if let Some(&start_idx) = eb.id_to_idx.get(&source) {
        dist[start_idx] = 0;
        queue.push_back(start_idx);

        while let Some(v_idx) = queue.pop_front() {
            let d = dist[v_idx] + 1;
            let first = eb.vertices[v_idx] as usize;
            let count = eb.block_counts[v_idx] as usize;
            for b in 0..count {
                let off = (first + b) * crate::gpu_edge_block::GPUEdgeBlockGraph::BLOCK_SIZE_U32;
                let ec = eb.blocks[off + 1] as usize;
                for e in 0..ec {
                    let nbr = eb.blocks[off + 2 + e] as usize;
                    if nbr < n && dist[nbr] == u32::MAX {
                        dist[nbr] = d;
                        queue.push_back(nbr);
                    }
                }
            }
        }
    }

    let mut reached = 0usize;
    let mut results = Vec::new();
    for i in 0..n {
        if dist[i] != u32::MAX {
            results.push(serde_json::json!({ "vertex": eb.idx_to_id[i], "distance": dist[i] }));
            reached += 1;
        }
    }

    (200, serde_json::json!({
        "source": source,
        "reached": reached,
        "distances": results
    }).to_string())
}

// ── 增强遍历 ─────────────────────────

fn handle_walk(body: &[u8], state: &SharedGraph) -> (u16, String) {
    let v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v, Err(_) => return (400, json_err("invalid JSON")),
    };
    let start = v.get("start").and_then(|s| s.as_u64()).unwrap_or(0);
    let max_depth = v.get("max_depth").and_then(|d| d.as_u64()).unwrap_or(3) as usize;

    let g = state.read().unwrap();
    let mut edges_seen = Vec::new();
    let visited = g.walk(start, max_depth, |from, depth, to| {
        edges_seen.push(serde_json::json!({ "from": from, "depth": depth, "to": to }));
    });

    (200, serde_json::json!({
        "start": start, "max_depth": max_depth,
        "visited": visited,
        "edges_traversed": edges_seen
    }).to_string())
}

fn handle_subgraph(body: &[u8], state: &SharedGraph) -> (u16, String) {
    let v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v, Err(_) => return (400, json_err("invalid JSON")),
    };
    let seeds: Vec<u64> = v.get("seeds").and_then(|s| s.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_u64()).collect())
        .unwrap_or_default();
    let max_depth = v.get("max_depth").and_then(|d| d.as_u64()).unwrap_or(2) as usize;

    let g = state.read().unwrap();
    let (vertices, edges) = g.subgraph(&seeds, max_depth);

    let edge_list: Vec<serde_json::Value> = edges.iter().map(|(f, t, ed)| {
        let mut obj = serde_json::json!({ "from": f, "to": t });
        if let Some(data) = ed {
            obj["weight"] = serde_json::json!(data.weight);
        }
        obj
    }).collect();

    (200, serde_json::json!({
        "seeds": seeds, "max_depth": max_depth,
        "vertex_count": vertices.len(), "vertices": vertices,
        "edge_count": edges.len(), "edges": edge_list
    }).to_string())
}

fn handle_find_paths(body: &[u8], state: &SharedGraph) -> (u16, String) {
    let v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v, Err(_) => return (400, json_err("invalid JSON")),
    };
    let path_length = v.get("path_length").and_then(|l| l.as_u64()).unwrap_or(2) as usize;
    let max_results = v.get("max_results").and_then(|m| m.as_u64()).unwrap_or(100) as usize;

    let g = state.read().unwrap();
    let paths = g.find_paths(
        None::<&dyn Fn(u64) -> bool>,
        None::<&dyn Fn(&crate::gpu_edge_block::EdgeData) -> bool>,
        path_length,
    );

    let limited: Vec<_> = paths.iter().take(max_results).collect();

    (200, serde_json::json!({
        "path_length": path_length,
        "total_found": paths.len(),
        "returned": limited.len(),
        "paths": limited
    }).to_string())
}

// ── 管理端点 ─────────────────────────

fn handle_admin_save(state: &SharedGraph) -> (u16, String) {
    let path = state.read().unwrap().file_path().map(|s| s.to_string());
    match path {
        Some(p) => {
            match state.read().unwrap().save_to_file() {
                Ok(()) => (200, serde_json::json!({ "status": "saved", "path": p }).to_string()),
                Err(e) => (500, serde_json::json!({ "error": format!("{:?}", e) }).to_string()),
            }
        }
        None => (400, json_err("no data file path configured")),
    }
}

fn handle_admin_shutdown(state: &SharedGraph) -> (u16, String) {
    let path = state.read().unwrap().file_path().map(|s| s.to_string());
    SHUTDOWN_FLAG.store(true, Ordering::Relaxed);
    (200, serde_json::json!({ "status": "shutting_down", "path": path }).to_string())
}

// ── PropertyValue 转换 ─────────────────────────

fn json_to_property(val: &serde_json::Value) -> PropertyValue {
    match val {
        serde_json::Value::String(s) => PropertyValue::String(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { PropertyValue::Int(i) }
            else if let Some(f) = n.as_f64() { PropertyValue::Double(f) }
            else { PropertyValue::Null }
        }
        serde_json::Value::Bool(b) => PropertyValue::Bool(*b),
        _ => PropertyValue::Null,
    }
}

fn property_to_json(pv: PropertyValue) -> serde_json::Value {
    match pv {
        PropertyValue::String(s) => serde_json::Value::String(s),
        PropertyValue::Int(i) => serde_json::json!(i),
        PropertyValue::Double(d) => serde_json::json!(d),
        PropertyValue::Bool(b) => serde_json::Value::Bool(b),
        PropertyValue::Null => serde_json::Value::Null,
    }
}

// ── 启动 ─────────────────────────

/// 启动 HTTP 服务器（每连接一个线程）
pub fn serve(addr: &str, graph: GraphDB) {
    let state: SharedGraph = Arc::new(RwLock::new(graph));
    let listener = TcpListener::bind(addr).expect("Failed to bind");

    println!("🚀 Axolotl GraphDB server on http://{}", addr);
    println!("   GET  /health");
    println!("   GET  /stats");
    println!("   POST /vertices    {{\"id\": N, \"properties\": {{...}}}}");
    println!("   GET  /vertices/:id");
    println!("   DELETE /vertices/:id");
    println!("   POST /edges       {{\"from\": N, \"to\": M, \"weight\": 1.0}}");
    println!("   DELETE /edges/:from/:to");
    println!("   GET  /neighbors/:id");
    println!("   GET  /neighbors/:id/in");
    println!("   POST /algorithms/pagerank {{\"iterations\": 100}}");
    println!("   POST /algorithms/bfs      {{\"source\": 0}}");
    println!("   POST /admin/save     手动保存");
    println!("   POST /admin/shutdown  安全关闭（自动保存）");
    println!();

    SHUTDOWN_FLAG.store(false, Ordering::Relaxed);
    let pool = ThreadPool::new(POOL_SIZE);

    listener.set_nonblocking(true).ok();

    loop {
        if SHUTDOWN_FLAG.load(Ordering::Relaxed) {
            // 最后一次保存
            let g = state.read().unwrap();
            match g.save_to_file() {
                Ok(()) => println!("[shutdown] Saved."),
                Err(e) => eprintln!("[shutdown] Save failed: {:?}", e),
            }
            break;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let state = state.clone();
                pool.execute(move || {
                    let req = match parse_request(&mut stream) {
                        Some(r) => r,
                        None => {
                            respond(&mut stream, 400, "application/json", &json_err("bad request"));
                            return;
                        }
                    };
                    if SHUTDOWN_FLAG.load(Ordering::Relaxed) { return; }
                    let (status, body) = handle_request(req, &state);
                    respond(&mut stream, status, "application/json", &body);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            Err(e) => eprintln!("Accept error: {}", e),
        }
    }

    println!("[shutdown] Server stopped.");
}
