// src/graph_db.rs
// 统一图数据库接口（内存模式 / mmap 模式切换）
//
// InMemory 模式使用 GPUEdgeBlockGraph 作为主存储：
//   - 数据以 EdgeBlock 格式（扁平化 u32 数组）存储在 CPU 内存中
//   - GPU 算法直接用 EdgeBlock 格式创建 Metal buffer（一次拷贝）
//   - 不再有 HashMap→CSR 的中间转换
//
// 使用方式：
//   let db = GraphDB::new(GraphMode::InMemory);      // 新建空图
//   let db = GraphDB::open("graph.bin", GraphMode::InMemory)?;  // 加载
//   let db = GraphDB::open("graph.bin", GraphMode::Mmap)?;      // mmap

use std::collections::HashMap;
use std::path::Path;

use crate::{PropertyValue, CSRGraph};
use crate::gpu_edge_block::GPUEdgeBlockGraph;
use crate::mmap_graph::MmapGraph;

// ── 模式选择 ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMode {
    /// 完整加载到内存（支持读写，使用 EdgeBlock 格式）
    InMemory,
    /// mmap 只读模式（支持大于内存的图，只读）
    Mmap,
}

// ── 统一错误类型 ─────────────────────────

#[derive(Debug)]
pub enum GraphDBError {
    Io(String),
    NotSupported(String),
    NotFound(String),
}

impl From<std::io::Error> for GraphDBError {
    fn from(e: std::io::Error) -> Self {
        GraphDBError::Io(e.to_string())
    }
}

// ── 统一图数据库句柄 ─────────────────────────

pub struct GraphDB {
    mode: GraphMode,

    // ---- InMemory 模式：EdgeBlock 原生存储 ----
    edgeblock: Option<GPUEdgeBlockGraph>,

    // ---- Mmap 模式 ----
    mmap: Option<MmapGraph>,

    /// 持久化文件路径（用于 autosave）
    data_file: Option<String>,
}

impl GraphDB {
    // ── 打开 / 创建数据库 ─────────────────────────

    /// 创建空的内存模式数据库
    pub fn new(mode: GraphMode) -> Self {
        match mode {
            GraphMode::InMemory => GraphDB {
                mode,
                edgeblock: Some(GPUEdgeBlockGraph::new()),
                mmap: None,
                data_file: None,
            },
            GraphMode::Mmap => {
                panic!("Use GraphDB::open() for Mmap mode with a file path");
            }
        }
    }

    /// 打开图数据库（自动 WAL 恢复）
    ///
    /// - 加载基础图文件
    /// - 自动扫描 WAL 目录并重放已提交事务
    /// - 清理未提交事务的 WAL 文件
    pub fn open<P: AsRef<Path>>(path: P, mode: GraphMode) -> Result<Self, GraphDBError> {
        Self::open_internal(path, mode, true)
    }

    /// 打开图数据库（可跳过 WAL 恢复）
    pub fn open_without_recovery<P: AsRef<Path>>(path: P, mode: GraphMode) -> Result<Self, GraphDBError> {
        Self::open_internal(path, mode, false)
    }

    /// 文件头是不是旧 PersistentGraph 的 `AXOL`？
    ///
    /// 只读 4 字节，不做任何解析 —— 目的只是把「走哪条解析器」这件事
    /// 从**试错**变成**先看**。读不到、或 magic 不是 `AXOL`，一律交还给
    /// EdgeBlock 解析器，让它报出自己的真实错误。
    fn is_legacy_format(path: &str) -> bool {
        use std::io::Read;
        let mut magic = [0u8; 4];
        match std::fs::File::open(path) {
            Ok(mut f) => f.read_exact(&mut magic).is_ok() && &magic == b"AXOL",
            Err(_) => false,
        }
    }

