# aiplugins-devtools-qwen

A Qwen Code extension that ships dev-tool skills. The first skill is
`code-review-and-quality`, a multi-axis code review (correctness, readability,
architecture, security, performance) with security and performance checklists.

## Installation

### Via `qwen extensions install`

```bash
qwen extensions install https://github.com/ckir/aiplugins/releases/download/aiplugins-devtools-qwen-v0.1.0/aiplugins-devtools-qwen-extension.zip
```

> **Known issue:** As of Qwen Code v0.22.3, `qwen extensions install` from
> an archive URL exits silently without installing (tracked in
> [QwenLM/qwen-code#10741](https://github.com/QwenLM/qwen-code/issues/10741)).
> Use the manual install below until this is fixed.

### Manual install

1. Download the extension bundle:

   | Platform | URL |
   |---|---|
   | All | https://github.com/ckir/aiplugins/releases/download/aiplugins-devtools-qwen-v0.1.0/aiplugins-devtools-qwen-extension.zip |

2. Extract it to your Qwen extensions directory. The installed directory is named after the manifest `name` (`aiplugins-devtools`), not after this repository folder:

   ```bash
   # Linux / macOS
   mkdir -p ~/.qwen/extensions/aiplugins-devtools
   unzip aiplugins-devtools-qwen-extension.zip -d ~/.qwen/extensions/aiplugins-devtools/
   ```

   ```powershell
   # Windows (PowerShell)
   New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.qwen\extensions\aiplugins-devtools"
   Expand-Archive -Path aiplugins-devtools-qwen-extension.zip -DestinationPath "$env:USERPROFILE\.qwen\extensions\aiplugins-devtools" -Force
   ```

3. Verify it was detected:

   ```bash
   qwen extensions list
   ```

   You should see `aiplugins-devtools - dev-tool skills` listed and enabled.

### Building from source

```bash
git clone https://github.com/ckir/aiplugins.git
cd aiplugins
qwen extensions link ./qwen/plugins/aiplugins-devtools-qwen
```

## Attribution

The skill is adapted from Addy Osmani's agent-skills; see
`skills/code-review-and-quality/THIRD-PARTY-NOTICE.md` for attribution and the
upstream licence. The canonical source lives in `shared/devtools/` and is copied
here by `just emit-devtools-skills`.
