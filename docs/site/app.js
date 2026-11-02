// Storefront page script. ES module, no dependencies.
// The DOM is built with createElement/textContent/setAttribute/append only.

/**
 * @typedef {{label: string, command: string}} Install
 * @typedef {{residentBytes?: number, setupBytes?: number, deltaBytes?: number, headroomBytes?: number}} Budget
 * @typedef {{resident: number, invocation: number, setup: number|null, budget: Budget}} Footprint
 * @typedef {{dirKey: string, name: string, version: string, description: string, skillsOnly: boolean,
 *   readme: string, install: Install[], footprint: Footprint|null}} Plugin
 * @typedef {{key: string, plugins: Object<string, Plugin>}} Family
 * @typedef {{agents: {id: string, label: string}[], families: Family[]}} Catalog
 */

/** @type {{id: string, label: string}[]} */
let agents = [];
let cardCounter = 0;

/** @param {string} tag @param {string} [cls] @param {string} [text] @returns {HTMLElement} */
function el(tag, cls, text) {
  const node = document.createElement(tag);
  if (cls) node.setAttribute('class', cls);
  if (text !== undefined) node.textContent = text;
  return node;
}

/** @param {number} n @returns {string} */
function formatBytes(n) {
  return String(Math.round(n)).replace(/\B(?=(\d{3})+(?!\d))/g, ',') + ' B';
}

/** @returns {Promise<Catalog>} */
async function loadCatalog() {
  let res;
  try {
    res = await fetch('./catalog.json');
  } catch (e) {
    throw new Error('Could not load the plugin catalog (network error).');
  }
  if (!res.ok) {
    throw new Error('Could not load the plugin catalog (HTTP ' + res.status + ').');
  }
  try {
    return await res.json();
  } catch (e) {
    throw new Error('The plugin catalog is not valid JSON.');
  }
}

/**
 * Copy text and announce the result in the live region. Never throws and
 * always resolves.
 * @param {string} text
 * @returns {Promise<boolean>} true when a copy succeeded, false otherwise
 */
async function copyText(text) {
  let done = false;
  if (typeof navigator !== 'undefined' && navigator.clipboard && navigator.clipboard.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      done = true;
    } catch (e) {
      done = false;
    }
  }
  if (!done) {
    const prev = document.activeElement;
    try {
      const ta = document.createElement('textarea');
      ta.value = text;
      ta.setAttribute('readonly', '');
      ta.setAttribute('style', 'position:fixed;top:0;left:0;opacity:0');
      document.body.append(ta);
      try {
        ta.select();
        done = document.execCommand('copy') === true;
      } catch (e) {
        done = false;
      }
      ta.remove();
    } catch (e) {
      done = false;
    }
    try {
      if (prev && typeof prev.focus === 'function') prev.focus();
    } catch (e) {
      /* focus could not be restored */
    }
  }
  const live = document.getElementById('live');
  if (live) {
    // Clear first and set later, so a repeated identical message is announced again.
    live.textContent = '';
    setTimeout(() => {
      live.textContent = done ? 'Copied' : 'Copy failed. Select the text and copy it manually.';
    }, 50);
  }
  return done;
}

/**
 * Make a button copy text and show the outcome on itself for about two seconds.
 * @param {HTMLElement} btn @param {string} text @returns {void}
 */
function wireCopy(btn, text) {
  let timer = null;
  btn.addEventListener('click', () => {
    copyText(text).then((ok) => {
      btn.textContent = ok ? 'Copied' : 'Copy failed';
      if (timer !== null) clearTimeout(timer);
      timer = setTimeout(() => {
        btn.textContent = 'Copy';
        timer = null;
      }, 2000);
    });
  });
}

/** @param {string} text @param {string} what @returns {HTMLElement} */
function copyButton(text, what) {
  const b = el('button', 'btn copy', 'Copy');
  b.setAttribute('type', 'button');
  b.setAttribute('aria-label', 'Copy ' + what);
  wireCopy(b, text);
  return b;
}