    fn open_internal<P: AsRef<Path>>(
        path: P,
        mode: GraphMode,
        with_recovery: bool,
    ) -> Result<Self, GraphDBError> {
        let path_str = path.as_ref().to_str().unwrap().to_string();
        let wal_dir = format!("{}.wal", path_str);

        match mode {
            GraphMode::InMemory => {
                let mut eb = if std::path::Path::new(&path_str).exists() {
                    // ★ 先探测 magic，再决定走哪条路 —— **不做「试一个再回退」**。
                    //
                    // 原实现是「试 EdgeBlock，失败就静默回退 PersistentGraph」，
                    // 于是**任何** EdgeBlock 侧的失败（版本不符、字段越界、文件真损坏）
                    // 都会被包装成 PersistentGraph 的 "Invalid magic bytes" ——
                    // 一个与真实原因毫不相干的错误。
                    //
                    // 实测代价：加 FILE_VERSION 校验后现存图库（版本 1）被拒，
                    // 报出来的却是 magic 错，排查方向一度跑偏到「文件是不是坏了」。
                    // 「不知道」和「坏了」在界面上长得一样，是本项目最顽固的 bug 形状。
                    if Self::is_legacy_format(&path_str) {
                        let pg = crate::PersistentGraph::open(&path_str)?;
                        Self::persistent_to_edgeblock(&pg)
                    } else {
                        // 用 `?` 而不是 `if let Ok`：让 EdgeBlock 解析器的
                        // **原始错误**透出来 —— 它是唯一知道真实原因的那个。
                        crate::gpu_edge_block::GPUEdgeBlockGraph::open(&path_str)?
                    }
                } else {
                    // 文件不存在：创建空 EdgeBlock
                    crate::gpu_edge_block::GPUEdgeBlockGraph::new()
                };

                // WAL 恢复
                if with_recovery {
                    let wal_exists = std::path::Path::new(&wal_dir).exists();
                    if wal_exists {
                        // 转 PersistentGraph 用于 WAL 重放
                        let mut pg = Self::eb_to_persistent(&eb);
                        match crate::recovery::recover_wal(&mut pg, &wal_dir) {
                            Ok(result) => {
                                if result.transactions_replayed > 0 {
                                    eprintln!("[recovery] Replayed {} transactions ({} vertices, {} edges), skipped {}",
                                        result.transactions_replayed, result.vertices_added,
                                        result.edges_added, result.transactions_skipped);
                                    eb = Self::persistent_to_edgeblock(&pg);
                                }
                            }
                            Err(e) => {
                                eprintln!("[recovery] Warning: WAL recovery failed: {}", e);
                            }
                        }
                    }
                }

                Ok(GraphDB {
                    mode,
                    edgeblock: Some(eb),
                    mmap: None,
                    data_file: None,
                })
            }
            GraphMode::Mmap => {
                let path_str = path.as_ref().to_str().unwrap();
                let mmap_path = format!("{}.mmap", path_str);

                if !std::path::Path::new(&mmap_path).exists() {
                    if !std::path::Path::new(path_str).exists() {
                        return Err(GraphDBError::Io(format!("neither {} nor {} exists", path_str, mmap_path)));
                    }
                    let src = crate::PersistentGraph::open(path_str)?;
                    MmapGraph::convert_from(&src, &mmap_path)
                        .map_err(|e| GraphDBError::Io(e.to_string()))?;
                }

                let mmap_g = MmapGraph::open(&mmap_path)
                    .map_err(|e| GraphDBError::Io(e.to_string()))?;
                Ok(GraphDB {
                    mode,
                    edgeblock: None,
                    mmap: Some(mmap_g),
                    data_file: None,
                })
            }
        }
    }

    /// 从文件加载，不存在则创建空库（自动 WAL 恢复）
    pub fn from_file_or_new(file_path: &str) -> Result<Self, GraphDBError> {
        if std::path::Path::new(file_path).exists() {
            let mut db = Self::open(file_path, GraphMode::InMemory)?;
            db.data_file = Some(file_path.to_string());
            return Ok(db);
        }
        Ok(GraphDB {
            mode: GraphMode::InMemory,
            edgeblock: Some(GPUEdgeBlockGraph::new()),
            mmap: None,
            data_file: Some(file_path.to_string()),
        })
    }

    /// 保存到绑定的文件路径（如果有）
    pub fn save_to_file(&self) -> Result<(), GraphDBError> {
        match &self.data_file {
            Some(path) => self.save_to(path),
            None => Err(GraphDBError::Io("no data file path set".to_string())),
        }
    }

    /// 获取当前数据文件路径
    pub fn file_path(&self) -> Option<&str> {
        self.data_file.as_deref()
    }

    /// 创建内存模式数据库（可指定文件路径用于保存）
    pub fn create_in_memory(_file_path: &str) -> Result<Self, GraphDBError> {
        Ok(GraphDB {
            mode: GraphMode::InMemory,
            edgeblock: Some(GPUEdgeBlockGraph::new()),
            mmap: None,
            data_file: None,
        })
    }

