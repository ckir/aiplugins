# Clavity-Style Marketplace Site Design Specification

## Overview

This specification defines the redesign and expansion of the **AI Plugins Marketplace** storefront (`docs/site/`), bringing the visual polish, terminal demonstration, multi-agent workflow representation, and rich presentation of [ckir.github.io/clavity](https://ckir.github.io/clavity/) to `https://ckir.github.io/aiplugins/`.

The site acts as the primary showcase, documentation hub, and plugin catalog for the repository, serving users of **Claude Code**, **Google Antigravity**, **OpenCode**, and **Qwen**.

---

## Architectural Principles & Constraints

1. **Single-Page Application Structure:**
   * Built in `docs/site/` as a static web application (`index.html`, `styles.css`, `app.js`, `aiplugins.svg`) deployed via GitHub Pages alongside generated Rust documentation (`api/`).
   * Adheres to all verification constraints checked by `scripts/stage-site.sh`:
     * No root-absolute URLs (no `href="/"`, `src="/"`, `url("/"`). All links are relative (`./`, `../`, `#anchor`).
     * `index.html` contains the required `<!-- NOSCRIPT -->` marker and the `href="api/index.html"` link.
     * `app.js` builds DOM elements purely with safe DOM methods (`document.createElement`, `setAttribute`, `textContent`, `append`, `replaceChildren`). Never uses `innerHTML`, `outerHTML`, `insertAdjacentHTML`, or `document.write`.
     * Retains full compatibility with `scripts/check-site.sh` and `catalog.json` generation.

2. **Performance & Lightweight Execution:**
   * Zero external JavaScript or CSS dependencies.
   * Total uncompressed asset footprint under 100 KB.
   * Full functionality with JavaScript disabled (via `<noscript>` fallback listing all plugins, descriptions, and direct links).

3. **Responsive & Accessible Design:**
   * Mobile-first responsive layout ranging from 320px phone screens to wide desktop displays.
   * WCAG AA contrast compliance across both dark and light modes.
   * ARIA tablists, keyboard navigation (Arrow keys, Home, End), and live status announcements for copy actions.

---

## Visual Design & Theming

### 1. Color Palette & Cyberpunk Dark Theme
Inspired by Clavity's dark cyber aesthetic:
* **Backgrounds:**
  * Base Background: `#020713`
  * Deep Background: `#001333`
  * Mid / Borders: `#052b70`
  * Translucent Panels: `rgba(5, 43, 112, 0.35)` with `backdrop-filter: blur(8px)`
* **Agent Color Accents:**
  * Claude Code: Neon Orange (`--claude: #ff4b18`, soft: `#ffb14a`)
  * Google Antigravity: Neon Cyan (`--agy: #00cfff`, soft: `#00eaff`)
  * OpenCode: Terminal Emerald (`--opencode: #22c55e`, soft: `#4ade80`)
  * Qwen: Royal Purple (`--qwen: #a855f7`, soft: `#c084fc`)
* **Footprint Meters:**
  * Resident: Cyan (`#00cfff`)
  * Invocation: Amber (`#ffb14a`)
  * Setup: Emerald (`#22c55e`)
* **Light Theme Fallback:**
  * Clean, accessible high-contrast palette for users who toggle the theme switch.

### 2. Brand Identity (`aiplugins.svg`)
* A custom vector SVG emblem featuring 4 interconnected agent nodes converging into a high-tech modular plug icon.
* Embedded in the navbar, hero banner, favicon, and footer.

---

## Page Components & Sections

### 1. Global Sticky Navigation (`.site-nav`)
* **Brand:** `aiplugins.svg` logo with bold title "AI Plugins".
* **Navigation Links:**
  * `#demo` (Demo)
  * `#values` (Why AI Plugins)
  * `#how-it-works` (Architecture)
  * `#catalog` (Plugin Catalog)
  * `#install` (Quick Install)
  * `#docs` (Documentation)
* **Actions:**
  * Theme Toggle (`#theme-toggle`) switching between Dark and Light mode, persisting to `localStorage`.
  * Call-to-action button (`.nav-cta`) scrolling directly to `#catalog`.

### 2. Hero Section (`.hero`)
* **Header Tag:** `Cross-Agent Monorepo & MCP Marketplace`
* **Headline:** *"One Marketplace. Four Autonomous Agents."*
* **Subline:** *"Production-ready plugins, tools, and MCP servers for Claude Code, Antigravity, OpenCode, and Qwen — with zero-compile instant installs and strict context budgets."*
* **Workflow Stages Chips:**
  A 6-stage interactive pipeline connected by glowing arrows (`→` desktop, `↓` mobile):
  1. `Discover` — Search the `aiplugins` catalog & releases.
  2. `Install` — One-liner instant installs per agent.
  3. `Bridge` — Expose tools over standard MCP servers.
  4. `Optimize` — `rtk-mcp` 60–90% shell token reduction.
  5. `Delegate` — Worker subagent dispatch via `opencode-ctl-mcp`.
  6. `Review` — `aiplugins-devtools` multi-axis quality gate.
* **Hero CTAs:**
  * Primary Button: `Explore Plugins` (`#catalog`)
  * Secondary Button: `Quick Install` (`#install`)
  * Tertiary Link: `User Guide (PDF)` (`USER_GUIDE.pdf`)

### 3. Dual-Agent Terminal Demo Window (`#demo`)
* Styled as a desktop terminal window with red, yellow, and green window buttons and title bar.
* **Left Pane (Driver: Claude Code):**
  * Shows Claude executing token-optimized commands via `rtk-mcp` (`rewrote 'git status' -> 82% token savings`).
  * Dispatches an MCP worker task via `opencode-ctl-mcp`.
* **Right Pane (Worker/Peer: Antigravity & OpenCode):**
  * Shows headless Ghidra worker decompiling a binary symbol (`shared/ghidra-mcp`).
  * Executes a multi-axis code review pass via `aiplugins-devtools` and yields `VERDICT: GREEN`.

### 4. Value Proposition Grid (`#values`)
Three cyberpunk-bordered feature cards:
1. **Zero-Compile Instant Installs:** Prebuilt binaries for Windows (x64), Linux (x86_64, aarch64), and macOS (x86_64, Apple Silicon).
2. **Context Footprint Transparency:** Full visibility into Resident vs Invocation vs Setup bytes before installing.
3. **Cross-Agent Unified Rust Core:** Shared, memory-safe engines in `shared/` fronted natively across agents.

### 5. "How It Works" Architectural Walkthrough (`#how-it-works`)
* Explains the monorepo design, shared Rust crates, agent-specific marketplace manifests, and runtime hook integrations.

### 6. Interactive Plugin Catalog & Footprints (`#catalog`)
* **Live Search:** Instant client-side filtering by plugin name, description, or keyword.
* **Agent Filter Chips:** `All`, `Claude Code`, `OpenCode`, `Antigravity`, `Qwen`.
* **Card Anatomy:**
  * Family key and agent tab list.
  * Version badge and link to plugin README.
  * Context footprint meters with budget comparison bars.
  * Pre-formatted install command with one-click `Copy` button.
* **Accessible `<noscript>` Fallback:** Preserves full listing of plugins, descriptions, and links for search crawlers and non-JS clients.

### 7. Quick Install Section (`#install`)
* Direct copyable commands for:
  * Adding the marketplace: `claude plugin marketplace add ckir/aiplugins`
  * Installing headline plugins across Claude Code, Antigravity, OpenCode, and Qwen.
* Accessible live status announcer for screen readers (`#live`).

### 8. Documentation & Footer (`#docs`, `footer`)
* Links to `USER_GUIDE.pdf`, Rustdoc API index (`api/index.html`), GitHub, Changelog, and License.
* Trademark notices and copyright disclaimer.

---

## Verification Plan

1. **Staged Tree Gate:**
   * Run `just site-check` to execute `scripts/gen-site.sh`, `scripts/check-site.sh`, and `scripts/check-site.sh --selftest`.
   * Run `bash scripts/stage-site.sh` to confirm collision guards and crate gates pass.
2. **DOM Safety & Constraint Audit:**
   * Ensure `app.js` contains no `innerHTML`, `outerHTML`, or `document.write`.
   * Ensure no root-absolute URLs exist in any site asset.
3. **Responsive & Browser Testing:**
   * Test mobile view (320px) up to full desktop width.
   * Verify dark and light theme toggle functionality.