/**
 * @param {string} cls @param {string} label @param {number} value
 * @param {number|undefined} budget @param {string} budgetName @returns {HTMLElement}
 */
function figure(cls, label, value, budget, budgetName) {
  const fig = el('div', 'fig ' + cls);
  const head = el('div', 'fig-head');
  head.append(el('span', 'fig-label', label), el('span', 'fig-value', formatBytes(value)));
  fig.append(head);
  if (typeof budget === 'number' && budget > 0) {
    const pct = Math.min(100, (value / budget) * 100);
    const bar = el('div', 'bar');
    bar.setAttribute('role', 'img');
    bar.setAttribute('aria-label', label + ' ' + formatBytes(value) + ' of ' + formatBytes(budget) + ' ' + budgetName);
    const fill = el('div', 'bar-fill');
    fill.style.width = pct + '%';
    bar.append(fill);
    fig.append(bar);
  }
  return fig;
}

/** @param {Footprint|null} fp @returns {HTMLElement} */
function renderFootprint(fp) {
  const box = el('div', 'footprint');
  if (!fp) {
    box.append(el('span', 'badge', 'Not measured'));
  } else {
    const budget = fp.budget || {};
    box.append(figure('fig-resident', 'Resident', fp.resident, budget.residentBytes, 'budget'));
    box.append(figure('fig-invocation', 'Invocation', fp.invocation, undefined, ''));
    if (fp.setup !== null && fp.setup !== undefined) {
      box.append(figure('fig-setup', 'Setup', fp.setup, budget.setupBytes, 'budget'));
    }
  }
  box.append(el('p', 'note', 'Bytes the host loads. Compare between plugins; this is not a context budget.'));
  return box;
}

/** @param {Plugin} plugin @returns {HTMLElement} */
function renderPanelBody(plugin) {
  const body = el('div', 'panel-body');
  body.append(el('p', 'desc', plugin.description || ''));
  const meta = el('div', 'meta');
  meta.append(el('span', 'version', 'v' + plugin.version));
  if (plugin.readme) {
    const a = el('a', '', 'README');
    a.setAttribute('aria-label', 'README for ' + plugin.name);
    a.setAttribute('href', plugin.readme);
    meta.append(a);
  }
  body.append(meta);
  for (const step of plugin.install || []) {
    const wrap = el('div', 'install');
    wrap.append(el('div', 'install-title', step.label));
    const row = el('div', 'cmd-row');
    const pre = el('pre', 'cmd');
    pre.append(el('code', '', step.command));
    row.append(pre, copyButton(step.command, step.label + ' command'));
    wrap.append(row);
    body.append(wrap);
  }
  body.append(renderFootprint(plugin.footprint));
  return body;
}

