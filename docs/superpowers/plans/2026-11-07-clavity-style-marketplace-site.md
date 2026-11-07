# Clavity-Style Marketplace Site Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the `aiplugins` storefront in `docs/site/` into a high-polish, dark cyber aesthetic marketplace matching `https://ckir.github.io/clavity/`, complete with a dual-agent terminal demo, 6-stage workflow chips, interactive catalog with live search, footprint meters, and full compliance with repository gates.

**Architecture:** A static, dependency-free web application deployed to GitHub Pages via `scripts/stage-site.sh`. Semantic HTML5 with an accessible `<noscript>` fallback, vanilla CSS custom properties supporting dark/light themes, and safe ES module JavaScript that builds UI components strictly with safe DOM methods.

**Tech Stack:** HTML5, CSS3, JavaScript (ES modules, DOM APIs), SVG, Bash (verification scripts).

**Spec:** `docs/superpowers/specs/2026-11-07-clavity-style-marketplace-site-design.md`

## Global Constraints

- No root-absolute URLs in any staged file (no `href="/"`, `src="/"`, `url("/"`, `fetch("/"`).
- `index.html` must contain the exact `<!-- NOSCRIPT -->` marker and the footer link `href="api/index.html"`.
- `app.js` must never use `innerHTML`, `outerHTML`, `insertAdjacentHTML`, or `document.write`.
- All checks in `just site-check` and `bash scripts/stage-site.sh` must pass with exit 0.

---

### Task 1: Brand Asset & Stylesheet Transformation

**Files:**
- Create: `docs/site/aiplugins.svg`
- Modify: `docs/site/styles.css`

**Interfaces:**
- Produces:
  - `docs/site/aiplugins.svg`: Vector icon showing 4 interconnected agent nodes.
  - `docs/site/styles.css`: Complete CSS styling covering navigation, hero, terminal demo, stage chips, value cards, footprint meters, dark/light theme variables, and copy status.

- [x] **Step 1: Create the SVG brand emblem**

Create `docs/site/aiplugins.svg` with a 4-node agent constellation and cyber plug icon.

```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120" width="120" height="120" fill="none">
  <defs>
    <linearGradient id="cyber-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#00cfff" />
      <stop offset="50%" stop-color="#3b82f6" />
      <stop offset="100%" stop-color="#ff4b18" />
    </linearGradient>
    <filter id="neon-glow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="3" result="blur" />
      <feMerge>
        <feMergeNode in="blur" />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    </filter>
  </defs>
  <circle cx="60" cy="60" r="54" stroke="url(#cyber-grad)" stroke-width="2.5" fill="#020713" opacity="0.9" />
  <circle cx="60" cy="60" r="44" stroke="#052b70" stroke-width="1.5" stroke-dasharray="4 4" />
  <!-- Interconnecting Circuit Lines -->
  <path d="M 60 22 L 60 48 M 60 72 L 60 98 M 22 60 L 48 60 M 72 60 L 98 60" stroke="#00cfff" stroke-width="2" stroke-linecap="round" opacity="0.8" />
  <path d="M 32 32 L 48 48 M 88 88 L 72 72 M 88 32 L 72 48 M 32 88 L 48 72" stroke="#ff4b18" stroke-width="1.5" stroke-linecap="round" opacity="0.7" />
  <!-- Central Hub / Plug Motif -->
  <rect x="48" y="48" width="24" height="24" rx="6" fill="#001333" stroke="url(#cyber-grad)" stroke-width="2" filter="url(#neon-glow)" />
  <circle cx="60" cy="60" r="4" fill="#00cfff" />
  <!-- Agent Nodes: Claude, Antigravity, OpenCode, Qwen -->
  <circle cx="60" cy="22" r="5" fill="#ff4b18" filter="url(#neon-glow)" />
  <circle cx="98" cy="60" r="5" fill="#00cfff" filter="url(#neon-glow)" />
  <circle cx="60" cy="98" r="5" fill="#22c55e" filter="url(#neon-glow)" />
  <circle cx="22" cy="60" r="5" fill="#a855f7" filter="url(#neon-glow)" />
</svg>
```

- [x] **Step 2: Update styles.css with Clavity-inspired design system**

Implement `docs/site/styles.css` with:
- Variables: `--bg: #020713`, `--bg-deep: #001333`, `--bg-mid: #052b70`, `--driver: #ff4b18`, `--peer: #00cfff`, `--opencode: #22c55e`, `--qwen: #a855f7`, `--text: #eaf2ff`, `--muted: #9db3d6`, `--panel: rgba(5, 43, 112, 0.35)`.
- Global responsive rules, sticky navbar, hero section, stage flowchips, dual-pane terminal mockup, value cards, footprint gauges, and light theme overrides.
- Zero root-absolute references (`url("/"`).

- [x] **Step 3: Verify CSS syntax and absence of root-absolute references**