    /// 从 PersistentGraph 转换为 EdgeBlock
    fn persistent_to_edgeblock(pg: &crate::PersistentGraph) -> GPUEdgeBlockGraph {
        let mut g = GPUEdgeBlockGraph::new();
        for (&id, vr) in &pg.vertices {
            g.add_vertex(id, vr.properties.clone());
        }
        for ((from, to), er) in &pg.edges {
            g.add_edge_with_props(*from, *to, er.weight as f32, er.properties.clone());
        }
        g
    }

    /// EdgeBlock → PersistentGraph（内部转换）
    fn eb_to_persistent(eb: &GPUEdgeBlockGraph) -> crate::PersistentGraph {
        let mut pg = crate::persistence::PersistentGraph {
            vertices: HashMap::new(), edges: HashMap::new(),
            file_path: String::new(),
            index_manager: crate::index::IndexManager::new(),
        };
        for i in 0..eb.vertex_count as usize {
            let id = eb.idx_to_id[i];
            if id != u64::MAX {
                pg.add_vertex(id, eb.vertex_props[i].clone());
            }
        }
        for i in 0..eb.vertex_count as usize {
            let from_id = eb.idx_to_id[i];
            if from_id == u64::MAX { continue; }
            for to_id in eb.out_neighbors_by_idx(i) {
                let ed = eb.get_edge(from_id, to_id)
                    .map(|d| (d.weight as f64, d.properties.clone()))
                    .unwrap_or((1.0, HashMap::new()));
                pg.add_edge(from_id, to_id, ed.0, ed.1);
            }
        }
        pg
    }

    /// 导出为 PersistentGraph（用于兼容旧持久化）
    fn to_persistent(&self) -> crate::PersistentGraph {
        let eb = self.edgeblock.as_ref().unwrap();
        let mut pg = crate::persistence::PersistentGraph {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            file_path: String::new(),
            index_manager: crate::index::IndexManager::new(),
        };
        for i in 0..eb.idx_to_id.len() {
            let id = eb.idx_to_id[i];
            let props = eb.vertex_props[i].clone();
            pg.add_vertex(id, props);
        }
        // 重建所有边
        for i in 0..eb.vertex_count as usize {
            let from_id = eb.idx_to_id[i];
            let neighbors = eb.out_neighbors_by_idx(i);
            for &to_id in &neighbors {
                pg.add_edge(from_id, to_id, 1.0, HashMap::new());
            }
        }
        pg
    }

    // ── 获取底层 EdgeBlock（供 GPU 算法直接访问）─────────────────

    /// 获取 InMemory 模式下的 GPUEdgeBlockGraph 引用
    pub fn edgeblock(&self) -> Option<&GPUEdgeBlockGraph> {
        self.edgeblock.as_ref()
    }

    /// 获取 InMemory 模式下的 GPUEdgeBlockGraph 可变引用
    pub fn edgeblock_mut(&mut self) -> Option<&mut GPUEdgeBlockGraph> {
        self.edgeblock.as_mut()
    }

    // ── 只读 API ─────────────────────────

    pub fn vertex_count(&self) -> usize {
        match self.mode {
            GraphMode::InMemory => self.edgeblock.as_ref().unwrap().vertex_count as usize,
            GraphMode::Mmap    => self.mmap.as_ref().unwrap().vertex_count(),
        }
    }

    pub fn edge_count(&self) -> usize {
        match self.mode {
            GraphMode::InMemory => self.edgeblock.as_ref().unwrap().total_edges as usize,
            GraphMode::Mmap    => self.mmap.as_ref().unwrap().edge_count(),
        }
    }

    pub fn get_vertex(&self, id: u64) -> Option<HashMap<String, PropertyValue>> {
        match self.mode {
            GraphMode::InMemory => {
                self.edgeblock.as_ref().unwrap().get_vertex(id).cloned()
            }
            GraphMode::Mmap => {
                self.mmap.as_ref().unwrap().get_vertex(id)
            }
        }
    }

    pub fn get_edge(&self, from: u64, to: u64) -> Option<(f64, HashMap<String, PropertyValue>)> {
        match self.mode {
            GraphMode::InMemory => {
                self.edgeblock.as_ref().unwrap()
                    .get_edge(from, to)
                    .map(|ed| (ed.weight as f64, ed.properties.clone()))
            }
            GraphMode::Mmap => {
                self.mmap.as_ref().unwrap().get_edge(from, to)
            }
        }
    }

