/**
 * RustyGraph Interactive Documentation & Cypher Playground Engine
 * Pure Vanilla JavaScript Client-Side Engine
 */

document.addEventListener('DOMContentLoaded', () => {
  initThemeToggle();
  initCopyButtons();
  initTabs();
  initSearch();
  initScrollSpy();
  initPlayground();
});

/* =============================================================================
   1. Theme Management (Dark Obsidian / Clean Light)
   ============================================================================= */
function initThemeToggle() {
  const toggleBtn = document.getElementById('themeToggle');
  if (!toggleBtn) return;

  const currentTheme = localStorage.getItem('rustygraph_theme') || 'dark';
  document.documentElement.setAttribute('data-theme', currentTheme);

  toggleBtn.addEventListener('click', () => {
    const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
    const nextTheme = isDark ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', nextTheme);
    localStorage.setItem('rustygraph_theme', nextTheme);
  });
}

/* =============================================================================
   2. One-Click Copy Snippets
   ============================================================================= */
function initCopyButtons() {
  document.querySelectorAll('.btn-copy').forEach(btn => {
    btn.addEventListener('click', async () => {
      const container = btn.closest('.code-container');
      const code = container ? container.querySelector('code')?.innerText : '';
      if (!code) return;

      try {
        await navigator.clipboard.writeText(code);
        const originalText = btn.innerHTML;
        btn.innerHTML = '<span>✓ Copied!</span>';
        setTimeout(() => {
          btn.innerHTML = originalText;
        }, 2000);
      } catch (err) {
        console.error('Failed to copy code: ', err);
      }
    });
  });
}

/* =============================================================================
   3. Quickstart Tabs
   ============================================================================= */
function initTabs() {
  document.querySelectorAll('.tab-group').forEach(group => {
    const buttons = group.querySelectorAll('.tab-btn');
    const container = group.parentElement;
    const contents = container.querySelectorAll('.tab-content');

    buttons.forEach((btn, index) => {
      btn.addEventListener('click', () => {
        buttons.forEach(b => b.classList.remove('active'));
        contents.forEach(c => c.classList.remove('active'));

        btn.classList.add('active');
        if (contents[index]) {
          contents[index].classList.add('active');
        }
      });
    });
  });
}

/* =============================================================================
   4. Client-side Search & Filtering
   ============================================================================= */
function initSearch() {
  const searchInput = document.getElementById('docSearchInput');
  if (!searchInput) return;

  window.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      searchInput.focus();
    }
  });

  searchInput.addEventListener('input', () => {
    const term = searchInput.value.toLowerCase().trim();
    const navItems = document.querySelectorAll('.sidebar-nav a');

    navItems.forEach(item => {
      const text = item.textContent.toLowerCase();
      if (!term || text.includes(term)) {
        item.parentElement.style.display = 'block';
      } else {
        item.parentElement.style.display = 'none';
      }
    });
  });
}

/* =============================================================================
   5. ScrollSpy
   ============================================================================= */
function initScrollSpy() {
  const sections = document.querySelectorAll('.doc-section, .hero');
  const navLinks = document.querySelectorAll('.sidebar-nav a');
  if (!sections.length || !navLinks.length) return;

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          const id = entry.target.getAttribute('id');
          navLinks.forEach(link => {
            if (link.getAttribute('href') === `#${id}`) {
              link.classList.add('active');
            } else {
              link.classList.remove('active');
            }
          });
        }
      });
    },
    { threshold: 0.25, rootMargin: '-70px 0px -40% 0px' }
  );

  sections.forEach(sec => observer.observe(sec));
}

/* =============================================================================
   6. Interactive Cypher Playground & Force-Directed Graph Engine
   ============================================================================= */