/** @param {Family} family @param {string} agentId @returns {HTMLElement} */
function renderCard(family, agentId) {
  const uid = 'c' + (cardCounter++);
  const card = el('article', 'card');
  card.append(el('h3', '', family.key));
  const strip = el('div', 'tabs');
  strip.setAttribute('role', 'tablist');
  strip.setAttribute('aria-label', family.key + ' agents');
  const panel = el('div', 'panel');
  panel.setAttribute('role', 'tabpanel');
  panel.setAttribute('id', uid + '-panel');

  const plugins = family.plugins || {};
  const tabs = [];
  let current = plugins[agentId] ? agentId : null;
  if (current === null) {
    for (const a of agents) {
      if (plugins[a.id]) { current = a.id; break; }
    }
  }

  function select(id, focus) {
    current = id;
    for (const t of tabs) {
      const on = t.id === id;
      t.node.setAttribute('aria-selected', on ? 'true' : 'false');
      t.node.setAttribute('tabindex', on ? '0' : '-1');
    }
    const tab = tabs.find((t) => t.id === id);
    if (tab) {
      panel.setAttribute('aria-labelledby', uid + '-tab-' + id);
      if (focus && tab.node.focus) tab.node.focus();
    }
    panel.replaceChildren(renderPanelBody(plugins[id]));
  }

  for (const a of agents) {
    const enabled = Boolean(plugins[a.id]);
    const node = el('button', 'tab', a.label);
    node.setAttribute('type', 'button');
    node.setAttribute('role', 'tab');
    node.setAttribute('id', uid + '-tab-' + a.id);
    node.setAttribute('aria-controls', uid + '-panel');
    node.setAttribute('aria-selected', 'false');
    node.setAttribute('tabindex', '-1');
    if (!enabled) node.setAttribute('aria-disabled', 'true');
    tabs.push({ id: a.id, node, enabled });
    strip.append(node);
    node.addEventListener('click', () => { if (enabled) select(a.id, false); });
    node.addEventListener('keydown', (ev) => {
      const avail = tabs.filter((t) => t.enabled);
      if (avail.length === 0) return;
      const i = avail.findIndex((t) => t.id === current);
      let next = null;
      if (ev.key === 'ArrowRight' || ev.key === 'ArrowDown') next = avail[(i + 1) % avail.length];
      else if (ev.key === 'ArrowLeft' || ev.key === 'ArrowUp') next = avail[(i - 1 + avail.length) % avail.length];
      else if (ev.key === 'Home') next = avail[0];
      else if (ev.key === 'End') next = avail[avail.length - 1];
      if (next) {
        if (ev.preventDefault) ev.preventDefault();
        select(next.id, true);
      }
    });
  }
  card.append(strip, panel);
  if (current !== null) {
    select(current, false);
  } else {
    panel.append(el('p', 'empty', 'No plugin for any agent.'));
  }
  return card;
}

/** @param {Catalog} catalog @param {string} filter */
function renderCatalog(catalog, filter) {
  agents = catalog.agents || [];
  const container = document.getElementById('catalog');
  if (!container) return;
  const cards = [];
  for (const family of catalog.families || []) {
    const plugins = family.plugins || {};
    let agentId = null;
    if (filter === 'all') {
      const first = agents.find((a) => plugins[a.id]);
      agentId = first ? first.id : null;
    } else if (plugins[filter]) {
      agentId = filter;
    }
    if (agentId !== null) cards.push(renderCard(family, agentId));
  }
  container.replaceChildren(...cards);
  if (cards.length === 0) container.append(el('p', 'empty', 'No plugins match this filter.'));
}

/** @returns {void} */
function initTheme() {
  const root = document.documentElement;
  try {
    const saved = localStorage.getItem('theme');
    if (saved === 'dark' || saved === 'light') root.setAttribute('data-theme', saved);
  } catch (e) {
    /* storage unavailable: follow the system theme */
  }
  const btn = document.getElementById('theme-toggle');
  if (!btn) return;
  btn.addEventListener('click', () => {
    let dark;
    const set = root.getAttribute('data-theme');
    if (set === 'dark' || set === 'light') {
      dark = set === 'dark';
    } else {
      dark = typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: dark)').matches;
    }
    const next = dark ? 'light' : 'dark';
    root.setAttribute('data-theme', next);
    try {
      localStorage.setItem('theme', next);
    } catch (e) {
      /* the choice just is not remembered */
    }
  });
}

function main() {
  initTheme();
  const heroCopy = document.getElementById('hero-copy');
  if (heroCopy) {
    wireCopy(heroCopy, 'claude plugin marketplace add ckir/aiplugins');
  }
  const container = document.getElementById('catalog');
  loadCatalog().then((catalog) => {
    let filter = 'all';
    const chips = Array.from(document.querySelectorAll('.chip'));
    for (const chip of chips) {
      chip.addEventListener('click', () => {
        filter = chip.getAttribute('data-filter') || 'all';
        for (const c of chips) c.setAttribute('aria-pressed', c === chip ? 'true' : 'false');
        renderCatalog(catalog, filter);
      });
    }
    renderCatalog(catalog, filter);
  }).catch((err) => {
    if (container) {
      container.replaceChildren(el('p', 'error', err && err.message ? err.message : String(err)));
    }
  });
}

if (typeof document !== 'undefined') main();