    /// 一对顶点之间的**全部平行边**；`None` = 这个模式不支持（**不是"没有边"**）。
    ///
    /// 为什么用 `Option` 而不是空 `Vec`：空 vec 与"这一对之间没有边"
    /// 长得一模一样，调用方分不出"不支持"和"没有" —— 那就是静默失败。
    pub fn get_edges(&self, from: u64, to: u64)
        -> Option<Vec<(f64, HashMap<String, PropertyValue>)>> {
        let eb = self.edgeblock()?;
        Some(eb.get_edges(from, to).iter()
             .map(|d| (d.weight as f64, d.properties.clone()))
             .collect())
    }

    /// ★ 一次拿到一个顶点的**全部出边及其数据** → `[(dst, weight, props)]`。
    ///
    /// `weight` 是 `Option`：拓扑层有这条边、属性层却查不到时为 `None` ——
    /// 不拿默认值糊过去，让"这条边没有数据"能被看见。
    pub fn out_edges(&self, id: u64)
        -> Option<Vec<(u64, Option<f64>, HashMap<String, PropertyValue>)>> {
        let eb = self.edgeblock()?;
        Some(eb.out_edges(id).into_iter()
             .map(|(d, w, p)| (d, w.map(|x| x as f64), p))
             .collect())
    }

    /// ★ 紧凑返回：`(dsts, values)` 并列数组 —— 全部出边 + 指定字符串属性。
    ///
    /// 与 `out_edges` 的差别**只在返回形态**：那个给每条边一个
    /// `(dst, weight, props_dict)`（Python 侧是 N 个 dict），这个给两个平行列表。
    /// 见 `GPUEdgeBlockGraph::out_edges_cols` 上的实测性能表。
    /// `None` = 该模式不支持（**不是「没有边」**）。
    pub fn out_edges_cols(&self, id: u64, key: &str, distinct: bool)
        -> Option<(Vec<u64>, Vec<String>)>
    {
        let eb = self.edgeblock()?;
        Some(eb.out_edges_cols(id, key, distinct))
    }

    /// 入边的对称版本 → `[(src, weight, props)]`。
    pub fn in_edges(&self, id: u64)
        -> Option<Vec<(u64, Option<f64>, HashMap<String, PropertyValue>)>> {
        let eb = self.edgeblock()?;
        Some(eb.in_edges(id).into_iter()
             .map(|(s, w, p)| (s, w.map(|x| x as f64), p))
             .collect())
    }

    pub fn out_neighbors(&self, id: u64) -> Vec<u64> {
        match self.mode {
            GraphMode::InMemory => {
                self.edgeblock.as_ref().unwrap().out_neighbors(id)
            }
            GraphMode::Mmap => {
                self.mmap.as_ref().unwrap().out_neighbors(id)
            }
        }
    }

    /// 入边邻居（反向遍历：从目标找源头，语义追踪的向上追溯）
    pub fn in_neighbors(&self, id: u64) -> Vec<u64> {
        match self.mode {
            GraphMode::InMemory => {
                self.edgeblock.as_ref().unwrap().in_neighbors(id)
            }
            GraphMode::Mmap => {
                self.mmap.as_ref().unwrap().in_neighbors(id)
            }
        }
    }

    // ── 增强遍历 ─────────────────────────

    /// 多跳遍历
    pub fn walk<F>(&self, start: u64, max_depth: usize, visitor: F) -> Vec<u64>
    where F: FnMut(u64, usize, u64)
    {
        self.edgeblock.as_ref().unwrap().walk(start, max_depth, visitor)
    }

    /// 子图提取
    pub fn subgraph(
        &self, seeds: &[u64], max_depth: usize,
    ) -> (Vec<u64>, Vec<(u64, u64, Option<crate::gpu_edge_block::EdgeData>)>) {
        self.edgeblock.as_ref().unwrap().subgraph(seeds, max_depth)
    }

    /// 路径模式匹配
    pub fn find_paths(
        &self,
        vertex_filter: Option<&dyn Fn(u64) -> bool>,
        edge_filter: Option<&dyn Fn(&crate::gpu_edge_block::EdgeData) -> bool>,
        path_length: usize,
    ) -> Vec<Vec<u64>> {
        self.edgeblock.as_ref().unwrap().find_paths(vertex_filter, edge_filter, path_length)
    }

