(function () {
  const palette = window.PREVIEW_PALETTE || {};
  const soft = palette.soft || {};
  const root = document.documentElement;
  const set = (name, value) => { if (value) root.style.setProperty(name, value); };
  set("--bg", palette.bg);
  set("--fg", palette.fg);
  set("--accent", palette.accent);
  set("--text", soft.text);
  set("--muted", soft.muted);
  set("--faint", soft.faint);
  set("--surface", soft.surface);
  set("--elevated", soft.elevated);
  set("--line", soft.line);
  set("--accent-soft", soft.accentSoft);
  set("--fg-soft", soft.fgSoft);
  set("--scrim", soft.scrim);
  set("--shadow", soft.shadow);
  if (palette.fontFamily) set("--font", palette.fontFamily);
  if (palette.fontSizePx) set("--size", palette.fontSizePx + "px");

  const entries = [
    { id: "groceries", title: "Check on my groceries", hint: "Example answer", badge: "Example", keys: "check on my groceries grocer food" },
    { id: "example-a", title: "Call Example A", hint: "Bot · example thread", badge: "Bot", keys: "call example-a" },
    { id: "example-b", title: "Call Example B", hint: "Bot · example thread", badge: "Bot", keys: "call example-b" },
    { id: "inbox", title: "Messages", hint: "Sample contact is human", badge: "Inbox", keys: "messages inbox sample contact" },
    { id: "recent", title: "Recent work", hint: "Example notes", badge: "Example", keys: "recent work" },
    { id: "decisions", title: "Open decisions", hint: "Example notes", badge: "Example", keys: "open decisions" }
  ];

  const icons = {
    groceries: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="7"></circle><path d="M12 9v4l2 2"></path></svg>',
    example-a: '<svg viewBox="0 0 24 24"><rect x="5" y="5" width="14" height="14" rx="4"></rect></svg>',
    example-b: '<svg viewBox="0 0 24 24"><rect x="5" y="5" width="14" height="14" rx="4"></rect></svg>',
    inbox: '<svg viewBox="0 0 24 24"><path d="M4 12h5l2 3h2l2-3h5"></path><rect x="4" y="6" width="16" height="12" rx="2"></rect></svg>',
    recent: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="7"></circle><path d="M12 8v5"></path></svg>',
    decisions: '<svg viewBox="0 0 24 24"><path d="M6 7h12M6 12h8M6 17h10"></path></svg>'
  };

  const state = {
    query: "",
    cursor: 0,
    panel: null,
    thread: null,
    groceries: [
      { name: "Example oats", have: false },
      { name: "Example milk", have: true },
      { name: "Example eggs", have: false }
    ],
    deeper: false
  };

  const params = new URLSearchParams(location.search);
  if (params.get("view")) state.panel = params.get("view");

  const q = document.getElementById("q");
  const results = document.getElementById("results");
  const detail = document.getElementById("detail");
  const split = document.getElementById("split");
  const ask = document.getElementById("ask");

  function matches(entry) {
    const query = state.query.trim().toLowerCase();
    if (!query) return true;
    const blob = (entry.title + " " + entry.keys).toLowerCase();
    return query.split(/\s+/).every((term) => blob.includes(term));
  }

  function visible() {
    return entries.filter(matches);
  }

  function esc(value) {
    return String(value).replace(/[&<>"']/g, (ch) => ({
      "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;"
    }[ch]));
  }

  function renderList() {
    const rows = visible();
    if (state.cursor >= rows.length) state.cursor = Math.max(0, rows.length - 1);
    results.innerHTML = rows.map((entry, index) => (
      '<li><button type="button" class="result" role="option" data-id="' + entry.id + '" aria-selected="' + (index === state.cursor) + '">' +
      (icons[entry.id] || "") +
      '<span><span class="title">' + esc(entry.title) + '</span><span class="hint">' + esc(entry.hint) + '</span></span>' +
      '<span class="kind">' + esc(entry.badge) + '</span></button></li>'
    )).join("") || '<li class="empty">No example row matches. Nothing was searched remotely.</li>';
  }

  function banner() {
    return '<p class="banner">Stale example. Not live. Not from a store or an account.</p>';
  }

  function renderDetail() {
    if (!state.panel) {
      detail.hidden = true;
      detail.innerHTML = "";
      split.classList.remove("has-detail");
      return;
    }
    detail.hidden = false;
    split.classList.add("has-detail");
    if (state.panel === "groceries") {
      const items = state.groceries.map((item, index) => (
        '<li><input type="checkbox" data-check="' + index + '"' + (item.have ? " checked" : "") + '>' +
        '<input type="text" data-name="' + index + '" value="' + esc(item.name) + '"></li>'
      )).join("");
      detail.innerHTML =
        '<p class="kicker">Example answer</p><h2>Check on my groceries</h2>' + banner() +
        '<p class="answer">Sample list only. Edits stay in this page and are discarded on reload.</p>' +
        '<ul class="items">' + items + '</ul>' +
        '<div class="row-actions"><button type="button" class="chip" id="add-item">Add example item</button>' +
        '<button type="button" class="chip" id="reset-items">Reset sample list</button></div>' +
        '<div class="budget"><strong>Freshness unavailable</strong><span>No balance is loaded or shown.</span></div>' +
        '<button type="button" class="resource" id="deeper" aria-expanded="' + state.deeper + '">Dig deeper</button>' +
        (state.deeper ? '<p class="note">Example note: a real check would name a list and a time. This preview has neither, and it does not open the network.</p>' : '');
      return;
    }
    if (state.panel === "example-a" || state.panel === "example-b") {
      const name = state.panel === "example-a" ? "Example A" : "Example B";
      detail.innerHTML =
        '<p class="kicker">Example thread</p><h2>Call ' + name + '</h2>' +
        '<p class="identity">Bot · ' + name + '</p>' + banner() +
        '<div class="thread"><div class="bubble"><span class="who">Bot · example</span>Nothing is connected. This reply is sample copy.</div></div>' +
        '<p class="note">Launcher unavailable. No message was sent.</p>' +
        '<form class="composer" id="composer"><input id="draft" placeholder="Demo note, not sent" autocomplete="off"><button type="submit">Keep here</button></form>';
      return;
    }
    if (state.panel === "inbox") {
      if (state.thread) {
        const human = state.thread === "human";
        const title = human ? "Sample contact" : (state.thread === "example-a" ? "Example A" : "Example B");
        detail.innerHTML =
          '<button type="button" class="back" id="back-inbox">Back</button>' +
          '<h2>' + esc(title) + '</h2>' +
          '<p class="identity ' + (human ? "human" : "") + '">' + (human ? "Human" : "Bot") + '</p>' + banner() +
          '<div class="thread"><div class="bubble"><span class="who">' + (human ? "Human · example" : "Bot · example") + '</span>Sample line. Nobody was contacted.</div></div>';
        return;
      }
      detail.innerHTML =
        '<p class="kicker">Example inbox</p><h2>Messages</h2>' + banner() +
        '<button type="button" class="inbox-row" data-thread="example-a"><span>Example A</span><span class="kind">Bot</span></button>' +
        '<button type="button" class="inbox-row" data-thread="example-b"><span>Example B</span><span class="kind">Bot</span></button>' +
        '<button type="button" class="inbox-row" data-thread="human"><span>Sample contact</span><span class="kind">Human</span></button>';
      return;
    }
    if (state.panel === "recent") {
      detail.innerHTML =
        '<p class="kicker">Example notes</p><h2>Recent work</h2>' + banner() +
        '<div class="cards"><div class="card"><b>Example</b><span>A visual preview sits beside the stock menu. This is not a git status.</span></div>' +
        '<div class="card"><b>Example</b><span>Ghostty and the stock menu were reported to share a theme. This page does not read that theme live.</span></div></div>';
      return;
    }
    detail.innerHTML =
      '<p class="kicker">Example notes</p><h2>Open decisions</h2>' + banner() +
      '<div class="cards"><div class="card"><b>Example</b><span>Super+Space stays the stock menu.</span></div>' +
      '<div class="card"><b>Example</b><span>This preview does not bind Alt+Space by itself.</span></div></div>';
  }

  function render() {
    renderList();
    renderDetail();
  }

  ask.addEventListener("submit", (event) => {
    event.preventDefault();
    const rows = visible();
    if (rows[state.cursor]) state.panel = rows[state.cursor].id;
    render();
  });

  q.addEventListener("input", () => {
    state.query = q.value;
    state.cursor = 0;
    render();
  });

  results.addEventListener("click", (event) => {
    const button = event.target.closest("[data-id]");
    if (!button) return;
    state.panel = button.getAttribute("data-id");
    render();
  });

  detail.addEventListener("click", (event) => {
    const check = event.target.closest("[data-check]");
    if (check) {
      state.groceries[Number(check.getAttribute("data-check"))].have = check.checked;
      return;
    }
    if (event.target.id === "add-item") {
      state.groceries.push({ name: "Example item", have: false });
      render();
      return;
    }
    if (event.target.id === "reset-items") {
      state.groceries = [
        { name: "Example oats", have: false },
        { name: "Example milk", have: true },
        { name: "Example eggs", have: false }
      ];
      render();
      return;
    }
    if (event.target.id === "deeper") {
      state.deeper = !state.deeper;
      render();
      return;
    }
    const thread = event.target.closest("[data-thread]");
    if (thread) {
      state.thread = thread.getAttribute("data-thread");
      render();
      return;
    }
    if (event.target.id === "back-inbox") {
      state.thread = null;
      render();
    }
  });

  detail.addEventListener("input", (event) => {
    const field = event.target.closest("[data-name]");
    if (!field) return;
    state.groceries[Number(field.getAttribute("data-name"))].name = field.value;
  });

  detail.addEventListener("submit", (event) => {
    event.preventDefault();
    const draft = document.getElementById("draft");
    if (!draft || !draft.value.trim()) return;
    const note = document.createElement("p");
    note.className = "note";
    note.textContent = "Kept in this page only: " + draft.value.trim();
    draft.value = "";
    event.target.insertAdjacentElement("beforebegin", note);
  });

  document.addEventListener("keydown", (event) => {
    const rows = visible();
    if (event.key === "Escape") {
      if (state.thread) state.thread = null;
      else if (state.deeper) state.deeper = false;
      else if (state.panel) state.panel = null;
      else if (state.query) { state.query = ""; q.value = ""; }
      render();
      event.preventDefault();
      return;
    }
    if (event.target !== q) return;
    if (event.key === "ArrowDown") {
      state.cursor = Math.min(rows.length - 1, state.cursor + 1);
      render();
      event.preventDefault();
    } else if (event.key === "ArrowUp") {
      state.cursor = Math.max(0, state.cursor - 1);
      render();
      event.preventDefault();
    }
  });

  q.focus();
  render();
})();