Run: `grep -E 'url\(["'\''"]?/' docs/site/styles.css || true`
Expected: Empty output (no matches).

- [x] **Step 4: Commit Task 1 changes**

```bash
git add docs/site/aiplugins.svg docs/site/styles.css
git commit -m "feat(site): add svg brand identity and clavity cyber theme styles"
```

---

### Task 2: Semantic HTML5 Structure & Page Components

**Files:**
- Modify: `docs/site/index.html`

**Interfaces:**
- Consumes: `docs/site/styles.css`, `docs/site/aiplugins.svg`.
- Produces: Semantic HTML5 markup containing all page sections, keeping `<!-- NOSCRIPT -->` and `href="api/index.html"`.

- [x] **Step 1: Update docs/site/index.html**

Incorporate:
1. Meta tags: dark color scheme, responsive viewport, description, favicon link to `./aiplugins.svg`.
2. Sticky navigation bar (`.site-nav`) with brand link, links to `#demo`, `#values`, `#how-it-works`, `#catalog`, `#install`, `#docs`, theme switcher, and CTA button.
3. Hero with glowing gradient, headline (*"One Marketplace. Four Autonomous Agents."*), subline, 6-stage workflow chips with arrow links, and CTA buttons.
4. Dual-pane terminal mockup (`#demo`) with window control dots and title bar.
5. Value proposition grid (`#values`) with 3 cards.
6. How it works walkthrough (`#how-it-works`).
7. Interactive Catalog container (`#catalog`) with search input, agent filter chips, and the required `<!-- NOSCRIPT -->` line.
8. Context footprint explainer (`#footprints`).
9. Quick install commands with copy buttons (`#install`).
10. Guides & Resources links (`#docs`) including `USER_GUIDE.pdf` and `api/index.html`.
11. Footer with emblem, project links, and trademark disclaimers.
12. ARIA live region (`#live`) for copy status announcements.

- [x] **Step 2: Verify invariants in index.html**

Run:
```bash
grep -qxF '<!-- NOSCRIPT -->' docs/site/index.html && echo "NOSCRIPT ok"
grep -qF 'href="api/index.html"' docs/site/index.html && echo "API link ok"
grep -E '(href|src)=["'\''"]/' docs/site/index.html || echo "No root-absolute links ok"
```
Expected: All three print `ok`.

- [x] **Step 3: Commit Task 2 changes**

```bash
git add docs/site/index.html
git commit -m "feat(site): update index.html with clavity layout, hero, terminal demo, and catalog structure"
```

---

### Task 3: Interactive Application Script & Safe DOM Manipulation

**Files:**
- Modify: `docs/site/app.js`

**Interfaces:**
- Consumes: `catalog.json` (via `./catalog.json`).
- Produces: Safe DOM generation for search filtering, agent tabs, footprint bars, copy actions, and theme toggling.

- [x] **Step 1: Update docs/site/app.js**

Implement:
1. `el(tag, cls, text)` helper function.
2. `loadCatalog()` fetching `./catalog.json`.
3. Client-side search and agent filter combining text input (`#search-input`) and agent chips (`.chip`).
4. Accessible card rendering with keyboard-navigable tabs (`[role="tab"]`).
5. Visual footprint meters with budget comparison bars.
6. Accessible clipboard copy mechanism using `navigator.clipboard` with fallback, updating button text and announcing in `#live`.
7. Dark and Light theme toggle persisting to `localStorage`.
8. Complete prohibition of `innerHTML`, `outerHTML`, `insertAdjacentHTML`, and `document.write`.

- [x] **Step 2: Verify DOM safety invariants**

Run:
```bash
grep -E 'innerHTML|outerHTML|insertAdjacentHTML|document\.write' docs/site/app.js || echo "DOM safety ok"
grep -E 'fetch\(["'\''`]?/' docs/site/app.js || echo "No root-absolute fetch ok"
```
Expected: All print `ok`.

- [x] **Step 3: Commit Task 3 changes**

```bash
git add docs/site/app.js
git commit -m "feat(site): implement search filtering, safe DOM rendering, and interactive controls in app.js"
```

---

### Task 4: Staged Verification & End-to-End Site Check

**Files:**
- Test against: `scripts/stage-site.sh`, `scripts/check-site.sh`, `scripts/gen-site.sh`.

- [x] **Step 1: Run site-check verification**

Run: `just site-check`
Expected: Exit code 0, all marketplace and catalog invariant tests pass.

- [x] **Step 2: Run site staging gate**

Run: `bash scripts/stage-site.sh`
Expected: Exit code 0, prints `stage-site: gate ok (N crates listed)`.

- [x] **Step 3: Inspect staged files in target/site**

Verify `target/site/index.html` has:
- Staged noscript list without `<!-- NOSCRIPT -->`.
- Staged CSS and JS files non-empty.
- Correct relative links.

- [x] **Step 4: Final commit and cleanup**

```bash
git status
```
Confirm working tree is clean.