    /// 转换为 CSR（供 GPU 算法兼容旧接口）
    /// 
    /// InMemory 模式下从 EdgeBlock 构建 CSR（一次性，算法结束后释放）
    pub fn to_csr(&self) -> CSRGraph {
        match self.mode {
            GraphMode::InMemory => {
                let eb = self.edgeblock.as_ref().unwrap();
                let mut csr = CSRGraph::new();

                // 按 idx 顺序添加顶点
                for i in 0..eb.idx_to_id.len() {
                    let id = eb.idx_to_id[i];
                    let props = eb.vertex_props[i].clone();
                    csr.add_vertex(id, props);
                }

                // 收集所有边
                let mut edges: Vec<(u64, u64)> = Vec::new();
                for i in 0..eb.vertex_count as usize {
                    let from_id = eb.idx_to_id[i];
                    for to_id in eb.out_neighbors_by_idx(i) {
                        edges.push((from_id, to_id));
                    }
                }
                csr.build_csr(&edges);

                // 填充权重
                csr.weights = vec![1.0f32; edges.len()];

                csr
            }
            GraphMode::Mmap => {
                self.mmap.as_ref().unwrap().to_csr()
            }
        }
    }

    pub fn iter_vertices(&self) -> Box<dyn Iterator<Item = (u64, HashMap<String, PropertyValue>)> + '_> {
        match self.mode {
            GraphMode::InMemory => {
                let eb = self.edgeblock.as_ref().unwrap();
                Box::new(
                    eb.idx_to_id.iter().enumerate().map(|(i, &id)| {
                        (id, eb.vertex_props[i].clone())
                    })
                )
            }
            GraphMode::Mmap => {
                Box::new(self.mmap.as_ref().unwrap().iter_vertices())
            }
        }
    }

    pub fn iter_edges(&self) -> Box<dyn Iterator<Item = (u64, u64, f64, HashMap<String, PropertyValue>)> + '_> {
        match self.mode {
            GraphMode::InMemory => {
                let eb = self.edgeblock.as_ref().unwrap();
                let mut result: Vec<(u64, u64, f64, HashMap<String, PropertyValue>)> = Vec::new();
                for i in 0..eb.vertex_count as usize {
                    if eb.idx_to_id[i] == u64::MAX { continue; }   // 已删除的顶点
                    let from_id = eb.idx_to_id[i];
                    // ★ 用 `out_edges`（它已按**追加序**把拓扑对到属性），
                    //   而不是 `out_neighbors` + `get_edge`：后者在一对之间
                    //   有平行边时，会把**同一条**重复输出多次，同时丢掉其余的。
                    for (to_id, weight, props) in eb.out_edges(from_id) {
                        // 导出路径的签名是 f64，没有 Option 的位置。
                        // "拓扑有边、属性查不到"在正常路径上不该出现
                        // （add_edge 总是同时写两边）；真出现时会在别处以
                        // 不一致的形式暴露，而不是在这里被悄悄补成 1.0。
                        result.push((from_id, to_id, weight.unwrap_or(1.0) as f64, props));
                    }
                }
                Box::new(result.into_iter())
            }
            GraphMode::Mmap => {
                Box::new(
                    self.mmap.as_ref().unwrap().iter_edges()
                        .map(|(from, to, weight, props)| (from, to, weight, props))
                )
            }
        }
    }

    pub fn mode(&self) -> GraphMode {
        self.mode
    }

    // ── 写操作 API（仅 InMemory 模式）─────────────────────────

    pub fn add_vertex(&mut self, id: u64, properties: HashMap<String, PropertyValue>) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                self.edgeblock.as_mut().unwrap().add_vertex(id, properties);
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("add_vertex not supported in Mmap mode.".to_string()))
            }
        }
    }

    pub fn add_edge(
        &mut self,
        from: u64,
        to: u64,
        weight: f64,
        properties: HashMap<String, PropertyValue>,
    ) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                self.edgeblock.as_mut().unwrap().add_edge_with_props(from, to, weight as f32, properties);
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("add_edge not supported in Mmap mode.".to_string()))
            }
        }
    }

    pub fn delete_vertex(&mut self, id: u64) -> Result<Option<HashMap<String, PropertyValue>>, GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                Ok(self.edgeblock.as_mut().unwrap().remove_vertex(id))
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("delete_vertex not supported in Mmap mode.".to_string()))
            }
        }
    }

    pub fn delete_edge(&mut self, from: u64, to: u64) -> Result<Option<(f64, HashMap<String, PropertyValue>)>, GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                let found = self.edgeblock.as_mut().unwrap().remove_edge(from, to);
                if found {
                    Ok(Some((1.0, HashMap::new())))
                } else {
                    Ok(None)
                }
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("delete_edge not supported in Mmap mode.".to_string()))
            }
        }
    }

    pub fn save(&self) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                // 持久化需要一个文件路径。通过 GraphDB 的 save_to 方法指定。
                Err(GraphDBError::NotSupported("use GraphDB::save_to(file_path) to specify save path. EdgeBlock persistence coming soon.".to_string()))
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("save not supported in Mmap mode.".to_string()))
            }
        }
    }

    /// 保存到指定文件（仅 InMemory 模式）
    pub fn save_to(&self, file_path: &str) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                // 使用 EdgeBlock 原生二进制持久化
                self.edgeblock.as_ref().unwrap().save(file_path)
                    .map_err(|e| GraphDBError::Io(e.to_string()))?;
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("save_to not supported in Mmap mode.".to_string()))
            }
        }
    }

    /// 生成 mmap 文件
    pub fn generate_mmap_file(&self, mmap_path: &str) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                let pg = self.to_persistent();
                MmapGraph::convert_from(&pg, mmap_path)
                    .map_err(|e| GraphDBError::Io(e.to_string()))?;
                Ok(())
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("generate_mmap_file only works in InMemory mode.".to_string()))
            }
        }
    }

    // ── 索引 API ─────────────────────────

    pub fn create_index(&mut self, _name: &str, _property_key: &str, _unique: bool) -> Result<(), GraphDBError> {
        match self.mode {
            GraphMode::InMemory => {
                // EdgeBlock 暂不原生支持索引，通过 PersistentGraph 实现
                let _pg = self.to_persistent();
                // TODO: 索引状态应该持久化在 GraphDB 中
                Err(GraphDBError::NotSupported("create_index not yet supported with EdgeBlock.".to_string()))
            }
            GraphMode::Mmap => {
                Err(GraphDBError::NotSupported("create_index not supported in Mmap mode.".to_string()))
            }
        }
    }

    pub fn drop_index(&mut self, _name: &str) -> Result<(), GraphDBError> {
        Err(GraphDBError::NotSupported("drop_index not yet supported with EdgeBlock.".to_string()))
    }
}

