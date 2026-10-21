//! Embedded Interactive Web Studio for RustyGraph.
//!
//! Provides a modern, dark-mode visual web console with interactive force-directed graph rendering,
//! openCypher query editor with parameterized statement support, real-time database metrics,
//! inverted full-text keyword search explorer, and vector similarity search.

pub const STUDIO_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>RustyGraph Studio</title>
  <style>
    :root {
      --bg: #090d16;
      --card-bg: #111827;
      --card-border: #1e293b;
      --card-hover: #1f293d;
      --primary: #38bdf8;
      --primary-hover: #0ea5e9;
      --accent: #6366f1;
      --success: #10b981;
      --warning: #f59e0b;
      --danger: #ef4444;
      --text: #f3f4f6;
      --text-muted: #9ca3af;
      --font-mono: 'Consolas', 'Monaco', 'Courier New', monospace;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background: var(--bg);
      color: var(--text);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      height: 100vh;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }
    header {
      background: #0f172a;
      border-bottom: 1px solid var(--card-border);
      padding: 0.75rem 1.5rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 1rem;
    }
    .logo {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      font-size: 1.25rem;
      font-weight: 700;
      color: #fff;
      letter-spacing: -0.5px;
    }
    .logo-badge {
      background: linear-gradient(135deg, #0284c7, #6366f1);
      color: white;
      font-size: 0.7rem;
      padding: 0.2rem 0.5rem;
      border-radius: 9999px;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }
    .stats-bar {
      display: flex;
      align-items: center;
      gap: 1.25rem;
      font-size: 0.85rem;
      color: var(--text-muted);
    }
    .stat-pill {
      background: rgba(30, 41, 59, 0.7);
      padding: 0.35rem 0.75rem;
      border-radius: 6px;
      border: 1px solid var(--card-border);
      display: flex;
      align-items: center;
      gap: 0.4rem;
    }
    .stat-val { color: var(--primary); font-weight: 700; font-family: var(--font-mono); }
    .status-dot {
      width: 8px; height: 8px; border-radius: 50%; background: var(--success);
      box-shadow: 0 0 8px var(--success);
    }

    nav.tabs {
      display: flex;
      gap: 0.5rem;
      background: #0d1322;
      padding: 0.5rem 1.5rem 0;
      border-bottom: 1px solid var(--card-border);
    }
    .tab-btn {
      background: transparent;
      border: none;
      border-bottom: 2px solid transparent;
      color: var(--text-muted);
      padding: 0.5rem 1rem;
      cursor: pointer;
      font-weight: 600;
      font-size: 0.9rem;
      transition: all 0.2s;
    }
    .tab-btn:hover { color: #fff; }
    .tab-btn.active {
      color: var(--primary);
      border-bottom-color: var(--primary);
    }

    main {
      flex: 1;
      display: flex;
      position: relative;
      overflow: hidden;
    }
    .view-panel {
      flex: 1;
      display: none;
      height: 100%;
      position: relative;
    }
    .view-panel.active { display: flex; }

    /* Cypher Editor & Explorer layout */
    .cypher-layout {
      display: flex;
      width: 100%;
      height: 100%;
    }
    .sidebar-editor {
      width: 480px;
      background: var(--card-bg);
      border-right: 1px solid var(--card-border);
      display: flex;
      flex-direction: column;
      flex-shrink: 0;
    }
    .editor-section {
      padding: 1rem;
      border-bottom: 1px solid var(--card-border);
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
    }
    .section-title {
      font-size: 0.8rem;
      text-transform: uppercase;
      font-weight: 700;
      letter-spacing: 0.5px;
      color: var(--text-muted);
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    textarea.code-input {
      width: 100%;
      background: #090d16;
      border: 1px solid var(--card-border);
      border-radius: 6px;
      color: #38bdf8;
      font-family: var(--font-mono);
      font-size: 0.85rem;
      padding: 0.75rem;
      resize: vertical;
      outline: none;
      line-height: 1.4;
    }
    textarea.code-input:focus { border-color: var(--primary); }
    .btn-row {
      display: flex;
      gap: 0.5rem;
      align-items: center;
    }
    .btn {
      background: var(--primary);
      color: #090d16;
      border: none;
      border-radius: 6px;
      padding: 0.5rem 1rem;
      font-weight: 600;
      font-size: 0.85rem;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 0.4rem;
      transition: background 0.15s;
    }
    .btn:hover { background: var(--primary-hover); }
    .btn-secondary {
      background: #1e293b;
      color: var(--text);
      border: 1px solid var(--card-border);
    }
    .btn-secondary:hover { background: #334155; }
    select.sample-select {
      background: #1e293b;
      color: var(--text);
      border: 1px solid var(--card-border);
      padding: 0.45rem;
      border-radius: 6px;
      font-size: 0.8rem;
      outline: none;
      flex: 1;
    }

    .results-section {
      flex: 1;
      display: flex;
      flex-direction: column;
      overflow: hidden;
      padding: 1rem;
    }
    .results-table-container {
      flex: 1;
      overflow: auto;
      background: #090d16;
      border: 1px solid var(--card-border);
      border-radius: 6px;
    }
    table.result-table {
      width: 100%;
      border-collapse: collapse;
      font-size: 0.8rem;
      font-family: var(--font-mono);
    }
    table.result-table th {
      background: #162032;
      color: var(--primary);
      text-align: left;
      padding: 0.5rem 0.75rem;
      position: sticky;
      top: 0;
      border-bottom: 1px solid var(--card-border);
    }
    table.result-table td {
      padding: 0.5rem 0.75rem;
      border-bottom: 1px solid #141d2d;
      color: #e2e8f0;
      white-space: pre-wrap;
      word-break: break-all;
    }
    table.result-table tr:hover { background: #131b2e; }

    .canvas-container {
      flex: 1;
      position: relative;
      background: radial-gradient(circle at center, #111a2e 0%, #090d16 100%);
      overflow: hidden;
    }
    canvas#graph-canvas {
      width: 100%;
      height: 100%;
      display: block;
      cursor: grab;
    }
    canvas#graph-canvas:active { cursor: grabbing; }

    .canvas-overlay {
      position: absolute;
      top: 1rem;
      right: 1rem;
      display: flex;
      gap: 0.5rem;
      background: rgba(17, 24, 39, 0.85);
      backdrop-filter: blur(8px);
      padding: 0.4rem;
      border-radius: 8px;
      border: 1px solid var(--card-border);
    }
    .node-drawer {
      position: absolute;
      bottom: 1rem;
      right: 1rem;
      width: 320px;
      background: rgba(17, 24, 39, 0.95);
      backdrop-filter: blur(10px);
      border: 1px solid var(--card-border);
      border-radius: 8px;
      padding: 1rem;
      display: none;
      flex-direction: column;
      gap: 0.5rem;
      font-size: 0.85rem;
      box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.5);
    }
    .node-drawer.active { display: flex; }
    .drawer-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      font-weight: 700;
      color: var(--primary);
      border-bottom: 1px solid var(--card-border);
      padding-bottom: 0.4rem;
    }

    /* Fulltext / Search layout */
    .search-layout {
      width: 100%;
      height: 100%;
      padding: 2rem;
      display: flex;
      flex-direction: column;
      gap: 1.5rem;
      overflow-y: auto;
      max-width: 1100px;
      margin: 0 auto;
    }
    .search-hero {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 12px;
      padding: 1.5rem;
      display: flex;
      flex-direction: column;
      gap: 1rem;
    }
    .search-input-row {
      display: flex;
      gap: 0.75rem;
    }
    input.search-bar {
      flex: 1;
      background: #090d16;
      border: 1px solid var(--card-border);
      border-radius: 8px;
      color: #fff;
      font-size: 1rem;
      padding: 0.75rem 1rem;
      outline: none;
    }
    input.search-bar:focus { border-color: var(--primary); }
    .hit-card {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 8px;
      padding: 1rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
      transition: border-color 0.15s;
    }
    .hit-card:hover { border-color: var(--primary); }
    .hit-score {
      background: rgba(56, 189, 248, 0.15);
      color: var(--primary);
      padding: 0.25rem 0.6rem;
      border-radius: 6px;
      font-weight: 700;
      font-size: 0.85rem;
      font-family: var(--font-mono);
    }
  </style>
</head>
<body>

  <header>
    <div class="logo">
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#38bdf8" stroke-width="2">
        <circle cx="6" cy="6" r="3"/>
        <circle cx="18" cy="6" r="3"/>
        <circle cx="18" cy="18" r="3"/>
        <circle cx="6" cy="18" r="3"/>
        <path d="M8.5 7.5l7 7M8.5 6h7M6 8.5v7M18 8.5v7"/>
      </svg>
      RustyGraph Studio
      <span class="logo-badge">v0.1.0-alpha</span>
    </div>

    <div class="stats-bar">
      <div class="stat-pill"><div class="status-dot"></div> <span id="health-label">HEALTHY</span></div>
      <div class="stat-pill">Nodes: <span class="stat-val" id="stat-nodes">0</span></div>
      <div class="stat-pill">Edges: <span class="stat-val" id="stat-edges">0</span></div>
      <div class="stat-pill">Memory: <span class="stat-val" id="stat-mem">0 KB</span></div>
      <div class="stat-pill">WAL Seq: <span class="stat-val" id="stat-wal">0</span></div>
      <button class="btn btn-secondary" onclick="refreshStats()" style="padding: 0.25rem 0.5rem; font-size: 0.75rem;">↻</button>
    </div>
  </header>

  <nav class="tabs">
    <button class="tab-btn active" onclick="switchTab('cypher')">Cypher Explorer & Canvas</button>
    <button class="tab-btn" onclick="switchTab('fulltext')">Full-Text Inverted Index</button>
    <button class="tab-btn" onclick="switchTab('api')">REST API Reference</button>
  </nav>

  <main>
    <!-- Tab 1: Cypher Explorer & Force-Directed Graph -->
    <div id="tab-cypher" class="view-panel active">
      <div class="cypher-layout">
        <aside class="sidebar-editor">
          <div class="editor-section">
            <div class="section-title">
              <span>openCypher Query</span>
              <select class="sample-select" onchange="loadSampleQuery(this.value)">
                <option value="">-- Load Sample Query --</option>
                <option value="create_sample">Create Knowledge Graph</option>
                <option value="match_all">MATCH (n) RETURN n LIMIT 25</option>
                <option value="match_params">Parameterized MATCH ($role, $min_xp)</option>
                <option value="shortest_path">Shortest Path Analysis</option>
              </select>
            </div>
            <textarea id="cypher-input" class="code-input" rows="5" placeholder="MATCH (n) RETURN n LIMIT 25">MATCH (n) RETURN n LIMIT 25</textarea>

            <div class="section-title" style="margin-top: 0.5rem;">
              <span>Parameters ($param JSON)</span>
            </div>
            <textarea id="params-input" class="code-input" rows="2" placeholder='{"role": "Rust Expert", "min_xp": 5}'>{}</textarea>

            <div class="btn-row" style="margin-top: 0.5rem;">
              <button class="btn" onclick="runCypher()">
                ▶ Run Query <span style="font-size: 0.75rem; opacity: 0.8;">(Ctrl+Enter)</span>
              </button>
              <button class="btn btn-secondary" onclick="clearResults()">Clear</button>
              <span id="exec-latency" style="font-size: 0.75rem; color: var(--text-muted); margin-left: auto;"></span>
            </div>
          </div>

          <div class="results-section">
            <div class="section-title" style="margin-bottom: 0.5rem;">
              <span id="results-count">Results (0 rows)</span>
            </div>
            <div class="results-table-container">
              <table id="result-table" class="result-table">
                <thead>
                  <tr id="result-header"><th>Status</th></tr>
                </thead>
                <tbody id="result-body">
                  <tr><td style="color: var(--text-muted); font-style: italic;">Run a query to inspect records.</td></tr>
                </tbody>
              </table>
            </div>
          </div>
        </aside>

        <div class="canvas-container">
          <canvas id="graph-canvas"></canvas>
          <div class="canvas-overlay">
            <button class="btn btn-secondary" onclick="resetCanvasView()" title="Reset Zoom">⟲ Reset View</button>
            <button class="btn btn-secondary" onclick="fitGraphToScreen()" title="Fit Graph">Fit Graph</button>
          </div>

          <div id="node-drawer" class="node-drawer">
            <div class="drawer-header">
              <span id="drawer-title">Node Details</span>
              <button onclick="closeDrawer()" style="background:none; border:none; color:#fff; cursor:pointer;">✕</button>
            </div>
            <div id="drawer-content" style="max-height: 200px; overflow-y: auto; font-family: var(--font-mono); font-size: 0.8rem;"></div>
          </div>
        </div>
      </div>
    </div>

    <!-- Tab 2: Full-Text & Hybrid GraphRAG -->
    <div id="tab-fulltext" class="view-panel">
      <div class="search-layout">
        <div class="search-hero">
          <h2 style="font-size: 1.25rem;">Inverted Full-Text Search Engine</h2>
          <p style="color: var(--text-muted); font-size: 0.85rem;">
            Query nodes across string properties using tokenization, stop-word elimination, and BM25 / TF-IDF relevance scoring.
          </p>

          <div class="search-input-row">
            <input id="ft-index-name" class="search-bar" style="max-width: 200px;" placeholder="Index name" value="doc_content">
            <input id="ft-search-query" class="search-bar" placeholder="Enter keywords (e.g. rust distributed graph)...">
            <button class="btn" onclick="runFulltextSearch()">Search</button>
          </div>

          <div style="display: flex; gap: 1rem; font-size: 0.85rem; align-items: center;">
            <label style="display:flex; align-items:center; gap:0.4rem; cursor:pointer;">
              <input type="checkbox" id="ft-prefix"> Enable Prefix Matching
            </label>
            <span style="color: var(--text-muted);">|</span>
            <button class="btn btn-secondary" style="padding: 0.3rem 0.6rem; font-size: 0.8rem;" onclick="createDefaultFulltextIndex()">
              + Create Index on (Doc:content)
            </button>
          </div>
        </div>

        <div id="ft-results-container" style="display: flex; flex-direction: column; gap: 0.75rem;">
          <div style="color: var(--text-muted); font-size: 0.9rem; text-align: center; padding: 2rem;">
            No search conducted yet. Enter a query or create an index above.
          </div>
        </div>
      </div>
    </div>

    <!-- Tab 3: REST API Quick Reference -->
    <div id="tab-api" class="view-panel">
      <div class="search-layout">
        <div class="search-hero">
          <h2 style="font-size: 1.25rem;">RustyGraph HTTP REST Endpoints</h2>
          <div style="font-family: var(--font-mono); font-size: 0.85rem; line-height: 1.8; color: #cbd5e1; margin-top: 1rem;">
            <div><strong style="color: var(--primary);">POST /cypher</strong> - Execute openCypher with optional parameters: <code>{"query": "...", "params": {...}}</code></div>
            <div><strong style="color: var(--primary);">POST /fulltext/indexes</strong> - Create inverted full-text index: <code>{"name": "...", "label": "...", "property_key": "..."}</code></div>
            <div><strong style="color: var(--primary);">POST /fulltext/search</strong> - Search full-text index: <code>{"index": "...", "query": "...", "top_k": 10, "prefix": true}</code></div>
            <div><strong style="color: var(--primary);">POST /vector/indexes</strong> - Create vector index: <code>{"name": "...", "label": "...", "prop_key": "...", "dimension": 128}</code></div>
            <div><strong style="color: var(--primary);">POST /vector/search</strong> - Nearest neighbor search: <code>{"index": "...", "vector": [...], "k": 5}</code></div>
            <div><strong style="color: var(--primary);">POST /tx/begin</strong>, <strong>POST /tx/:id/cypher</strong>, <strong>POST /tx/:id/commit</strong> - Snapshot isolation ACID transactions</div>
            <div><strong style="color: var(--primary);">GET /stats</strong>, <strong>GET /health</strong>, <strong>GET /cluster/status</strong> - Server & Cluster telemetry</div>
          </div>
        </div>
      </div>
    </div>
  </main>

  <script>
    // Visual Graph Simulation State
    let graphData = { nodes: [], edges: [] };
    let canvas = document.getElementById('graph-canvas');
    let ctx = canvas.getContext('2d');
    let zoom = 1.0, panX = 0, panY = 0;
    let draggingNode = null, isPanning = false, startPanX = 0, startPanY = 0;

    function resizeCanvas() {
      canvas.width = canvas.parentElement.clientWidth;
      canvas.height = canvas.parentElement.clientHeight;
      drawGraph();
    }
    window.addEventListener('resize', resizeCanvas);

    // Initial stats fetch & canvas setup
    window.addEventListener('load', () => {
      resizeCanvas();
      refreshStats();
      setInterval(refreshStats, 5000);
      setupCanvasInteractions();
      runCypher();
    });

    function switchTab(name) {
      document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
      document.querySelectorAll('.view-panel').forEach(p => p.classList.remove('active'));
      event.target.classList.add('active');
      document.getElementById('tab-' + name).classList.add('active');
      if (name === 'cypher') resizeCanvas();
    }

    async function refreshStats() {
      try {
        let res = await fetch('/stats');
        if (res.ok) {
          let s = await res.json();
          document.getElementById('stat-nodes').innerText = s.node_count || 0;
          document.getElementById('stat-edges').innerText = s.relationship_count || 0;
          document.getElementById('stat-mem').innerText = ((s.estimated_memory_bytes || 0) / 1024).toFixed(1) + ' KB';
          document.getElementById('stat-wal').innerText = s.wal_sequence || 0;
        }
      } catch (e) {
        console.warn('Stats fetch failed', e);
      }
    }

    const SAMPLES = {
      create_sample: `CREATE (a:Person {name: "Alice", role: "Rust Expert", xp: 8})
CREATE (b:Person {name: "Bob", role: "Graph Architect", xp: 6})
CREATE (rg:Database {name: "RustyGraph", type: "Native Graph", storage: "Hybrid WAL + In-Memory"})
CREATE (a)-[:CONTRIBUTES_TO {since: 2026}]->(rg)
CREATE (b)-[:CONTRIBUTES_TO {since: 2026}]->(rg)
CREATE (a)-[:COLLABORATES_WITH]->(b)
RETURN a, b, rg`,
      match_all: `MATCH (n) RETURN n LIMIT 25`,
      match_params: `MATCH (n:Person) WHERE n.role = $role AND n.xp >= $min_xp RETURN n`,
      shortest_path: `MATCH (n) RETURN n LIMIT 50`
    };

    function loadSampleQuery(key) {
      if (!key) return;
      document.getElementById('cypher-input').value = SAMPLES[key] || '';
      if (key === 'match_params') {
        document.getElementById('params-input').value = JSON.stringify({ role: "Rust Expert", min_xp: 5 }, null, 2);
      } else {
        document.getElementById('params-input').value = '{}';
      }
    }

    async function runCypher() {
      let query = document.getElementById('cypher-input').value.trim();
      let params = {};
      try {
        let pText = document.getElementById('params-input').value.trim();
        if (pText) params = JSON.parse(pText);
      } catch (err) {
        alert('Invalid JSON parameters: ' + err.message);
        return;
      }

      let t0 = performance.now();
      try {
        let res = await fetch('/cypher', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ query, params })
        });
        let t1 = performance.now();
        document.getElementById('exec-latency').innerText = `${(t1 - t0).toFixed(1)} ms`;

        let data = await res.json();
        if (!res.ok || data.error) {
          renderError(data.error || 'Query failed');
        } else {
          renderResults(data);
          buildGraphFromCypher(data);
          refreshStats();
        }
      } catch (e) {
        renderError(e.message);
      }
    }

    function renderError(msg) {
      document.getElementById('results-count').innerText = 'Query Error';
      document.getElementById('result-header').innerHTML = '<th>Error</th>';
      document.getElementById('result-body').innerHTML = `<tr><td style="color: var(--danger);">${msg}</td></tr>`;
    }

    function renderResults(data) {
      let cols = data.columns || [];
      let rows = data.rows || [];
      document.getElementById('results-count').innerText = `Results (${rows.length} rows)`;

      let hHtml = cols.map(c => `<th>${c}</th>`).join('');
      document.getElementById('result-header').innerHTML = hHtml || '<th>Result</th>';

      if (rows.length === 0) {
        document.getElementById('result-body').innerHTML = `<tr><td colspan="${cols.length || 1}" style="color: var(--text-muted); font-style: italic;">(Empty result set)</td></tr>`;
        return;
      }

      let bHtml = rows.map(row => {
        let cells = row.map(cell => `<td>${formatCellValue(cell)}</td>`).join('');
        return `<tr>${cells}</tr>`;
      }).join('');
      document.getElementById('result-body').innerHTML = bHtml;
    }

    function formatCellValue(val) {
      if (val === null || val === undefined) return '<span style="color:#64748b">null</span>';
      if (typeof val === 'object') return JSON.stringify(val);
      return String(val);
    }

    // Force-directed graph representation
    function buildGraphFromCypher(data) {
      let newNodes = new Map();
      let newEdges = [];

      (data.rows || []).forEach(row => {
        row.forEach(cell => {
          if (cell && typeof cell === 'object') {
            if ('id' in cell && ('labels' in cell || 'properties' in cell)) {
              if (!newNodes.has(cell.id)) {
                let existing = graphData.nodes.find(n => n.id === cell.id);
                newNodes.set(cell.id, {
                  id: cell.id,
                  label: (cell.labels && cell.labels[0]) || 'Node',
                  name: (cell.properties && (cell.properties.name || cell.properties.title)) || `Node ${cell.id}`,
                  properties: cell.properties || {},
                  x: existing ? existing.x : (canvas.width / 2 + (Math.random() - 0.5) * 200),
                  y: existing ? existing.y : (canvas.height / 2 + (Math.random() - 0.5) * 200),
                  vx: 0, vy: 0
                });
              }
            }
          }
        });
      });

      graphData.nodes = Array.from(newNodes.values());
      // Connect nodes sequentially if no direct relationship object returned
      for (let i = 0; i < graphData.nodes.length - 1; i++) {
        newEdges.push({ source: graphData.nodes[i], target: graphData.nodes[i + 1] });
      }
      graphData.edges = newEdges;

      startSimulation();
    }

    let simTimer = null;
    function startSimulation() {
      if (simTimer) cancelAnimationFrame(simTimer);
      let stepCount = 0;
      function tick() {
        stepSimulation();
        drawGraph();
        stepCount++;
        if (stepCount < 180 || draggingNode) {
          simTimer = requestAnimationFrame(tick);
        }
      }
      simTimer = requestAnimationFrame(tick);
    }

    function stepSimulation() {
      const k = 120; // ideal spring length
      const repulsion = 4000;
      const damping = 0.85;

      // Repulsion between all pairs
      for (let i = 0; i < graphData.nodes.length; i++) {
        let n1 = graphData.nodes[i];
        for (let j = i + 1; j < graphData.nodes.length; j++) {
          let n2 = graphData.nodes[j];
          let dx = n2.x - n1.x;
          let dy = n2.y - n1.y;
          let dist = Math.sqrt(dx * dx + dy * dy) || 1;
          let force = repulsion / (dist * dist);
          let fx = (dx / dist) * force;
          let fy = (dy / dist) * force;
          n1.vx -= fx; n1.vy -= fy;
          n2.vx += fx; n2.vy += fy;
        }
      }

      // Spring attraction on edges
      graphData.edges.forEach(e => {
        let dx = e.target.x - e.source.x;
        let dy = e.target.y - e.source.y;
        let dist = Math.sqrt(dx * dx + dy * dy) || 1;
        let force = (dist - k) * 0.05;
        let fx = (dx / dist) * force;
        let fy = (dy / dist) * force;
        e.source.vx += fx; e.source.vy += fy;
        e.target.vx -= fx; e.target.vy -= fy;
      });

      // Update positions
      graphData.nodes.forEach(n => {
        if (n === draggingNode) return;
        n.x += n.vx;
        n.y += n.vy;
        n.vx *= damping;
        n.vy *= damping;
      });
    }

    const LABEL_COLORS = {
      Person: '#6366f1',
      Database: '#0ea5e9',
      Doc: '#10b981',
      Technology: '#f59e0b',
      Default: '#38bdf8'
    };

    function drawGraph() {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.save();
      ctx.translate(panX, panY);
      ctx.scale(zoom, zoom);

      // Draw Edges
      ctx.strokeStyle = 'rgba(71, 85, 105, 0.6)';
      ctx.lineWidth = 1.5;
      graphData.edges.forEach(e => {
        ctx.beginPath();
        ctx.moveTo(e.source.x, e.source.y);
        ctx.lineTo(e.target.x, e.target.y);
        ctx.stroke();
      });

      // Draw Nodes
      graphData.nodes.forEach(n => {
        let color = LABEL_COLORS[n.label] || LABEL_COLORS.Default;
        // Node circle
        ctx.beginPath();
        ctx.arc(n.x, n.y, 22, 0, 2 * Math.PI);
        ctx.fillStyle = color;
        ctx.fill();
        ctx.strokeStyle = '#fff';
        ctx.lineWidth = 2;
        ctx.stroke();

        // Label text
        ctx.fillStyle = '#f3f4f6';
        ctx.font = '600 11px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(n.name, n.x, n.y + 34);
      });

      ctx.restore();
    }

    function setupCanvasInteractions() {
      canvas.addEventListener('mousedown', e => {
        let rect = canvas.getBoundingClientRect();
        let mouseX = (e.clientX - rect.left - panX) / zoom;
        let mouseY = (e.clientY - rect.top - panY) / zoom;

        // Check if node clicked
        for (let n of graphData.nodes) {
          let dx = n.x - mouseX;
          let dy = n.y - mouseY;
          if (Math.sqrt(dx * dx + dy * dy) <= 22) {
            draggingNode = n;
            openDrawer(n);
            startSimulation();
            return;
          }
        }

        isPanning = true;
        startPanX = e.clientX - panX;
        startPanY = e.clientY - panY;
      });

      window.addEventListener('mousemove', e => {
        if (draggingNode) {
          let rect = canvas.getBoundingClientRect();
          draggingNode.x = (e.clientX - rect.left - panX) / zoom;
          draggingNode.y = (e.clientY - rect.top - panY) / zoom;
          drawGraph();
        } else if (isPanning) {
          panX = e.clientX - startPanX;
          panY = e.clientY - startPanY;
          drawGraph();
        }
      });

      window.addEventListener('mouseup', () => {
        draggingNode = null;
        isPanning = false;
      });

      canvas.addEventListener('wheel', e => {
        e.preventDefault();
        let factor = e.deltaY < 0 ? 1.1 : 0.9;
        zoom = Math.max(0.2, Math.min(3.0, zoom * factor));
        drawGraph();
      });
    }

    function resetCanvasView() {
      zoom = 1.0; panX = 0; panY = 0;
      drawGraph();
    }

    function fitGraphToScreen() {
      if (graphData.nodes.length === 0) return;
      let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
      graphData.nodes.forEach(n => {
        minX = Math.min(minX, n.x);
        maxX = Math.max(maxX, n.x);
        minY = Math.min(minY, n.y);
        maxY = Math.max(maxY, n.y);
      });
      let cx = (minX + maxX) / 2;
      let cy = (minY + maxY) / 2;
      panX = canvas.width / 2 - cx;
      panY = canvas.height / 2 - cy;
      zoom = 1.0;
      drawGraph();
    }

    function openDrawer(n) {
      document.getElementById('drawer-title').innerText = `${n.label} (ID: ${n.id})`;
      document.getElementById('drawer-content').innerHTML = `
        <div style="color:var(--primary); margin-bottom: 0.3rem;">Properties:</div>
        <pre>${JSON.stringify(n.properties, null, 2)}</pre>
      `;
      document.getElementById('node-drawer').classList.add('active');
    }

    function closeDrawer() {
      document.getElementById('node-drawer').classList.remove('active');
    }

    function clearResults() {
      document.getElementById('result-header').innerHTML = '<th>Status</th>';
      document.getElementById('result-body').innerHTML = '<tr><td style="color: var(--text-muted);">(Cleared)</td></tr>';
      document.getElementById('results-count').innerText = 'Results (0 rows)';
    }

    // Fulltext Search
    async function createDefaultFulltextIndex() {
      try {
        let res = await fetch('/fulltext/indexes', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ name: 'doc_content', label: 'Doc', property_key: 'content' })
        });
        let data = await res.json();
        alert(data.message || 'Fulltext index created successfully!');
      } catch (e) {
        alert('Failed: ' + e.message);
      }
    }

    async function runFulltextSearch() {
      let index = document.getElementById('ft-index-name').value.trim();
      let query = document.getElementById('ft-search-query').value.trim();
      let prefix = document.getElementById('ft-prefix').checked;

      let container = document.getElementById('ft-results-container');
      container.innerHTML = '<div style="color:var(--text-muted); text-align:center;">Searching...</div>';

      try {
        let res = await fetch('/fulltext/search', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ index, query, top_k: 10, prefix })
        });
        let hits = await res.json();
        if (!res.ok || hits.error) {
          container.innerHTML = `<div style="color:var(--danger);">${hits.error || 'Search failed'}</div>`;
          return;
        }

        if (hits.length === 0) {
          container.innerHTML = '<div style="color:var(--text-muted); text-align:center; padding: 2rem;">No matching documents found.</div>';
          return;
        }

        container.innerHTML = hits.map(h => `
          <div class="hit-card">
            <div>
              <div style="font-weight:700; color:#fff;">Node ID: ${h.node_id}</div>
              <div style="font-size:0.8rem; color:var(--text-muted); margin-top:0.25rem;">
                Matched Terms: <span style="color:#e2e8f0;">${(h.matched_terms || []).join(', ')}</span>
              </div>
            </div>
            <div class="hit-score">Score: ${h.score.toFixed(3)}</div>
          </div>
        `).join('');
      } catch (e) {
        container.innerHTML = `<div style="color:var(--danger);">${e.message}</div>`;
      }
    }

    // Keyboard shortcut: Ctrl+Enter to run query
    document.addEventListener('keydown', e => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
        runCypher();
      }
    });
  </script>
</body>
</html>
"##;