function initPlayground() {
  const canvas = document.getElementById('graphCanvas');
  const queryInput = document.getElementById('playgroundQuery');
  const runBtn = document.getElementById('runQueryBtn');
  const outputBox = document.getElementById('playgroundOutput');
  const txIndicator = document.getElementById('txIndicator');

  if (!canvas || !queryInput || !runBtn) return;

  const ctx = canvas.getContext('2d');
  let animationFrameId = null;

  // In-Memory Graph State
  let graphState = {
    nodes: [
      { id: 1, label: 'Person', name: 'Alice', age: 30, x: 120, y: 140, vx: 0, vy: 0 },
      { id: 2, label: 'Person', name: 'Bob', age: 28, x: 260, y: 100, vx: 0, vy: 0 },
      { id: 3, label: 'Person', name: 'Charlie', age: 24, x: 280, y: 220, vx: 0, vy: 0 },
      { id: 4, label: 'Server', name: 'Edge-US-East', ip: '192.168.1.1', x: 420, y: 150, vx: 0, vy: 0 },
      { id: 5, label: 'Paper', name: 'Attention All You Need', citations: 120000, x: 150, y: 250, vx: 0, vy: 0 },
    ],
    edges: [
      { id: 101, source: 1, target: 2, type: 'KNOWS', since: 2021 },
      { id: 102, source: 2, target: 3, type: 'KNOWS', since: 2023 },
      { id: 103, source: 1, target: 4, type: 'ADMINISTERS', role: 'root' },
      { id: 104, source: 3, target: 5, type: 'AUTHORED', year: 2024 },
    ]
  };

  // Transaction state: null = not in tx, or snapshot object
  let activeTx = null;

  // Resize canvas to display resolution
  function resizeCanvas() {
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * window.devicePixelRatio;
    canvas.height = rect.height * window.devicePixelRatio;
    ctx.scale(window.devicePixelRatio, window.devicePixelRatio);
  }
  window.addEventListener('resize', resizeCanvas);
  resizeCanvas();

  // Color scheme per node label
  const labelColors = {
    'Person': '#00f0ff',
    'Server': '#f97316',
    'Product': '#10b981',
    'Paper': '#a855f7',
    'User': '#38bdf8',
    'Default': '#94a3b8'
  };

  // Dragging support
  let draggedNode = null;
  canvas.addEventListener('mousedown', (e) => {
    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const displayNodes = activeTx ? activeTx.nodes : graphState.nodes;
    for (let n of displayNodes) {
      const dist = Math.hypot(n.x - mouseX, n.y - mouseY);
      if (dist < 22) {
        draggedNode = n;
        break;
      }
    }
  });

  window.addEventListener('mousemove', (e) => {
    if (draggedNode) {
      const rect = canvas.getBoundingClientRect();
      draggedNode.x = Math.max(25, Math.min(rect.width - 25, e.clientX - rect.left));
      draggedNode.y = Math.max(25, Math.min(rect.height - 25, e.clientY - rect.top));
    }
  });

  window.addEventListener('mouseup', () => {
    draggedNode = null;
  });

  // Simple Physics simulation step
  function simulatePhysics(nodes, edges, width, height) {
    // Repulsion between nodes
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        let dx = nodes[j].x - nodes[i].x;
        let dy = nodes[j].y - nodes[i].y;
        let dist = Math.hypot(dx, dy) || 1;
        if (dist < 180) {
          let force = (180 - dist) / 180 * 1.5;
          let fx = (dx / dist) * force;
          let fy = (dy / dist) * force;
          if (nodes[i] !== draggedNode) { nodes[i].x -= fx; nodes[i].y -= fy; }
          if (nodes[j] !== draggedNode) { nodes[j].x += fx; nodes[j].y += fy; }
        }
      }
    }

    // Spring attraction along edges
    const nodeMap = new Map(nodes.map(n => [n.id, n]));
    for (let edge of edges) {
      let src = nodeMap.get(edge.source);
      let dst = nodeMap.get(edge.target);
      if (src && dst) {
        let dx = dst.x - src.x;
        let dy = dst.y - src.y;
        let dist = Math.hypot(dx, dy) || 1;
        let ideal = 100;
        let force = (dist - ideal) * 0.02;
        let fx = (dx / dist) * force;
        let fy = (dy / dist) * force;
        if (src !== draggedNode) { src.x += fx; src.y += fy; }
        if (dst !== draggedNode) { dst.x -= fx; dst.y -= fy; }
      }
    }

    // Keep within bounds
    for (let n of nodes) {
      n.x = Math.max(30, Math.min(width - 30, n.x));
      n.y = Math.max(30, Math.min(height - 30, n.y));
    }
  }

  // Render loop
  function drawGraph() {
    const rect = canvas.getBoundingClientRect();
    const w = rect.width;
    const h = rect.height;

    ctx.clearRect(0, 0, w, h);

    const activeGraph = activeTx ? activeTx : graphState;
    simulatePhysics(activeGraph.nodes, activeGraph.edges, w, h);

    const nodeMap = new Map(activeGraph.nodes.map(n => [n.id, n]));

    // Draw Edges
    for (let edge of activeGraph.edges) {
      const src = nodeMap.get(edge.source);
      const dst = nodeMap.get(edge.target);
      if (!src || !dst) continue;

      ctx.beginPath();
      ctx.moveTo(src.x, src.y);
      ctx.lineTo(dst.x, dst.y);
      ctx.strokeStyle = activeTx ? 'rgba(249, 115, 22, 0.45)' : 'rgba(0, 240, 255, 0.35)';
      ctx.lineWidth = 1.8;
      ctx.stroke();

      // Draw Edge Label in center
      const midX = (src.x + dst.x) / 2;
      const midY = (src.y + dst.y) / 2;
      ctx.fillStyle = '#64748b';
      ctx.font = '10px var(--font-mono)';
      ctx.textAlign = 'center';
      ctx.fillText(`:${edge.type}`, midX, midY - 4);
    }

    // Draw Nodes
    for (let n of activeGraph.nodes) {
      const color = labelColors[n.label] || labelColors['Default'];

      // Outer glow
      ctx.beginPath();
      ctx.arc(n.x, n.y, 18, 0, Math.PI * 2);
      ctx.fillStyle = color + '22';
      ctx.fill();

      // Core Circle
      ctx.beginPath();
      ctx.arc(n.x, n.y, 14, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.fill();
      ctx.strokeStyle = '#fff';
      ctx.lineWidth = 1.5;
      ctx.stroke();

      // Node Name Label
      ctx.fillStyle = '#ffffff';
      ctx.font = 'bold 11px var(--font-sans)';
      ctx.textAlign = 'center';
      ctx.fillText(n.name || `#${n.id}`, n.x, n.y + 28);

      // Node Category Badge
      ctx.fillStyle = color;
      ctx.font = '9px var(--font-mono)';
      ctx.fillText(`:${n.label}`, n.x, n.y + 40);
    }

    animationFrameId = requestAnimationFrame(drawGraph);
  }
  drawGraph();

  // Preset query handler
  document.querySelectorAll('.preset-chip').forEach(chip => {
    chip.addEventListener('click', () => {
      document.querySelectorAll('.preset-chip').forEach(c => c.classList.remove('active'));
      chip.classList.add('active');
      const q = chip.getAttribute('data-query');
      if (q) {
        queryInput.value = q;
        runQuery(q);
      }
    });
  });

  // Query Execution Simulator
  function runQuery(rawQuery) {
    const start = performance.now();
    const q = rawQuery.trim();

    // Check meta-commands
    if (q.startsWith(':')) {
      handleMetaCommand(q, start);
      return;
    }

    const currentGraph = activeTx ? activeTx : graphState;

    try {
      // 1. MATCH queries
      if (q.toUpperCase().startsWith('MATCH')) {
        let result = evaluateMatchQuery(q, currentGraph);
        const duration = ((performance.now() - start) * 1000).toFixed(1);
        renderTableOutput(result.columns, result.rows, duration);
        return;
      }

      // 2. CREATE queries
      if (q.toUpperCase().startsWith('CREATE')) {
        let result = evaluateCreateQuery(q, currentGraph);
        const duration = ((performance.now() - start) * 1000).toFixed(1);
        renderTableOutput(['result'], [[result]], duration);
        return;
      }

      // 3. Unknown
      renderErrorOutput(`Syntax Error: Unrecognized openCypher clause in query "${q}". Expected MATCH, CREATE, or :command`);
    } catch (e) {
      renderErrorOutput(e.message);
    }
  }

  function handleMetaCommand(cmd, start) {
    const elapsed = ((performance.now() - start) * 1000).toFixed(1);

    if (cmd.startsWith(':begin')) {
      if (activeTx) {
        renderErrorOutput(`Error: Transaction ${activeTx.id} is already open. Use :commit or :rollback.`);
        return;
      }
      activeTx = {
        id: Math.floor(Math.random() * 900) + 100,
        nodes: JSON.parse(JSON.stringify(graphState.nodes)),
        edges: JSON.parse(JSON.stringify(graphState.edges)),
        mutations: []
      };
      updateTxUi();
      renderTableOutput(['transaction_id', 'status', 'isolation'], [[activeTx.id, 'ACTIVE', 'SNAPSHOT_ISOLATION']], elapsed);
      return;
    }

    if (cmd === ':commit') {
      if (!activeTx) {
        renderErrorOutput('Error: No active transaction to commit. Use :begin first.');
        return;
      }
      const id = activeTx.id;
      graphState.nodes = activeTx.nodes;
      graphState.edges = activeTx.edges;
      activeTx = null;
      updateTxUi();
      renderTableOutput(['result', 'wal_persisted'], [[`Transaction ${id} committed successfully to WAL and live database.`, true]], elapsed);
      return;
    }

    if (cmd === ':rollback') {
      if (!activeTx) {
        renderErrorOutput('Error: No active transaction to rollback.');
        return;
      }
      const id = activeTx.id;
      activeTx = null;
      updateTxUi();
      renderTableOutput(['result', 'staged_changes_discarded'], [[`Transaction ${id} rolled back. Staged mutations discarded.`, true]], elapsed);
      return;
    }

    if (cmd === ':stats') {
      renderTableOutput(['node_count', 'edge_count', 'wal_seq', 'uptime_sec'], [
        [graphState.nodes.length, graphState.edges.length, 12, 120]
      ], elapsed);
      return;
    }

    renderErrorOutput(`Unknown REPL meta-command: ${cmd}. Available: :begin, :commit, :rollback, :stats`);
  }

  function updateTxUi() {
    if (activeTx) {
      txIndicator.textContent = `⚡ In Transaction (tx:${activeTx.id})`;
      txIndicator.classList.add('in-tx');
    } else {
      txIndicator.textContent = `Live Database`;
      txIndicator.classList.remove('in-tx');
    }
  }

  function evaluateMatchQuery(q, graph) {
    // Check if aggregation query
    if (q.toUpperCase().includes('COUNT') || q.toUpperCase().includes('AVG') || q.toUpperCase().includes('SUM')) {
      const persons = graph.nodes.filter(n => n.label === 'Person' || n.age);
      const totalCount = persons.length;
      const sumAge = persons.reduce((acc, p) => acc + (p.age || 0), 0);
      const avgAge = totalCount ? (sumAge / totalCount).toFixed(1) : 0;
      const minAge = Math.min(...persons.map(p => p.age || 99));
      const maxAge = Math.max(...persons.map(p => p.age || 0));

      return {
        columns: ['count(*)', 'avg(p.age)', 'min(p.age)', 'max(p.age)'],
        rows: [[totalCount, Number(avgAge), minAge, maxAge]]
      };
    }

    // Default Person match
    const rows = [];
    const personMap = new Map(graph.nodes.filter(n => n.label === 'Person').map(n => [n.id, n]));

    for (let edge of graph.edges) {
      if (edge.type === 'KNOWS') {
        const src = personMap.get(edge.source);
        const dst = personMap.get(edge.target);
        if (src && dst) {
          rows.push([src.name, dst.name, edge.since]);
        }
      }
    }

    return {
      columns: ['p.name', 'friend.name', 'since'],
      rows: rows.length ? rows : [['(No match)', '(No match)', 0]]
    };
  }

  function evaluateCreateQuery(q, graph) {
    const id = graph.nodes.length + 1;
    const nameMatch = q.match(/name:\s*'([^']+)'/i);
    const labelMatch = q.match(/\(([a-zA-Z0-9_]+):([a-zA-Z0-9_]+)/i);

    const label = labelMatch ? labelMatch[2] : 'Node';
    const name = nameMatch ? nameMatch[1] : `Node-${id}`;

    graph.nodes.push({
      id,
      label,
      name,
      x: 200 + Math.random() * 100,
      y: 150 + Math.random() * 100,
      vx: 0,
      vy: 0
    });

    return `Created 1 node (: ${label} {name: '${name}'})`;
  }

  function renderTableOutput(columns, rows, duration) {
    let out = `┌${columns.map(c => '─'.repeat(Math.max(c.length, 12))).join('┬')}┐\n`;
    out += `│${columns.map(c => ` ${c.padEnd(Math.max(c.length, 11))} `).join('│')}│\n`;
    out += `├${columns.map(c => '─'.repeat(Math.max(c.length, 12))).join('┼')}┤\n`;

    for (let r of rows) {
      out += `│${r.map((val, i) => {
        let s = String(val);
        return ` ${s.padEnd(Math.max(columns[i].length, 11))} `;
      }).join('│')}│\n`;
    }

    out += `└${columns.map(c => '─'.repeat(Math.max(c.length, 12))).join('┴')}┘\n`;
    out += `(${rows.length} ${rows.length === 1 ? 'row' : 'rows'} in ${duration} µs)\n`;

    outputBox.textContent = out;
  }

  function renderErrorOutput(msg) {
    outputBox.textContent = `❌ ${msg}`;
  }

  runBtn.addEventListener('click', () => {
    runQuery(queryInput.value);
  });

  // Direct transaction buttons
  const txBeginBtn = document.getElementById('txBeginBtn');
  const txCommitBtn = document.getElementById('txCommitBtn');
  const txRollbackBtn = document.getElementById('txRollbackBtn');

  if (txBeginBtn) txBeginBtn.addEventListener('click', () => runQuery(':begin'));
  if (txCommitBtn) txCommitBtn.addEventListener('click', () => runQuery(':commit'));
  if (txRollbackBtn) txRollbackBtn.addEventListener('click', () => runQuery(':rollback'));
}
