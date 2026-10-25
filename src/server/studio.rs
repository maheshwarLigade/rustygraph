//! Embedded Interactive Web Studio for RustyGraph.
//!
//! Provides a modern, dark-mode visual web console with interactive force-directed graph rendering,
//! openCypher query editor with parameterized statement support, real-time database metrics,
//! graph algorithm visualizer, schema & index cockpit, transaction telemetry, and REST API playground.

pub const STUDIO_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>RustyGraph Studio - Engine & Dev Cockpit</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
  <style>
    :root {
      --bg: #070a12;
      --bg-panel: #0d1322;
      --card-bg: #111827;
      --card-border: #1e293b;
      --card-hover: #1f293d;
      --primary: #38bdf8;
      --primary-hover: #0ea5e9;
      --primary-glow: rgba(56, 189, 248, 0.25);
      --accent: #818cf8;
      --accent-hover: #6366f1;
      --success: #10b981;
      --warning: #f59e0b;
      --danger: #ef4444;
      --text: #f8fafc;
      --text-muted: #94a3b8;
      --font-main: 'Outfit', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      --font-mono: 'JetBrains Mono', 'Consolas', 'Monaco', monospace;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background: var(--bg);
      color: var(--text);
      font-family: var(--font-main);
      height: 100vh;
      display: flex;
      flex-direction: column;
      overflow: hidden;
      letter-spacing: -0.01em;
    }
    header {
      background: rgba(13, 19, 34, 0.85);
      backdrop-filter: blur(12px);
      border-bottom: 1px solid var(--card-border);
      padding: 0.65rem 1.5rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 1rem;
      z-index: 10;
    }
    .logo {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      font-size: 1.2rem;
      font-weight: 700;
      color: #fff;
    }
    .logo-badge {
      background: linear-gradient(135deg, #0284c7, #818cf8);
      color: white;
      font-size: 0.68rem;
      padding: 0.2rem 0.55rem;
      border-radius: 9999px;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      box-shadow: 0 0 10px rgba(56, 189, 248, 0.3);
    }
    .stats-bar {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      font-size: 0.82rem;
      color: var(--text-muted);
    }
    .stat-pill {
      background: rgba(17, 24, 39, 0.8);
      padding: 0.3rem 0.65rem;
      border-radius: 6px;
      border: 1px solid var(--card-border);
      display: flex;
      align-items: center;
      gap: 0.35rem;
      white-space: nowrap;
    }
    .stat-val { color: var(--primary); font-weight: 700; font-family: var(--font-mono); }
    .status-dot {
      width: 8px; height: 8px; border-radius: 50%; background: var(--success);
      box-shadow: 0 0 8px var(--success);
      animation: pulse 2s infinite;
    }
    @keyframes pulse {
      0%, 100% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.6; transform: scale(1.15); }
    }

    nav.tabs {
      display: flex;
      gap: 0.25rem;
      background: #090e1a;
      padding: 0.4rem 1.5rem 0;
      border-bottom: 1px solid var(--card-border);
    }
    .tab-btn {
      background: transparent;
      border: none;
      border-bottom: 2px solid transparent;
      color: var(--text-muted);
      padding: 0.55rem 1rem;
      cursor: pointer;
      font-weight: 600;
      font-size: 0.86rem;
      font-family: var(--font-main);
      display: flex;
      align-items: center;
      gap: 0.4rem;
      transition: all 0.15s ease-in-out;
    }
    .tab-btn:hover { color: #fff; background: rgba(255, 255, 255, 0.03); border-radius: 6px 6px 0 0; }
    .tab-btn.active {
      color: var(--primary);
      border-bottom-color: var(--primary);
      background: rgba(56, 189, 248, 0.05);
      border-radius: 6px 6px 0 0;
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
      overflow: hidden;
    }
    .view-panel.active { display: flex; }

    /* Cypher Editor & Explorer layout */
    .cypher-layout {
      display: flex;
      width: 100%;
      height: 100%;
    }
    .sidebar-editor {
      width: 490px;
      background: var(--card-bg);
      border-right: 1px solid var(--card-border);
      display: flex;
      flex-direction: column;
      flex-shrink: 0;
      overflow: hidden;
    }
    .editor-section {
      padding: 1rem;
      border-bottom: 1px solid var(--card-border);
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
      flex-shrink: 0;
    }
    .section-title {
      font-size: 0.76rem;
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
      background: #060911;
      border: 1px solid var(--card-border);
      border-radius: 6px;
      color: #38bdf8;
      font-family: var(--font-mono);
      font-size: 0.83rem;
      padding: 0.65rem;
      resize: vertical;
      outline: none;
      line-height: 1.45;
    }
    textarea.code-input:focus { border-color: var(--primary); box-shadow: 0 0 0 1px var(--primary-glow); }
    .btn-row {
      display: flex;
      gap: 0.5rem;
      align-items: center;
      flex-wrap: wrap;
    }
    .btn {
      background: var(--primary);
      color: #070a12;
      border: none;
      border-radius: 6px;
      padding: 0.45rem 0.85rem;
      font-weight: 600;
      font-size: 0.82rem;
      font-family: var(--font-main);
      cursor: pointer;
      display: inline-flex;
      align-items: center;
      gap: 0.35rem;
      transition: all 0.15s ease;
    }
    .btn:hover { background: var(--primary-hover); transform: translateY(-1px); }
    .btn-secondary {
      background: #1e293b;
      color: var(--text);
      border: 1px solid var(--card-border);
    }
    .btn-secondary:hover { background: #334155; color: #fff; }
    .btn-accent {
      background: linear-gradient(135deg, #4f46e5, #818cf8);
      color: white;
    }
    .btn-accent:hover { filter: brightness(1.1); }
    .btn-danger {
      background: rgba(239, 68, 68, 0.15);
      color: var(--danger);
      border: 1px solid rgba(239, 68, 68, 0.3);
    }
    .btn-danger:hover { background: var(--danger); color: white; }

    select.sample-select {
      background: #1e293b;
      color: var(--text);
      border: 1px solid var(--card-border);
      border-radius: 6px;
      padding: 0.3rem 0.6rem;
      font-size: 0.78rem;
      outline: none;
      cursor: pointer;
      font-family: var(--font-main);
    }

    .results-section {
      flex: 1;
      display: flex;
      flex-direction: column;
      overflow: hidden;
      padding: 0.75rem 1rem;
    }
    .results-table-container {
      flex: 1;
      overflow: auto;
      border: 1px solid var(--card-border);
      border-radius: 6px;
      background: #090e1a;
    }
    table.result-table {
      width: 100%;
      border-collapse: collapse;
      font-size: 0.8rem;
      font-family: var(--font-mono);
      text-align: left;
    }
    table.result-table th {
      position: sticky;
      top: 0;
      background: #111827;
      color: var(--primary);
      padding: 0.5rem 0.75rem;
      border-bottom: 1px solid var(--card-border);
      font-weight: 600;
      font-size: 0.76rem;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      z-index: 1;
    }
    table.result-table td {
      padding: 0.45rem 0.75rem;
      border-bottom: 1px solid rgba(30, 41, 59, 0.5);
      color: #e2e8f0;
      max-width: 260px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    table.result-table tr:hover td { background: rgba(56, 189, 248, 0.05); }

    /* Canvas Viewport */
    .canvas-container {
      flex: 1;
      position: relative;
      background: radial-gradient(circle at center, #0e1526 0%, #070a12 100%);
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
      z-index: 5;
    }
    .canvas-legend {
      position: absolute;
      bottom: 1rem;
      left: 1rem;
      background: rgba(13, 19, 34, 0.85);
      backdrop-filter: blur(8px);
      border: 1px solid var(--card-border);
      border-radius: 8px;
      padding: 0.5rem 0.75rem;
      display: flex;
      gap: 0.75rem;
      font-size: 0.75rem;
      color: var(--text-muted);
      z-index: 5;
    }
    .legend-item { display: flex; align-items: center; gap: 0.35rem; }
    .legend-dot { width: 10px; height: 10px; border-radius: 50%; }

    /* Node Inspector Drawer */
    .node-drawer {
      position: absolute;
      bottom: 1rem;
      right: 1rem;
      width: 320px;
      background: rgba(17, 24, 39, 0.95);
      backdrop-filter: blur(12px);
      border: 1px solid var(--card-border);
      border-radius: 10px;
      box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.6);
      padding: 1rem;
      display: none;
      flex-direction: column;
      gap: 0.6rem;
      z-index: 10;
      animation: fadeIn 0.2s ease;
    }
    .node-drawer.active { display: flex; }
    @keyframes fadeIn { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }

    .drawer-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      font-weight: 700;
      font-size: 0.95rem;
      color: #fff;
      border-bottom: 1px solid var(--card-border);
      padding-bottom: 0.4rem;
    }

    /* Modal dialog */
    .modal-backdrop {
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.7);
      backdrop-filter: blur(4px);
      display: none;
      justify-content: center;
      align-items: center;
      z-index: 100;
    }
    .modal-backdrop.active { display: flex; }
    .modal-card {
      background: #0f172a;
      border: 1px solid var(--card-border);
      border-radius: 12px;
      width: 90%;
      max-width: 650px;
      max-height: 85vh;
      display: flex;
      flex-direction: column;
      box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.7);
      overflow: hidden;
    }
    .modal-header {
      padding: 1rem 1.25rem;
      border-bottom: 1px solid var(--card-border);
      display: flex;
      justify-content: space-between;
      align-items: center;
      font-weight: 700;
      font-size: 1.05rem;
    }
    .modal-body {
      padding: 1.25rem;
      overflow-y: auto;
      display: flex;
      flex-direction: column;
      gap: 1rem;
    }

    /* Studio Shared layouts */
    .grid-dashboard {
      width: 100%;
      height: 100%;
      padding: 1.5rem;
      display: flex;
      flex-direction: column;
      gap: 1.5rem;
      overflow-y: auto;
      max-width: 1200px;
      margin: 0 auto;
    }
    .dash-hero {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 12px;
      padding: 1.5rem;
      display: flex;
      flex-direction: column;
      gap: 1rem;
    }
    .card-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
      gap: 1rem;
    }
    .metric-card {
      background: #0d1322;
      border: 1px solid var(--card-border);
      border-radius: 8px;
      padding: 1rem;
      display: flex;
      flex-direction: column;
      gap: 0.35rem;
      transition: all 0.2s ease;
    }
    .metric-card:hover { border-color: var(--primary); transform: translateY(-2px); }
    .metric-title { font-size: 0.78rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600; }
    .metric-value { font-size: 1.4rem; font-weight: 700; color: #fff; font-family: var(--font-mono); }

    /* Form controls */
    .form-group { display: flex; flex-direction: column; gap: 0.35rem; }
    .form-group label { font-size: 0.8rem; font-weight: 600; color: var(--text-muted); }
    input.text-input, select.text-input {
      background: #090d16;
      border: 1px solid var(--card-border);
      border-radius: 6px;
      color: #fff;
      font-size: 0.85rem;
      padding: 0.55rem 0.75rem;
      outline: none;
      font-family: var(--font-main);
    }
    input.text-input:focus, select.text-input:focus { border-color: var(--primary); }

    /* History pill */
    .history-item {
      padding: 0.4rem 0.6rem;
      background: #0a0f1d;
      border: 1px solid var(--card-border);
      border-radius: 6px;
      cursor: pointer;
      display: flex;
      justify-content: space-between;
      align-items: center;
      font-size: 0.78rem;
      font-family: var(--font-mono);
      transition: border-color 0.15s;
    }
    .history-item:hover { border-color: var(--primary); }

    /* Badge */
    .badge {
      display: inline-block;
      padding: 0.2rem 0.5rem;
      border-radius: 4px;
      font-size: 0.72rem;
      font-weight: 600;
      text-transform: uppercase;
    }
    .badge-primary { background: rgba(56, 189, 248, 0.15); color: var(--primary); }
    .badge-accent { background: rgba(129, 140, 248, 0.15); color: var(--accent); }
    .badge-success { background: rgba(16, 185, 129, 0.15); color: var(--success); }
    .badge-warning { background: rgba(245, 158, 11, 0.15); color: var(--warning); }
  </style>
</head>
<body>

  <!-- Top Navigation & Live Telemetry Bar -->
  <header>
    <div class="logo">
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#38bdf8" stroke-width="2.2">
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
      <div class="stat-pill"><div class="status-dot"></div> <span id="health-label" style="font-weight:600; font-size:0.78rem; color:#fff;">ONLINE</span></div>
      <div class="stat-pill">Nodes: <span class="stat-val" id="stat-nodes">0</span></div>
      <div class="stat-pill">Edges: <span class="stat-val" id="stat-edges">0</span></div>
      <div class="stat-pill">Memory: <span class="stat-val" id="stat-mem">0 KB</span></div>
      <div class="stat-pill">WAL Seq: <span class="stat-val" id="stat-wal">0</span></div>
      <div class="stat-pill">Active Tx: <span class="stat-val" id="stat-tx">0</span></div>
      <button class="btn btn-secondary" onclick="refreshStats()" title="Refresh telemetry" style="padding: 0.25rem 0.5rem; font-size: 0.75rem;">↻</button>
    </div>
  </header>

  <!-- Navigation Tabs -->
  <nav class="tabs">
    <button class="tab-btn active" id="btn-tab-cypher" onclick="switchTab('cypher')">
      <span>⚡</span> Cypher Studio & Canvas
    </button>
    <button class="tab-btn" id="btn-tab-algo" onclick="switchTab('algo')">
      <span>🕸️</span> Graph Algorithms
    </button>
    <button class="tab-btn" id="btn-tab-schema" onclick="switchTab('schema')">
      <span>📑</span> Schema & Indexes
    </button>
    <button class="tab-btn" id="btn-tab-search" onclick="switchTab('search')">
      <span>🔍</span> Hybrid Search (Vector & Full-Text)
    </button>
    <button class="tab-btn" id="btn-tab-telemetry" onclick="switchTab('telemetry')">
      <span>🩺</span> Telemetry & Transactions
    </button>
    <button class="tab-btn" id="btn-tab-api" onclick="switchTab('api')">
      <span>🔌</span> REST API Sandbox
    </button>
  </nav>

  <main>
    <!-- TAB 1: Cypher Studio & Visual Canvas -->
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
                <option value="var_len_path">Variable-Length Path (*1..3)</option>
                <option value="aggregations">Aggregation (collect & count)</option>
                <option value="merge_upsert">MERGE Upsert (Atomic)</option>
              </select>
            </div>
            <textarea id="cypher-input" class="code-input" rows="5" placeholder="MATCH (n) RETURN n LIMIT 25">MATCH (n) RETURN n LIMIT 25</textarea>

            <div class="section-title" style="margin-top: 0.25rem;">
              <span>Parameters ($param JSON)</span>
            </div>
            <textarea id="params-input" class="code-input" rows="2" placeholder='{"role": "Rust Expert", "min_xp": 5}'>{}</textarea>

            <div class="btn-row" style="margin-top: 0.4rem;">
              <button class="btn" id="btn-run-cypher" onclick="runCypher()">
                ▶ Run Query <span style="font-size: 0.72rem; opacity: 0.8;">(Ctrl+Enter)</span>
              </button>
              <button class="btn btn-secondary" onclick="explainCurrentQuery()" title="Inspect Query Plan">
                📊 Explain Plan
              </button>
              <button class="btn btn-secondary" onclick="clearResults()">Clear</button>
              <span id="exec-latency" style="font-size: 0.75rem; color: var(--text-muted); margin-left: auto; font-family: var(--font-mono);"></span>
            </div>
          </div>

          <div class="results-section">
            <div class="section-title" style="margin-bottom: 0.4rem;">
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

            <!-- Query History -->
            <div class="section-title" style="margin-top: 0.6rem; margin-bottom: 0.3rem;">
              <span>Query History</span>
              <button onclick="clearHistory()" style="background:none; border:none; color:var(--text-muted); font-size:0.72rem; cursor:pointer;">Clear</button>
            </div>
            <div id="query-history-list" style="display:flex; flex-direction:column; gap:0.35rem; max-height: 110px; overflow-y:auto;">
              <div style="font-size:0.75rem; color:var(--text-muted); font-style:italic;">No previous queries executed yet.</div>
            </div>
          </div>
        </aside>

        <!-- Interactive Canvas -->
        <div class="canvas-container">
          <canvas id="graph-canvas"></canvas>
          <div class="canvas-overlay">
            <button class="btn btn-secondary" onclick="resetCanvasView()" title="Reset Zoom">⟲ Reset</button>
            <button class="btn btn-secondary" onclick="fitGraphToScreen()" title="Fit Graph">⛶ Fit Graph</button>
          </div>

          <div class="canvas-legend" id="canvas-legend">
            <div class="legend-item"><div class="legend-dot" style="background:#38bdf8;"></div> Node</div>
            <div class="legend-item"><div class="legend-dot" style="background:#818cf8;"></div> Person</div>
            <div class="legend-item"><div class="legend-dot" style="background:#10b981;"></div> Database</div>
            <div class="legend-item"><div class="legend-dot" style="background:#f59e0b;"></div> Concept</div>
          </div>

          <!-- Node Details Drawer -->
          <div id="node-drawer" class="node-drawer">
            <div class="drawer-header">
              <span id="drawer-title">Node Details</span>
              <button onclick="closeDrawer()" style="background:none; border:none; color:#fff; cursor:pointer; font-size:1rem;">✕</button>
            </div>
            <div id="drawer-content" style="max-height: 220px; overflow-y: auto; font-family: var(--font-mono); font-size: 0.8rem;"></div>
            <div style="display: flex; gap: 0.5rem; margin-top: 0.4rem;">
              <button class="btn btn-secondary" style="flex:1; font-size:0.75rem;" onclick="editNodePrompt()">+ Set Property</button>
              <button class="btn btn-danger" style="font-size:0.75rem;" onclick="deleteSelectedNode()">Delete</button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- TAB 2: Graph Algorithm Studio -->
    <div id="tab-algo" class="view-panel">
      <div class="grid-dashboard">
        <div class="dash-hero">
          <h2 style="font-size: 1.25rem; display: flex; align-items: center; gap: 0.5rem;">
            <span>🕸️</span> Graph Data Science & Algorithm Visualizer
          </h2>
          <p style="color: var(--text-muted); font-size: 0.85rem;">
            Execute high-performance in-memory graph algorithms (PageRank, Dijkstra Shortest Path, Louvain / Connected Components, Betweenness Centrality) and project results onto the interactive graph canvas.
          </p>

          <div style="display: flex; gap: 1rem; align-items: flex-end; flex-wrap: wrap;">
            <div class="form-group" style="min-width: 220px;">
              <label>Select Algorithm</label>
              <select id="algo-type-select" class="text-input" onchange="updateAlgoForm(this.value)">
                <option value="pagerank">PageRank Centrality</option>
                <option value="shortest_path">Shortest Path (BFS / Dijkstra)</option>
                <option value="betweenness">Betweenness Centrality</option>
                <option value="degree">Degree Centrality</option>
                <option value="community_wcc">Connected Components (WCC)</option>
                <option value="community_scc">Strongly Connected Components (SCC)</option>
                <option value="similarity">Jaccard Similarity</option>
              </select>
            </div>

            <!-- Param inputs -->
            <div id="algo-param-container" style="display: flex; gap: 0.75rem; align-items: flex-end; flex-wrap: wrap;">
              <div class="form-group">
                <label>Damping Factor</label>
                <input type="number" id="algo-param-damping" class="text-input" style="width: 110px;" value="0.85" step="0.05" min="0.1" max="0.99">
              </div>
              <div class="form-group">
                <label>Max Iterations</label>
                <input type="number" id="algo-param-iter" class="text-input" style="width: 100px;" value="50" min="1" max="500">
              </div>
            </div>

            <button class="btn btn-accent" onclick="runSelectedAlgorithm()">
              ⚡ Run Algorithm
            </button>
            <button class="btn btn-secondary" onclick="applyAlgoToCanvas()" id="btn-apply-canvas" style="display: none;">
              🎨 Visualize on Canvas
            </button>
          </div>
        </div>

        <!-- Algorithm Results Table -->
        <div class="dash-hero" style="flex:1;">
          <div class="section-title">
            <span id="algo-results-header">Algorithm Results</span>
            <span id="algo-latency-badge" class="badge badge-primary">Ready</span>
          </div>
          <div style="overflow-x: auto; flex:1;">
            <table id="algo-results-table" class="result-table" style="width:100%; border:1px solid var(--card-border); border-radius:6px;">
              <thead>
                <tr id="algo-table-head"><th>Rank</th><th>Node ID</th><th>Score</th><th>Details</th></tr>
              </thead>
              <tbody id="algo-table-body">
                <tr><td colspan="4" style="color:var(--text-muted); text-align:center; padding:2rem;">Choose an algorithm above and click "Run Algorithm".</td></tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>

    <!-- TAB 3: Schema & Index Cockpit -->
    <div id="tab-schema" class="view-panel">
      <div class="grid-dashboard">
        <div class="dash-hero">
          <div style="display: flex; justify-content: space-between; align-items: center;">
            <div>
              <h2 style="font-size: 1.25rem;">📑 Schema Catalog & Index Management</h2>
              <p style="color: var(--text-muted); font-size: 0.85rem; margin-top: 0.25rem;">
                Inspect active node labels, relationship types, property keys, and maintain high-performance B-tree, vector, and full-text indexes.
              </p>
            </div>
            <button class="btn" onclick="openCreateIndexModal()">+ Create Index</button>
          </div>
        </div>

        <!-- Schema Overview Cards -->
        <div class="card-grid" id="schema-cards-container">
          <div class="metric-card"><div class="metric-title">Total Labels</div><div class="metric-value" id="schema-lbl-count">0</div></div>
          <div class="metric-card"><div class="metric-title">Relationship Types</div><div class="metric-value" id="schema-rel-count">0</div></div>
          <div class="metric-card"><div class="metric-title">Active Indexes</div><div class="metric-value" id="schema-idx-count">0</div></div>
          <div class="metric-card"><div class="metric-title">Graph Density</div><div class="metric-value" id="schema-density">0.000</div></div>
        </div>

        <!-- Indexes Table -->
        <div class="dash-hero">
          <div class="section-title">
            <span>Active Indexes</span>
            <button class="btn btn-secondary" onclick="loadSchemaData()" style="padding:0.25rem 0.5rem; font-size:0.75rem;">↻ Refresh</button>
          </div>
          <div style="overflow-x: auto;">
            <table class="result-table" style="width:100%; border:1px solid var(--card-border); border-radius:6px;">
              <thead>
                <tr><th>Index Type</th><th>Name / Key</th><th>Target (Label : Property)</th><th>Configuration</th><th>Action</th></tr>
              </thead>
              <tbody id="indexes-table-body">
                <tr><td colspan="5" style="color:var(--text-muted); text-align:center;">Loading index catalog...</td></tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>

    <!-- TAB 4: Hybrid Search Studio (Vector & Full-Text) -->
    <div id="tab-search" class="view-panel">
      <div class="grid-dashboard">
        <div class="dash-hero">
          <h2 style="font-size: 1.25rem;">🔍 Hybrid Search Engine (Vector k-NN & BM25 Full-Text)</h2>
          <p style="color: var(--text-muted); font-size: 0.85rem;">
            Test inverted keyword full-text search with tokenization, BM25 scoring, and vector similarity search.
          </p>

          <div style="display:flex; gap:0.5rem; border-bottom:1px solid var(--card-border); padding-bottom:0.5rem;">
            <button class="btn btn-secondary" id="btn-mode-fulltext" onclick="toggleSearchMode('fulltext')">Full-Text (BM25)</button>
            <button class="btn btn-secondary" id="btn-mode-vector" onclick="toggleSearchMode('vector')">Vector Similarity</button>
          </div>

          <!-- Fulltext Search Form -->
          <div id="search-form-fulltext" style="display:flex; flex-direction:column; gap:0.75rem;">
            <div style="display: flex; gap: 0.75rem;">
              <input id="ft-index-name" class="text-input" style="max-width: 200px;" placeholder="Index name" value="doc_content">
              <input id="ft-search-query" class="text-input" style="flex:1;" placeholder="Enter keywords (e.g. rust distributed graph database)...">
              <button class="btn" onclick="runFulltextSearch()">Search</button>
            </div>
            <div style="display: flex; gap: 1rem; font-size: 0.82rem; align-items: center;">
              <label style="display:flex; align-items:center; gap:0.4rem; cursor:pointer;">
                <input type="checkbox" id="ft-prefix"> Enable Prefix Matching
              </label>
            </div>
          </div>

          <!-- Vector Search Form -->
          <div id="search-form-vector" style="display:none; flex-direction:column; gap:0.75rem;">
            <div style="display: flex; gap: 0.75rem;">
              <input id="vec-index-name" class="text-input" style="max-width: 200px;" placeholder="Index name" value="embeddings">
              <input id="vec-query-input" class="text-input" style="flex:1;" placeholder="Enter float vector: [0.12, 0.45, -0.08, ...]">
              <button class="btn" onclick="runVectorSearch()">k-NN Search</button>
            </div>
          </div>
        </div>

        <div class="dash-hero">
          <div class="section-title">
            <span>Search Results</span>
          </div>
          <div id="search-results-container" style="display: flex; flex-direction: column; gap: 0.75rem;">
            <div style="color: var(--text-muted); font-size: 0.85rem; text-align: center; padding: 2rem;">
              No query executed yet. Run a search above.
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- TAB 5: Live Telemetry & Transaction Monitor -->
    <div id="tab-telemetry" class="view-panel">
      <div class="grid-dashboard">
        <div class="dash-hero">
          <div style="display:flex; justify-content:space-between; align-items:center;">
            <div>
              <h2 style="font-size: 1.25rem;">🩺 Engine Health & Transaction Monitor</h2>
              <p style="color: var(--text-muted); font-size: 0.85rem; margin-top: 0.25rem;">
                Inspect active ACID transactions with snapshot isolation, WAL sequence progression, and database durability controls.
              </p>
            </div>
            <div style="display:flex; gap:0.5rem;">
              <button class="btn btn-secondary" onclick="triggerCheckpoint()">⚡ Force Checkpoint</button>
              <button class="btn btn-secondary" onclick="exportGraphJson()">💾 Download Backup</button>
              <button class="btn btn-secondary" onclick="openImportModal()">📥 Import JSON</button>
            </div>
          </div>
        </div>

        <!-- Telemetry Gauges -->
        <div class="card-grid">
          <div class="metric-card"><div class="metric-title">Active Nodes</div><div class="metric-value" id="tel-nodes">0</div></div>
          <div class="metric-card"><div class="metric-title">Active Relationships</div><div class="metric-value" id="tel-edges">0</div></div>
          <div class="metric-card"><div class="metric-title">Memory Allocation</div><div class="metric-value" id="tel-mem">0 KB</div></div>
          <div class="metric-card"><div class="metric-title">WAL Sequence</div><div class="metric-value" id="tel-wal">0</div></div>
          <div class="metric-card"><div class="metric-title">Uncheckpointed Ops</div><div class="metric-value" id="tel-ops">0</div></div>
          <div class="metric-card"><div class="metric-title">Uptime</div><div class="metric-value" id="tel-uptime">0s</div></div>
        </div>

        <!-- Active Transactions Table -->
        <div class="dash-hero">
          <div class="section-title">
            <span>Active Transactions (ACID Snapshot Isolation)</span>
            <button class="btn btn-secondary" onclick="refreshTransactions()" style="padding:0.25rem 0.5rem; font-size:0.75rem;">↻ Refresh</button>
          </div>
          <div style="overflow-x:auto;">
            <table class="result-table" style="width:100%; border:1px solid var(--card-border); border-radius:6px;">
              <thead>
                <tr><th>Transaction ID</th><th>Remaining Timeout</th><th>Staged Mutations</th><th>Actions</th></tr>
              </thead>
              <tbody id="tx-table-body">
                <tr><td colspan="4" style="color:var(--text-muted); text-align:center; padding:1.5rem;">No active transactions running.</td></tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>

    <!-- TAB 6: REST API Sandbox -->
    <div id="tab-api" class="view-panel">
      <div class="grid-dashboard">
        <div class="dash-hero">
          <h2 style="font-size: 1.25rem;">🔌 Interactive REST API Sandbox</h2>
          <p style="color: var(--text-muted); font-size: 0.85rem;">
            Test any RustyGraph HTTP endpoint directly from the browser with syntax-highlighted request and response inspector.
          </p>

          <div style="display:flex; gap:0.75rem; align-items:center;">
            <select id="api-endpoint-select" class="text-input" style="max-width:320px;" onchange="loadApiTemplate(this.value)">
              <option value="get_stats">GET /stats</option>
              <option value="get_schema">GET /schema</option>
              <option value="get_indexes">GET /indexes</option>
              <option value="post_cypher">POST /cypher</option>
              <option value="post_explain">POST /cypher/explain</option>
              <option value="post_pagerank">POST /algo/pagerank</option>
              <option value="post_shortest_path">POST /algo/shortest-path</option>
              <option value="post_centrality">POST /algo/centrality</option>
              <option value="post_community">POST /algo/community</option>
              <option value="post_similarity">POST /algo/similarity</option>
              <option value="post_nodes">POST /nodes</option>
              <option value="post_edges">POST /edges</option>
              <option value="get_tx_list">GET /tx/list</option>
              <option value="get_metrics">GET /metrics</option>
            </select>
            <button class="btn btn-accent" onclick="executeApiSandbox()">Send Request</button>
            <span id="api-status-badge" class="badge badge-primary" style="display:none;"></span>
          </div>

          <div style="display:grid; grid-template-columns: 1fr 1fr; gap:1rem; margin-top:0.5rem;">
            <div class="form-group">
              <label>Request Body (JSON)</label>
              <textarea id="api-request-body" class="code-input" rows="8">{}</textarea>
            </div>
            <div class="form-group">
              <label>Response</label>
              <textarea id="api-response-body" class="code-input" rows="8" readonly style="color:#a7f3d0;">{}</textarea>
            </div>
          </div>
        </div>
      </div>
    </div>
  </main>

  <!-- Query Plan EXPLAIN Modal -->
  <div id="modal-explain" class="modal-backdrop">
    <div class="modal-card">
      <div class="modal-header">
        <span>📊 Query Execution Plan</span>
        <button onclick="closeExplainModal()" style="background:none; border:none; color:#fff; cursor:pointer; font-size:1.1rem;">✕</button>
      </div>
      <div class="modal-body">
        <div style="color:var(--primary); font-size:0.85rem; font-weight:600;">Plan Operator Tree:</div>
        <pre id="explain-plan-tree" style="background:#070a12; padding:1rem; border-radius:6px; font-family:var(--font-mono); font-size:0.82rem; color:#38bdf8; overflow-x:auto;"></pre>
      </div>
    </div>
  </div>

  <!-- Create Index Modal -->
  <div id="modal-create-index" class="modal-backdrop">
    <div class="modal-card">
      <div class="modal-header">
        <span>Create Database Index</span>
        <button onclick="closeCreateIndexModal()" style="background:none; border:none; color:#fff; cursor:pointer; font-size:1.1rem;">✕</button>
      </div>
      <div class="modal-body">
        <div class="form-group">
          <label>Index Category</label>
          <select id="modal-idx-type" class="text-input" onchange="updateCreateIndexModalFields(this.value)">
            <option value="property">Secondary Property Index (B-Tree)</option>
            <option value="vector">Vector Index (Cosine / Euclidean)</option>
            <option value="fulltext">Full-Text Inverted Index (BM25)</option>
          </select>
        </div>
        <div class="form-group" id="group-idx-name" style="display:none;">
          <label>Index Name</label>
          <input type="text" id="modal-idx-name" class="text-input" placeholder="e.g. doc_embeddings">
        </div>
        <div class="form-group">
          <label>Target Node Label</label>
          <input type="text" id="modal-idx-label" class="text-input" placeholder="e.g. Person">
        </div>
        <div class="form-group">
          <label>Target Property Key</label>
          <input type="text" id="modal-idx-prop" class="text-input" placeholder="e.g. email or embedding">
        </div>
        <div class="form-group" id="group-idx-dim" style="display:none;">
          <label>Vector Dimension</label>
          <input type="number" id="modal-idx-dim" class="text-input" value="128">
        </div>
        <div style="display:flex; justify-content:flex-end; gap:0.5rem; margin-top:0.5rem;">
          <button class="btn btn-secondary" onclick="closeCreateIndexModal()">Cancel</button>
          <button class="btn" onclick="submitCreateIndex()">Create Index</button>
        </div>
      </div>
    </div>
  </div>

  <!-- Import Modal -->
  <div id="modal-import" class="modal-backdrop">
    <div class="modal-card">
      <div class="modal-header">
        <span>Import Database JSON Backup</span>
        <button onclick="closeImportModal()" style="background:none; border:none; color:#fff; cursor:pointer; font-size:1.1rem;">✕</button>
      </div>
      <div class="modal-body">
        <p style="color:var(--text-muted); font-size:0.85rem;">Paste full graph JSON dump below to restore graph topology.</p>
        <textarea id="import-json-text" class="code-input" rows="8" placeholder='{"nodes": [...], "relationships": [...]}'></textarea>
        <div style="display:flex; justify-content:flex-end; gap:0.5rem;">
          <button class="btn btn-secondary" onclick="closeImportModal()">Cancel</button>
          <button class="btn btn-accent" onclick="submitImportJson()">Import Now</button>
        </div>
      </div>
    </div>
  </div>

  <script>
    // Visual Graph Simulation State
    let graphData = { nodes: [], edges: [] };
    let canvas = document.getElementById('graph-canvas');
    let ctx = canvas.getContext('2d');
    let zoom = 1.0, panX = 0, panY = 0;
    let draggingNode = null, selectedNode = null, isPanning = false, startPanX = 0, startPanY = 0;
    let lastAlgoOverlay = null;
    let queryHistory = [];

    function resizeCanvas() {
      canvas.width = canvas.parentElement.clientWidth;
      canvas.height = canvas.parentElement.clientHeight;
      drawGraph();
    }
    window.addEventListener('resize', resizeCanvas);

    window.addEventListener('load', () => {
      resizeCanvas();
      refreshStats();
      setInterval(refreshStats, 4000);
      setupCanvasInteractions();
      runCypher();
      loadApiTemplate('get_stats');
    });

    function switchTab(name) {
      document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
      document.querySelectorAll('.view-panel').forEach(p => p.classList.remove('active'));
      document.getElementById('btn-tab-' + name).classList.add('active');
      document.getElementById('tab-' + name).classList.add('active');
      if (name === 'cypher') resizeCanvas();
      if (name === 'schema') loadSchemaData();
      if (name === 'telemetry') { refreshStats(); refreshTransactions(); }
    }

    async function refreshStats() {
      try {
        let res = await fetch('/stats');
        if (res.ok) {
          let s = await res.json();
          document.getElementById('stat-nodes').innerText = s.node_count || 0;
          document.getElementById('stat-edges').innerText = s.edge_count || 0;
          let memKb = ((s.estimated_memory_bytes || 0) / 1024).toFixed(1);
          document.getElementById('stat-mem').innerText = memKb + ' KB';
          document.getElementById('stat-wal').innerText = s.current_wal_sequence || 0;
          document.getElementById('stat-tx').innerText = s.active_transactions || 0;

          // Telemetry tab gauges
          let telNodes = document.getElementById('tel-nodes');
          if (telNodes) {
            telNodes.innerText = s.node_count || 0;
            document.getElementById('tel-edges').innerText = s.edge_count || 0;
            document.getElementById('tel-mem').innerText = memKb + ' KB';
            document.getElementById('tel-wal').innerText = s.current_wal_sequence || 0;
            document.getElementById('tel-ops').innerText = s.uncheckpointed_ops || 0;
            document.getElementById('tel-uptime').innerText = (s.uptime_seconds || 0) + 's';
          }
        }
      } catch (e) {
        console.warn('Stats fetch failed', e);
      }
    }

    const SAMPLES = {
      create_sample: `CREATE (a:Person {name: "Alice", role: "Rust Expert", xp: 8})
CREATE (b:Person {name: "Bob", role: "Graph Architect", xp: 6})
CREATE (c:Person {name: "Charlie", role: "AI Researcher", xp: 4})
CREATE (rg:Database {name: "RustyGraph", type: "Native Graph", storage: "Hybrid WAL + In-Memory"})
CREATE (a)-[:CONTRIBUTES_TO {since: 2026, weight: 5.0}]->(rg)
CREATE (b)-[:CONTRIBUTES_TO {since: 2026, weight: 4.0}]->(rg)
CREATE (c)-[:CONTRIBUTES_TO {since: 2026, weight: 3.0}]->(rg)
CREATE (a)-[:COLLABORATES_WITH {weight: 2.0}]->(b)
CREATE (b)-[:COLLABORATES_WITH {weight: 1.5}]->(c)
RETURN a, b, c, rg`,
      match_all: `MATCH (n) RETURN n LIMIT 25`,
      match_params: `MATCH (n:Person) WHERE n.role = $role AND n.xp >= $min_xp RETURN n`,
      var_len_path: `MATCH (a:Person {name: "Alice"})-[*1..3]->(target) RETURN DISTINCT target`,
      aggregations: `MATCH (p:Person)-[:CONTRIBUTES_TO]->(d:Database) RETURN d.name, count(p) as contributor_count, collect(p.name) as contributors`,
      merge_upsert: `MERGE (u:Person {name: "Diana", role: "Platform Engineer"})
ON CREATE SET u.joined = 2026, u.visits = 1
ON MATCH SET u.visits = 2
RETURN u.name, u.role, u.visits`
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
        let latency = (t1 - t0).toFixed(1);
        document.getElementById('exec-latency').innerText = `${latency} ms`;

        let data = await res.json();
        if (!res.ok || data.error) {
          renderError(data.error || 'Query failed');
          addQueryHistory(query, latency, false);
        } else {
          renderResults(data);
          buildGraphFromCypher(data);
          refreshStats();
          addQueryHistory(query, latency, true);
        }
      } catch (e) {
        renderError(e.message);
        addQueryHistory(query, 0, false);
      }
    }

    async function explainCurrentQuery() {
      let query = document.getElementById('cypher-input').value.trim();
      try {
        let res = await fetch('/cypher/explain', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ query })
        });
        let data = await res.json();
        if (res.ok && data.success) {
          document.getElementById('explain-plan-tree').innerText = data.plan_tree || JSON.stringify(data.plan, null, 2);
          document.getElementById('modal-explain').classList.add('active');
        } else {
          alert('Explain failed: ' + (data.error || 'Invalid query'));
        }
      } catch (e) {
        alert('Explain failed: ' + e.message);
      }
    }

    function closeExplainModal() {
      document.getElementById('modal-explain').classList.remove('active');
    }

    function addQueryHistory(q, lat, success) {
      queryHistory.unshift({ q, lat, success, time: new Date().toLocaleTimeString() });
      if (queryHistory.length > 8) queryHistory.pop();
      let container = document.getElementById('query-history-list');
      container.innerHTML = queryHistory.map((item, idx) => `
        <div class="history-item" onclick="loadHistoryItem(${idx})">
          <span style="overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:300px;">${escapeHtml(item.q)}</span>
          <span class="badge ${item.success ? 'badge-success' : 'badge-warning'}">${item.lat}ms</span>
        </div>
      `).join('');
    }

    function loadHistoryItem(idx) {
      if (queryHistory[idx]) {
        document.getElementById('cypher-input').value = queryHistory[idx].q;
      }
    }

    function clearHistory() {
      queryHistory = [];
      document.getElementById('query-history-list').innerHTML = '<div style="font-size:0.75rem; color:var(--text-muted); font-style:italic;">History cleared.</div>';
    }

    function renderError(msg) {
      document.getElementById('results-count').innerText = 'Query Error';
      document.getElementById('result-header').innerHTML = '<th>Error</th>';
      document.getElementById('result-body').innerHTML = `<tr><td style="color: var(--danger);">${escapeHtml(msg)}</td></tr>`;
    }

    function renderResults(data) {
      let cols = data.columns || [];
      let rows = data.rows || [];
      document.getElementById('results-count').innerText = `Results (${rows.length} rows)`;

      let hHtml = cols.map(c => `<th>${escapeHtml(c)}</th>`).join('');
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
      if (typeof val === 'object') return escapeHtml(JSON.stringify(val));
      return escapeHtml(String(val));
    }

    function escapeHtml(str) {
      return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
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
                let label = (cell.labels && cell.labels[0]) || 'Node';
                let name = (cell.properties && (cell.properties.name || cell.properties.title || cell.properties.label)) || `Node ${cell.id}`;
                newNodes.set(cell.id, {
                  id: cell.id,
                  label,
                  name,
                  properties: cell.properties || {},
                  radius: 22,
                  x: existing ? existing.x : (canvas.width / 2 + (Math.random() - 0.5) * 250),
                  y: existing ? existing.y : (canvas.height / 2 + (Math.random() - 0.5) * 250),
                  vx: 0, vy: 0
                });
              }
            } else if ('source' in cell && 'target' in cell && 'type' in cell) {
              newEdges.push({
                id: cell.id,
                sourceId: cell.source,
                targetId: cell.target,
                type: cell.type || 'RELATED',
                properties: cell.properties || {}
              });
            }
          }
        });
      });

      graphData.nodes = Array.from(newNodes.values());

      // If explicit edges weren't extracted, connect consecutive nodes
      if (newEdges.length === 0 && graphData.nodes.length > 1) {
        for (let i = 0; i < graphData.nodes.length - 1; i++) {
          newEdges.push({
            sourceId: graphData.nodes[i].id,
            targetId: graphData.nodes[i + 1].id,
            type: 'CONNECTED'
          });
        }
      }

      // Resolve edge source/target references
      let resolvedEdges = [];
      newEdges.forEach(e => {
        let src = graphData.nodes.find(n => n.id === e.sourceId);
        let dst = graphData.nodes.find(n => n.id === e.targetId);
        if (src && dst) {
          resolvedEdges.push({ source: src, target: dst, type: e.type, properties: e.properties });
        }
      });
      graphData.edges = resolvedEdges;

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
      const k = 130;
      const repulsion = 4500;
      const damping = 0.85;

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

      graphData.nodes.forEach(n => {
        if (n === draggingNode) return;
        n.x += n.vx;
        n.y += n.vy;
        n.vx *= damping;
        n.vy *= damping;
      });
    }

    const LABEL_COLORS = {
      Person: '#818cf8',
      Database: '#10b981',
      Doc: '#f59e0b',
      Concept: '#ec4899',
      Organization: '#06b6d4',
      Company: '#06b6d4',
      Default: '#38bdf8'
    };

    function getNodeColor(node) {
      if (node.communityColor) return node.communityColor;
      return LABEL_COLORS[node.label] || LABEL_COLORS.Default;
    }

    function drawGraph() {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.save();
      ctx.translate(panX, panY);
      ctx.scale(zoom, zoom);

      // Draw Edges
      graphData.edges.forEach(e => {
        let isHighlighted = lastAlgoOverlay && lastAlgoOverlay.highlightEdges && lastAlgoOverlay.highlightEdges.has(`${e.source.id}->${e.target.id}`);
        ctx.strokeStyle = isHighlighted ? '#f59e0b' : 'rgba(100, 116, 139, 0.5)';
        ctx.lineWidth = isHighlighted ? 3 : 1.5;

        ctx.beginPath();
        ctx.moveTo(e.source.x, e.source.y);
        ctx.lineTo(e.target.x, e.target.y);
        ctx.stroke();

        // Draw relationship type label pill
        let midX = (e.source.x + e.target.x) / 2;
        let midY = (e.source.y + e.target.y) / 2;
        if (e.type) {
          ctx.font = '500 10px monospace';
          let tw = ctx.measureText(e.type).width;
          ctx.fillStyle = '#0f172a';
          ctx.fillRect(midX - tw/2 - 4, midY - 6, tw + 8, 13);
          ctx.fillStyle = isHighlighted ? '#fbbf24' : '#94a3b8';
          ctx.textAlign = 'center';
          ctx.fillText(e.type, midX, midY + 4);
        }
      });

      // Draw Nodes
      graphData.nodes.forEach(n => {
        let color = getNodeColor(n);
        let radius = n.radius || 22;
        let isSelected = selectedNode === n;

        // Glowing selection halo
        if (isSelected) {
          ctx.beginPath();
          ctx.arc(n.x, n.y, radius + 6, 0, 2 * Math.PI);
          ctx.fillStyle = 'rgba(56, 189, 248, 0.35)';
          ctx.fill();
        }

        // Main Node Body
        ctx.beginPath();
        ctx.arc(n.x, n.y, radius, 0, 2 * Math.PI);
        ctx.fillStyle = color;
        ctx.fill();
        ctx.strokeStyle = isSelected ? '#38bdf8' : '#ffffff';
        ctx.lineWidth = isSelected ? 3 : 2;
        ctx.stroke();

        // Node ID or short badge inside
        ctx.fillStyle = '#ffffff';
        ctx.font = '700 11px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(n.id, n.x, n.y + 4);

        // Subtitle Name Text Under Node
        ctx.fillStyle = '#f8fafc';
        ctx.font = '600 11px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(n.name, n.x, n.y + radius + 14);
      });

      ctx.restore();
    }

    function setupCanvasInteractions() {
      canvas.addEventListener('mousedown', e => {
        let rect = canvas.getBoundingClientRect();
        let mouseX = (e.clientX - rect.left - panX) / zoom;
        let mouseY = (e.clientY - rect.top - panY) / zoom;

        for (let n of graphData.nodes) {
          let dx = n.x - mouseX;
          let dy = n.y - mouseY;
          let r = n.radius || 22;
          if (Math.sqrt(dx * dx + dy * dy) <= r) {
            draggingNode = n;
            selectedNode = n;
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
        zoom = Math.max(0.15, Math.min(3.5, zoom * factor));
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
        <div style="color:var(--primary); margin-bottom: 0.2rem; font-weight:600;">Labels: [${n.label}]</div>
        <div style="color:var(--primary); margin-bottom: 0.2rem; font-weight:600;">Properties:</div>
        <pre style="background:#070a12; padding:0.5rem; border-radius:4px; max-height:140px; overflow-y:auto;">${escapeHtml(JSON.stringify(n.properties, null, 2))}</pre>
      `;
      document.getElementById('node-drawer').classList.add('active');
    }

    function closeDrawer() {
      document.getElementById('node-drawer').classList.remove('active');
      selectedNode = null;
      drawGraph();
    }

    async function editNodePrompt() {
      if (!selectedNode) return;
      let propKey = prompt(`Enter property key to set on node ${selectedNode.id}:`, "tag");
      if (!propKey) return;
      let propVal = prompt(`Enter property value for "${propKey}":`, "demo");
      if (propVal === null) return;

      try {
        let q = `MATCH (n) WHERE id(n) = ${selectedNode.id} SET n.${propKey} = "${propVal}" RETURN n`;
        let res = await fetch('/cypher', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ query: q })
        });
        if (res.ok) {
          selectedNode.properties[propKey] = propVal;
          openDrawer(selectedNode);
          drawGraph();
          refreshStats();
        }
      } catch (e) {
        alert('Failed: ' + e.message);
      }
    }

    async function deleteSelectedNode() {
      if (!selectedNode) return;
      if (!confirm(`Delete node ${selectedNode.id} and all connected relationships?`)) return;
      try {
        let res = await fetch(`/nodes/${selectedNode.id}`, { method: 'DELETE' });
        if (res.ok) {
          graphData.nodes = graphData.nodes.filter(n => n.id !== selectedNode.id);
          graphData.edges = graphData.edges.filter(e => e.source.id !== selectedNode.id && e.target.id !== selectedNode.id);
          closeDrawer();
          drawGraph();
          refreshStats();
        } else {
          alert('Delete failed');
        }
      } catch (e) {
        alert('Delete failed: ' + e.message);
      }
    }

    function clearResults() {
      document.getElementById('result-header').innerHTML = '<th>Status</th>';
      document.getElementById('result-body').innerHTML = '<tr><td style="color: var(--text-muted);">(Cleared)</td></tr>';
      document.getElementById('results-count').innerText = 'Results (0 rows)';
    }

    // =========================================================================
    // TAB 2: Graph Algorithm Studio
    // =========================================================================

    let currentAlgoResults = null;

    function updateAlgoForm(type) {
      let c = document.getElementById('algo-param-container');
      if (type === 'pagerank') {
        c.innerHTML = `
          <div class="form-group"><label>Damping Factor</label><input type="number" id="algo-param-damping" class="text-input" style="width:110px;" value="0.85" step="0.05"></div>
          <div class="form-group"><label>Max Iterations</label><input type="number" id="algo-param-iter" class="text-input" style="width:100px;" value="50"></div>
        `;
      } else if (type === 'shortest_path') {
        c.innerHTML = `
          <div class="form-group"><label>Source Node ID</label><input type="number" id="algo-sp-src" class="text-input" style="width:120px;" placeholder="Node ID" value="1"></div>
          <div class="form-group"><label>Target Node ID</label><input type="number" id="algo-sp-dst" class="text-input" style="width:120px;" placeholder="Node ID" value="4"></div>
          <div class="form-group"><label>Method</label><select id="algo-sp-method" class="text-input"><option value="bfs">BFS (Hops)</option><option value="dijkstra">Dijkstra (Weighted)</option></select></div>
        `;
      } else if (type === 'betweenness' || type === 'degree') {
        c.innerHTML = `
          <div class="form-group"><label>Direction</label><select id="algo-cent-dir" class="text-input"><option value="both">Both</option><option value="outgoing">Outgoing</option><option value="incoming">Incoming</option></select></div>
        `;
      } else if (type === 'similarity') {
        c.innerHTML = `
          <div class="form-group"><label>Node 1 ID</label><input type="number" id="algo-sim-src" class="text-input" style="width:110px;" value="1"></div>
          <div class="form-group"><label>Node 2 ID</label><input type="number" id="algo-sim-dst" class="text-input" style="width:110px;" value="2"></div>
        `;
      } else {
        c.innerHTML = `<span style="font-size:0.8rem; color:var(--text-muted);">No additional parameters required.</span>`;
      }
    }

    async function runSelectedAlgorithm() {
      let type = document.getElementById('algo-type-select').value;
      let t0 = performance.now();
      let resBadge = document.getElementById('algo-latency-badge');
      resBadge.innerText = 'Running...';

      try {
        let endpoint = '', payload = {};
        if (type === 'pagerank') {
          endpoint = '/algo/pagerank';
          payload = {
            damping: parseFloat(document.getElementById('algo-param-damping').value) || 0.85,
            max_iterations: parseInt(document.getElementById('algo-param-iter').value) || 50
          };
        } else if (type === 'shortest_path') {
          endpoint = '/algo/shortest-path';
          payload = {
            source: parseInt(document.getElementById('algo-sp-src').value) || 0,
            target: parseInt(document.getElementById('algo-sp-dst').value) || 0,
            algorithm: document.getElementById('algo-sp-method').value || 'bfs',
            weight_property: 'weight'
          };
        } else if (type === 'betweenness') {
          endpoint = '/algo/centrality';
          payload = { metric: 'betweenness', direction: document.getElementById('algo-cent-dir').value || 'both', normalized: true };
        } else if (type === 'degree') {
          endpoint = '/algo/centrality';
          payload = { metric: 'degree', direction: document.getElementById('algo-cent-dir').value || 'both' };
        } else if (type === 'community_wcc') {
          endpoint = '/algo/community';
          payload = { algorithm: 'wcc' };
        } else if (type === 'community_scc') {
          endpoint = '/algo/community';
          payload = { algorithm: 'scc' };
        } else if (type === 'similarity') {
          endpoint = '/algo/similarity';
          payload = {
            source: parseInt(document.getElementById('algo-sim-src').value) || 0,
            target: parseInt(document.getElementById('algo-sim-dst').value) || 0,
            metric: 'jaccard'
          };
        }

        let res = await fetch(endpoint, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });
        let data = await res.json();
        let t1 = performance.now();
        resBadge.innerText = `${(t1 - t0).toFixed(1)} ms`;
        currentAlgoResults = { type, data };

        renderAlgorithmResults(type, data);
        document.getElementById('btn-apply-canvas').style.display = 'inline-flex';
      } catch (e) {
        resBadge.innerText = 'Error';
        alert('Algorithm error: ' + e.message);
      }
    }

    function renderAlgorithmResults(type, data) {
      let head = document.getElementById('algo-table-head');
      let body = document.getElementById('algo-table-body');

      if (type === 'pagerank' || type === 'betweenness' || type === 'degree') {
        head.innerHTML = `<th>Rank</th><th>Node ID</th><th>Score / Degree</th><th>Labels & Name</th>`;
        let nodes = data.top_nodes || [];
        if (nodes.length === 0) {
          body.innerHTML = `<tr><td colspan="4" style="color:var(--text-muted); text-align:center;">No results returned.</td></tr>`;
          return;
        }
        body.innerHTML = nodes.slice(0, 50).map((n, i) => `
          <tr>
            <td><strong>#${i + 1}</strong></td>
            <td><code>${n.node_id}</code></td>
            <td style="color:var(--primary); font-weight:700;">${(n.score !== undefined ? n.score.toFixed(5) : n.degree)}</td>
            <td>${escapeHtml((n.labels || []).join(', '))} ${n.properties && n.properties.name ? `("${escapeHtml(n.properties.name)}")` : ''}</td>
          </tr>
        `).join('');
      } else if (type === 'shortest_path') {
        head.innerHTML = `<th>Found</th><th>Path Cost</th><th>Hops</th><th>Path Sequence</th>`;
        if (!data.found) {
          body.innerHTML = `<tr><td colspan="4" style="color:var(--danger); text-align:center;">No path exists between node ${data.source} and ${data.target}.</td></tr>`;
        } else {
          body.innerHTML = `
            <tr>
              <td><span class="badge badge-success">PATH FOUND</span></td>
              <td style="color:var(--primary); font-weight:700;">${data.cost}</td>
              <td>${data.length}</td>
              <td><code>${escapeHtml(data.path)}</code></td>
            </tr>
          `;
        }
      } else if (type === 'community_wcc' || type === 'community_scc') {
        head.innerHTML = `<th>Cluster ID</th><th>Member Count</th><th>Node IDs</th>`;
        let clusters = data.communities || [];
        body.innerHTML = clusters.map((c, i) => `
          <tr>
            <td><strong>Cluster #${i + 1}</strong></td>
            <td><span class="badge badge-accent">${c.length} nodes</span></td>
            <td><code>${c.slice(0, 15).join(', ')}${c.length > 15 ? '...' : ''}</code></td>
          </tr>
        `).join('');
      } else if (type === 'similarity') {
        head.innerHTML = `<th>Source</th><th>Target</th><th>Metric</th><th>Score</th>`;
        body.innerHTML = `
          <tr>
            <td>${data.source}</td>
            <td>${data.target}</td>
            <td>${data.metric}</td>
            <td style="color:var(--primary); font-weight:700;">${(data.score !== undefined ? data.score.toFixed(4) : data.count)}</td>
          </tr>
        `;
      }
    }

    function applyAlgoToCanvas() {
      if (!currentAlgoResults) return;
      let { type, data } = currentAlgoResults;
      switchTab('cypher');

      if (type === 'pagerank' || type === 'betweenness' || type === 'degree') {
        let maxScore = Math.max(...(data.top_nodes || []).map(n => n.score !== undefined ? n.score : n.degree)) || 1;
        graphData.nodes.forEach(gn => {
          let match = (data.top_nodes || []).find(x => x.node_id === gn.id);
          if (match) {
            let score = match.score !== undefined ? match.score : match.degree;
            gn.radius = Math.max(16, Math.min(48, 16 + (score / maxScore) * 32));
          }
        });
      } else if (type === 'community_wcc' || type === 'community_scc') {
        const CLUSTER_COLORS = ['#38bdf8', '#818cf8', '#34d399', '#f59e0b', '#ec4899', '#06b6d4', '#a855f7'];
        (data.communities || []).forEach((c, idx) => {
          let color = CLUSTER_COLORS[idx % CLUSTER_COLORS.length];
          c.forEach(id => {
            let gn = graphData.nodes.find(n => n.id === id);
            if (gn) gn.communityColor = color;
          });
        });
      } else if (type === 'shortest_path' && data.found) {
        let edgeSet = new Set();
        let pathNodes = data.nodes || [];
        for (let i = 0; i < pathNodes.length - 1; i++) {
          edgeSet.add(`${pathNodes[i]}->${pathNodes[i+1]}`);
          edgeSet.add(`${pathNodes[i+1]}->${pathNodes[i]}`);
        }
        lastAlgoOverlay = { highlightEdges: edgeSet };
      }

      startSimulation();
    }

    // =========================================================================
    // TAB 3: Schema & Indexes Cockpit
    // =========================================================================

    async function loadSchemaData() {
      try {
        let res = await fetch('/schema');
        if (res.ok) {
          let s = await res.json();
          let lblCount = Object.keys(s.label_counts || {}).length;
          let relCount = Object.keys(s.relationship_type_counts || {}).length;
          let idxCount = (s.property_indices || []).length + (s.vector_indices || []).length + (s.fulltext_indices || []).length;

          document.getElementById('schema-lbl-count').innerText = lblCount;
          document.getElementById('schema-rel-count').innerText = relCount;
          document.getElementById('schema-idx-count').innerText = idxCount;
          document.getElementById('schema-density').innerText = (s.density || 0).toFixed(4);

          let tbody = document.getElementById('indexes-table-body');
          let rows = [];

          (s.property_indices || []).forEach(p => {
            rows.push(`
              <tr>
                <td><span class="badge badge-primary">Property (B-Tree)</span></td>
                <td><code>${escapeHtml(p.label)}_${escapeHtml(p.property)}</code></td>
                <td>:${escapeHtml(p.label)} (${escapeHtml(p.property)})</td>
                <td>Unique: ${p.unique ? 'Yes' : 'No'}</td>
                <td><button class="btn btn-danger" style="padding:0.25rem 0.5rem; font-size:0.75rem;" onclick="dropPropertyIndex('${p.label}', '${p.property}')">Drop</button></td>
              </tr>
            `);
          });

          (s.vector_indices || []).forEach(v => {
            rows.push(`
              <tr>
                <td><span class="badge badge-accent">Vector (k-NN)</span></td>
                <td><code>${escapeHtml(v.name)}</code></td>
                <td>:${escapeHtml(v.label)} (${escapeHtml(v.property_key)})</td>
                <td>Dim: ${v.dimensions}, Metric: ${escapeHtml(v.metric)} (${v.indexed_count || 0} vectors)</td>
                <td><button class="btn btn-danger" style="padding:0.25rem 0.5rem; font-size:0.75rem;" onclick="dropVectorIndex('${v.name}')">Drop</button></td>
              </tr>
            `);
          });

          (s.fulltext_indices || []).forEach(f => {
            rows.push(`
              <tr>
                <td><span class="badge badge-warning">Full-Text (BM25)</span></td>
                <td><code>${escapeHtml(f.name)}</code></td>
                <td>:${escapeHtml(f.label)} (${escapeHtml(f.property_key)})</td>
                <td>${f.indexed_documents || 0} indexed documents</td>
                <td><button class="btn btn-danger" style="padding:0.25rem 0.5rem; font-size:0.75rem;" onclick="dropFulltextIndex('${f.name}')">Drop</button></td>
              </tr>
            `);
          });

          tbody.innerHTML = rows.length > 0 ? rows.join('') : '<tr><td colspan="5" style="color:var(--text-muted); text-align:center; padding:1.5rem;">No secondary indexes created yet.</td></tr>';
        }
      } catch (e) {
        console.warn('Schema fetch failed', e);
      }
    }

    function openCreateIndexModal() {
      document.getElementById('modal-create-index').classList.add('active');
      updateCreateIndexModalFields('property');
    }
    function closeCreateIndexModal() {
      document.getElementById('modal-create-index').classList.remove('active');
    }

    function updateCreateIndexModalFields(type) {
      document.getElementById('group-idx-name').style.display = (type === 'property') ? 'none' : 'flex';
      document.getElementById('group-idx-dim').style.display = (type === 'vector') ? 'flex' : 'none';
    }

    async function submitCreateIndex() {
      let type = document.getElementById('modal-idx-type').value;
      let label = document.getElementById('modal-idx-label').value.trim();
      let prop = document.getElementById('modal-idx-prop').value.trim();
      let name = document.getElementById('modal-idx-name').value.trim();

      if (!label || !prop) { alert('Label and property are required'); return; }

      try {
        let res;
        if (type === 'property') {
          res = await fetch('/indexes/property', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ label, property: prop })
          });
        } else if (type === 'vector') {
          let dim = parseInt(document.getElementById('modal-idx-dim').value) || 128;
          res = await fetch('/vector/indexes', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, label, prop_key: prop, dimension: dim })
          });
        } else {
          res = await fetch('/fulltext/indexes', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, label, property_key: prop })
          });
        }

        let data = await res.json();
        if (res.ok) {
          closeCreateIndexModal();
          loadSchemaData();
        } else {
          alert('Failed: ' + (data.error || 'Server error'));
        }
      } catch (e) {
        alert('Error: ' + e.message);
      }
    }

    async function dropPropertyIndex(label, prop) {
      if (!confirm(`Drop index on :${label}(${prop})?`)) return;
      await fetch(`/indexes/property/${label}/${prop}`, { method: 'DELETE' });
      loadSchemaData();
    }
    async function dropVectorIndex(name) {
      if (!confirm(`Drop vector index "${name}"?`)) return;
      await fetch(`/vector/indexes/${name}`, { method: 'DELETE' });
      loadSchemaData();
    }
    async function dropFulltextIndex(name) {
      if (!confirm(`Drop fulltext index "${name}"?`)) return;
      await fetch(`/fulltext/indexes/${name}`, { method: 'DELETE' });
      loadSchemaData();
    }

    // =========================================================================
    // TAB 4: Search Studio
    // =========================================================================

    function toggleSearchMode(mode) {
      let isFt = mode === 'fulltext';
      document.getElementById('search-form-fulltext').style.display = isFt ? 'flex' : 'none';
      document.getElementById('search-form-vector').style.display = isFt ? 'none' : 'flex';
      document.getElementById('btn-mode-fulltext').className = isFt ? 'btn' : 'btn btn-secondary';
      document.getElementById('btn-mode-vector').className = isFt ? 'btn btn-secondary' : 'btn';
    }

    async function runFulltextSearch() {
      let index = document.getElementById('ft-index-name').value.trim();
      let query = document.getElementById('ft-search-query').value.trim();
      let prefix = document.getElementById('ft-prefix').checked;
      let container = document.getElementById('search-results-container');
      container.innerHTML = '<div style="color:var(--text-muted); text-align:center;">Searching...</div>';

      try {
        let res = await fetch('/fulltext/search', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ index, query, top_k: 10, prefix })
        });
        let hits = await res.json();
        if (!res.ok || hits.error) {
          container.innerHTML = `<div style="color:var(--danger);">${escapeHtml(hits.error || 'Search failed')}</div>`;
          return;
        }
        if (hits.length === 0) {
          container.innerHTML = '<div style="color:var(--text-muted); text-align:center; padding:1.5rem;">No matching documents found.</div>';
          return;
        }
        container.innerHTML = hits.map(h => `
          <div style="background:#090e1a; border:1px solid var(--card-border); border-radius:8px; padding:0.75rem 1rem; display:flex; justify-content:space-between; align-items:center;">
            <div>
              <div style="font-weight:700; color:#fff;">Node ID: ${h.node_id}</div>
              <div style="font-size:0.8rem; color:var(--text-muted); margin-top:0.2rem;">Matched Terms: <span style="color:#e2e8f0;">${escapeHtml((h.matched_terms || []).join(', '))}</span></div>
            </div>
            <div class="badge badge-primary">Score: ${h.score.toFixed(3)}</div>
          </div>
        `).join('');
      } catch (e) {
        container.innerHTML = `<div style="color:var(--danger);">${escapeHtml(e.message)}</div>`;
      }
    }

    async function runVectorSearch() {
      let index = document.getElementById('vec-index-name').value.trim();
      let vecStr = document.getElementById('vec-query-input').value.trim();
      let container = document.getElementById('search-results-container');

      let vector;
      try {
        vector = JSON.parse(vecStr);
        if (!Array.isArray(vector)) throw new Error('Must be an array of numbers');
      } catch (err) {
        alert('Invalid vector format: ' + err.message);
        return;
      }

      container.innerHTML = '<div style="color:var(--text-muted); text-align:center;">Querying nearest neighbors...</div>';
      try {
        let res = await fetch('/vector/search', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ index, vector, k: 5 })
        });
        let hits = await res.json();
        if (!res.ok || hits.error) {
          container.innerHTML = `<div style="color:var(--danger);">${escapeHtml(hits.error || 'Vector search failed')}</div>`;
          return;
        }
        if (hits.length === 0) {
          container.innerHTML = '<div style="color:var(--text-muted); text-align:center; padding:1.5rem;">No nearest neighbors found.</div>';
          return;
        }
        container.innerHTML = hits.map((h, i) => `
          <div style="background:#090e1a; border:1px solid var(--card-border); border-radius:8px; padding:0.75rem 1rem; display:flex; justify-content:space-between; align-items:center;">
            <div>
              <div style="font-weight:700; color:#fff;">#${i + 1} Nearest Neighbor: Node ID ${h.node_id}</div>
              <div style="font-size:0.8rem; color:var(--text-muted); margin-top:0.2rem;">Vector Index: <code>${escapeHtml(index)}</code></div>
            </div>
            <div class="badge badge-accent">Distance: ${h.score.toFixed(4)}</div>
          </div>
        `).join('');
      } catch (e) {
        container.innerHTML = `<div style="color:var(--danger);">${escapeHtml(e.message)}</div>`;
      }
    }

    // =========================================================================
    // TAB 5: Telemetry & Transactions
    // =========================================================================

    async function refreshTransactions() {
      try {
        let res = await fetch('/tx/list');
        if (res.ok) {
          let data = await res.json();
          let txs = data.active_transactions || [];
          let tbody = document.getElementById('tx-table-body');
          if (txs.length === 0) {
            tbody.innerHTML = '<tr><td colspan="4" style="color:var(--text-muted); text-align:center; padding:1.5rem;">No active ACID transactions running.</td></tr>';
            return;
          }
          tbody.innerHTML = txs.map(t => `
            <tr>
              <td><code>Transaction #${t.tx_id}</code></td>
              <td style="color:var(--primary); font-family:var(--font-mono);">${(t.remaining_ms / 1000).toFixed(1)}s remaining</td>
              <td>${t.staged_mutations || 0} staged queries</td>
              <td>
                <button class="btn btn-secondary" style="padding:0.2rem 0.5rem; font-size:0.75rem;" onclick="commitTx(${t.tx_id})">Commit</button>
                <button class="btn btn-danger" style="padding:0.2rem 0.5rem; font-size:0.75rem;" onclick="rollbackTx(${t.tx_id})">Rollback</button>
              </td>
            </tr>
          `).join('');
        }
      } catch (e) {
        console.warn('Tx fetch failed', e);
      }
    }

    async function commitTx(id) {
      await fetch(`/tx/${id}/commit`, { method: 'POST' });
      refreshTransactions();
      refreshStats();
    }
    async function rollbackTx(id) {
      await fetch(`/tx/${id}/rollback`, { method: 'POST' });
      refreshTransactions();
      refreshStats();
    }

    async function triggerCheckpoint() {
      try {
        let res = await fetch('/checkpoint', { method: 'POST' });
        let data = await res.json();
        alert('Checkpoint successful! Sequenced at: ' + data.checkpointed_sequence);
        refreshStats();
      } catch (e) {
        alert('Checkpoint failed: ' + e.message);
      }
    }

    async function exportGraphJson() {
      try {
        let res = await fetch('/export');
        let blob = await res.blob();
        let url = window.URL.createObjectURL(blob);
        let a = document.createElement('a');
        a.href = url;
        a.download = `rustygraph_backup_${Date.now()}.json`;
        a.click();
      } catch (e) {
        alert('Export failed: ' + e.message);
      }
    }

    function openImportModal() { document.getElementById('modal-import').classList.add('active'); }
    function closeImportModal() { document.getElementById('modal-import').classList.remove('active'); }

    async function submitImportJson() {
      let jsonStr = document.getElementById('import-json-text').value.trim();
      if (!jsonStr) return;
      try {
        let res = await fetch('/import', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: jsonStr
        });
        let data = await res.json();
        if (res.ok) {
          alert('Graph restored successfully!');
          closeImportModal();
          refreshStats();
          runCypher();
        } else {
          alert('Import error: ' + (data.error || 'Invalid JSON'));
        }
      } catch (e) {
        alert('Import error: ' + e.message);
      }
    }

    // =========================================================================
    // TAB 6: REST API Sandbox
    // =========================================================================

    const API_TEMPLATES = {
      get_stats: { method: 'GET', url: '/stats', body: '' },
      get_schema: { method: 'GET', url: '/schema', body: '' },
      get_indexes: { method: 'GET', url: '/indexes', body: '' },
      post_cypher: { method: 'POST', url: '/cypher', body: JSON.stringify({ query: 'MATCH (n) RETURN n LIMIT 10' }, null, 2) },
      post_explain: { method: 'POST', url: '/cypher/explain', body: JSON.stringify({ query: 'MATCH (p:Person)-[:KNOWS]->(b) RETURN p.name, b.name' }, null, 2) },
      post_pagerank: { method: 'POST', url: '/algo/pagerank', body: JSON.stringify({ damping: 0.85, max_iterations: 50 }, null, 2) },
      post_shortest_path: { method: 'POST', url: '/algo/shortest-path', body: JSON.stringify({ source: 1, target: 4, algorithm: 'bfs' }, null, 2) },
      post_centrality: { method: 'POST', url: '/algo/centrality', body: JSON.stringify({ metric: 'betweenness', direction: 'both' }, null, 2) },
      post_community: { method: 'POST', url: '/algo/community', body: JSON.stringify({ algorithm: 'wcc' }, null, 2) },
      post_similarity: { method: 'POST', url: '/algo/similarity', body: JSON.stringify({ source: 1, target: 2, metric: 'jaccard' }, null, 2) },
      post_nodes: { method: 'POST', url: '/nodes', body: JSON.stringify({ labels: ['Server'], properties: { name: 'prod-01', cpu: 16 } }, null, 2) },
      post_edges: { method: 'POST', url: '/edges', body: JSON.stringify({ source: 1, target: 2, rel_type: 'CONNECTED_TO', properties: { ping_ms: 4.2 } }, null, 2) },
      get_tx_list: { method: 'GET', url: '/tx/list', body: '' },
      get_metrics: { method: 'GET', url: '/metrics', body: '' }
    };

    let activeApiTemplate = API_TEMPLATES.get_stats;

    function loadApiTemplate(key) {
      activeApiTemplate = API_TEMPLATES[key] || API_TEMPLATES.get_stats;
      document.getElementById('api-request-body').value = activeApiTemplate.body;
      document.getElementById('api-response-body').value = '{}';
      document.getElementById('api-status-badge').style.display = 'none';
    }

    async function executeApiSandbox() {
      let t0 = performance.now();
      let badge = document.getElementById('api-status-badge');
      badge.style.display = 'inline-block';
      badge.innerText = 'Calling...';

      try {
        let opts = { method: activeApiTemplate.method };
        if (activeApiTemplate.method === 'POST') {
          opts.headers = { 'Content-Type': 'application/json' };
          opts.body = document.getElementById('api-request-body').value;
        }

        let res = await fetch(activeApiTemplate.url, opts);
        let t1 = performance.now();
        badge.innerText = `HTTP ${res.status} (${(t1 - t0).toFixed(1)} ms)`;
        badge.className = res.ok ? 'badge badge-success' : 'badge badge-danger';

        let text = await res.text();
        try {
          let json = JSON.parse(text);
          document.getElementById('api-response-body').value = JSON.stringify(json, null, 2);
        } catch {
          document.getElementById('api-response-body').value = text;
        }
      } catch (e) {
        badge.innerText = 'Network Error';
        badge.className = 'badge badge-danger';
        document.getElementById('api-response-body').value = e.message;
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