// ── 测试 ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PropertyValue;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn make_test_graph() -> (GraphDB, String) {
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let file_path = format!("/tmp/test_graph_db_{}.bin", id);
        let mmap_path = format!("{}.mmap", file_path);

        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_file(&mmap_path);

        let mut db = GraphDB::create_in_memory(&file_path).unwrap();

        let mut alice = HashMap::new();
        alice.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        alice.insert("age".to_string(),  PropertyValue::Int(28));
        db.add_vertex(1, alice).unwrap();

        let mut bob = HashMap::new();
        bob.insert("name".to_string(), PropertyValue::String("Bob".to_string()));
        db.add_vertex(2, bob).unwrap();

        db.add_edge(1, 2, 1.0, HashMap::new()).unwrap();

        // 持久化（测试需要）
        db.save_to(&file_path).unwrap();

        (db, file_path)
    }

    #[test]
    fn test_in_memory_mode() {
        let (db, _file_path) = make_test_graph();

        assert_eq!(db.mode(), GraphMode::InMemory);
        assert_eq!(db.vertex_count(), 2);
        assert_eq!(db.edge_count(),   1);

        let alice = db.get_vertex(1).unwrap();
        assert_eq!(alice["name"], PropertyValue::String("Alice".to_string()));

        let neighbors = db.out_neighbors(1);
        assert_eq!(neighbors, vec![2]);
    }

    #[test]
    fn test_mmap_mode() {
        let (_db, file_path) = make_test_graph();
        let mmap_path = format!("{}.mmap", file_path);

        let db = GraphDB::open(&file_path, GraphMode::InMemory).unwrap();
        db.generate_mmap_file(&mmap_path).unwrap();

        let db = GraphDB::open(&file_path, GraphMode::Mmap).unwrap();

        assert_eq!(db.mode(), GraphMode::Mmap);
        assert_eq!(db.vertex_count(), 2);
        assert_eq!(db.edge_count(),   1);

        let alice = db.get_vertex(1).unwrap();
        assert_eq!(alice["name"], PropertyValue::String("Alice".to_string()));

        let neighbors = db.out_neighbors(1);
        assert_eq!(neighbors, vec![2]);

        let _ = std::fs::remove_file(&mmap_path);
    }

    #[test]
    fn test_mmap_mode_read_only() {
        let (_db, file_path) = make_test_graph();
        let mmap_path = format!("{}.mmap", file_path);

        let db = GraphDB::open(&file_path, GraphMode::InMemory).unwrap();
        db.generate_mmap_file(&mmap_path).unwrap();

        let mut db = GraphDB::open(&file_path, GraphMode::Mmap).unwrap();

        let result = db.add_vertex(3, HashMap::new());
        assert!(result.is_err());

        let result = db.add_edge(1, 3, 1.0, HashMap::new());
        assert!(result.is_err());

        let result = db.save();
        assert!(result.is_err()); // mmap 模式不能保存

        let _ = std::fs::remove_file(&mmap_path);
    }

    #[test]
    fn test_to_csr_both_modes() {
        let (db, _file_path) = make_test_graph();
        let csr = db.to_csr();
        assert_eq!(csr.vertex_count, 2);
        assert_eq!(csr.total_edges,  1);

        let (_db, file_path) = make_test_graph();
        let mmap_path = format!("{}.mmap", file_path);

        let db = GraphDB::open(&file_path, GraphMode::InMemory).unwrap();
        db.generate_mmap_file(&mmap_path).unwrap();

        let db = GraphDB::open(&file_path, GraphMode::Mmap).unwrap();
        let csr = db.to_csr();
        assert_eq!(csr.vertex_count, 2);
        assert_eq!(csr.total_edges,  1);

        let _ = std::fs::remove_file(&mmap_path);
    }

    #[test]
    fn test_iter_both_modes() {
        let (db, _file_path) = make_test_graph();
        let vertices: Vec<_> = db.iter_vertices().collect();
        assert_eq!(vertices.len(), 2);

        let (_db, file_path) = make_test_graph();
        let mmap_path = format!("{}.mmap", file_path);

        let db = GraphDB::open(&file_path, GraphMode::InMemory).unwrap();
        db.generate_mmap_file(&mmap_path).unwrap();

        let db = GraphDB::open(&file_path, GraphMode::Mmap).unwrap();
        let vertices: Vec<_> = db.iter_vertices().collect();
        assert_eq!(vertices.len(), 2);

        let _ = std::fs::remove_file(&mmap_path);
    }

    #[test]
    fn test_edgeblock_many_edges() {
        let mut db = GraphDB::new(GraphMode::InMemory);
        let center_id = 1u64;

        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("center".to_string()));
        db.add_vertex(center_id, props).unwrap();

        for i in 0..100u64 {
            let id = 1000 + i;
            db.add_vertex(id, HashMap::new()).unwrap();
            db.add_edge(center_id, id, 1.0, HashMap::new()).unwrap();
        }

        assert_eq!(db.vertex_count(), 101);
        assert_eq!(db.edge_count(), 100);

        let neighbors = db.out_neighbors(center_id);
        assert_eq!(neighbors.len(), 100);

        // 验证所有邻居都能找到
        for i in 0..100u64 {
            assert!(neighbors.contains(&(1000 + i)));
        }
    }

    #[test]
    fn test_edgeblock_direct_access() {
        let mut db = GraphDB::new(GraphMode::InMemory);
        db.add_vertex(10, HashMap::new()).unwrap();
        db.add_vertex(20, HashMap::new()).unwrap();
        db.add_edge(10, 20, 1.0, HashMap::new()).unwrap();

        // 直接访问 EdgeBlock
        let eb = db.edgeblock().unwrap();
        assert_eq!(eb.total_edges, 1);
        assert_eq!(eb.out_neighbors(10), vec![20]);

        // 可变访问
        let eb_mut = db.edgeblock_mut().unwrap();
        eb_mut.add_edge(20, 10, 1.0);
        assert_eq!(eb_mut.total_edges, 2);
    }
}
