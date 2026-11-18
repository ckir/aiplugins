---
name: aiplugins-devtools-license-headers
description: Add or enforce per-file license headers with google/addlicense for custom or source-available licenses (PolyForm-NC plus extra terms). Use for addlicense, license, copyright or SPDX headers, header checks in CI or lefthook, or a LICENSE-ADDENDUM.
---

# License headers with addlicense (custom licenses)

`google/addlicense` has only four built-in presets (apache, mit, bsd, mpl), but it
already supports any license through `-f <template-file>`. No fork is needed. The
facts below come from reading the source at tag v1.2.0 (`main.go`, `tmpl.go`) and
were then confirmed by running the binary.

## The header text lives in a file, never in this skill

The user owns the legal wording. Do not invent or paraphrase license terms. Take
the template path from the user or the repo. If none exists, copy
`assets/header.tmpl.example` to the repo, tell the user it is a draft to edit,
and have them approve the wording before you apply anything.

The template is a Go `text/template` with three fields: `{{.Year}}`,
`{{.Holder}}`, `{{.SPDXID}}`. addlicense wraps it in the right comment syntax per
file type (`//` for .rs/.go, `#` for .sh/.py/.toml/.yml, `/** */` for .ts/.js,
`<!-- -->` for .html/.xml).

## Extra terms beyond the SPDX id: LICENSE-ADDENDUM

When the license is PolyForm (or similar) plus extra terms (evaluation grant,
anti-circumvention), keep `LICENSE` as the **verbatim** upstream text and put the
extra terms in a separate `LICENSE-ADDENDUM`. PolyForm asks that its text not be
modified, GitHub keeps detecting the license, and the extra terms stay visible.
Start from `assets/LICENSE-ADDENDUM.template`.

- The template has `[BRACKETS]` (licensor, year, evaluation days, contact,
  governing law). Those are the user's to fill: never invent a contact or a
  jurisdiction, and never commit the file with brackets left, because it is the
  live license of that repo. The commercial terms in it are one author's
  choices, so ask rather than assume they carry over.
- Point the header at both files (see `assets/header.tmpl.example`).
- Tell the user to have counsel read the addendum. It interacts with PolyForm's
  "noncommercial" definition, its Changes and New Works License, and its
  Violations clause (32-day cure).
- The SPDX id `PolyForm-Noncommercial-1.0.0` alone no longer describes the
  license once terms are added. Raise it before applying headers, because
  changing it later means rewriting every header: a `LicenseRef-...` id, with
  `license-file` instead of `license` in Cargo.toml. Ask the user to decide.

## Traps (each one measured, not guessed)

1. **Pass `-l <SPDX-id>` whenever the template uses `{{.SPDXID}}`.** With `-f`, the
   id is filled from `-l`, which defaults to `apache`, and that is silently
   rewritten to `Apache-2.0`. A PolyForm project would get a header claiming
   Apache-2.0 and no error. `-l` is not validated when `-f` is given.
2. **The template must contain `Copyright` or `SPDX-License-Identifier`.** addlicense
   decides "already has a header" by searching the file's first 1000 bytes for
   `copyright`, `mozilla public` or `spdx-license-identifier`
   (`hasLicense`, main.go). Without one of those, every run prepends another
   copy of the header.
3. **`-s` does nothing with `-f`** (except `-s=only`, which discards your template
   for a bare SPDX line). Put the SPDX line in your template instead.
4. **Unsupported extensions are skipped silently**: `.md`, `.json`, `.lock`,
   `.txt`, etc. They need no ignore entry, but `-check` also won't see them.
   Do not claim "every file has a header", only "every supported source file".
5. **Existing headers are never updated.** A file whose first 1000 bytes mention
   "copyright" (any copyright, including a third-party one) is left alone. To
   change the wording across the repo, strip the old header first (review the
   diff), then re-apply. Say so up front instead of promising an in-place update.
6. Files with a shebang or `<?xml`/doctype keep it on line 1; the header goes
   after it. Generated Go files (`// Code generated ... DO NOT EDIT.`) are skipped.

## Workflow

Use this skill's `scripts/license-headers.sh` (path relative to the skill folder). It encodes the traps above (refuses a template
with no copyright/SPDX marker, always passes `-l`, applies default ignores).

```bash
# 1. dry look: which files would change (check mode lists files missing a header)
<skill-dir>/scripts/license-headers.sh check --template LICENSE-HEADER.tmpl \
    --spdx PolyForm-Noncommercial-1.0.0 --holder "Name" --year 2026 .

# 2. apply, then review `git diff --stat` before committing
<skill-dir>/scripts/license-headers.sh apply --template LICENSE-HEADER.tmpl \
    --spdx PolyForm-Noncommercial-1.0.0 --holder "Name" --year 2026 .
```

Extra ignore globs: repeat `--ignore 'vendor/**'`. Defaults already cover
`target/`, `node_modules/`, `.git/`, `dist/`, lockfiles and `**/generated/**`.
Run from the repo root and stage only what `git diff --stat` shows you expect.

Prefer `git ls-files` scope over a directory walk when the repo has untracked or
vendored trees: `scripts/license-headers.sh apply ... $(git ls-files '*.rs')`.

## Enforcement (CI / pre-commit)

`check` exits non-zero and prints the path of each file missing a header, so it
drops into a hook. lefthook example:

```yaml
pre-commit:
  commands:
    license-headers:
      glob: "*.{rs,ts,js,py,sh,toml,yml,yaml}"
      run: <skill-dir>/scripts/license-headers.sh check --template LICENSE-HEADER.tmpl --spdx PolyForm-Noncommercial-1.0.0 --holder "Name" {staged_files}
```

`-check` only verifies a header *exists* (any copyright/SPDX text), not that it
matches your template. If wording drift matters, add a grep for a fixed line of
your template to the same hook.

## Also keep consistent

The SPDX id in the header must match the repo's manifest (`license = ...` in
Cargo.toml, `license` in package.json) and the LICENSE file. Grep those and
report any mismatch rather than fixing silently. If the license has extra terms
beyond the SPDX id (evaluation grant, anti-circumvention), the SPDX id alone is
incomplete, so the template should point to the file where the full terms live.

## Installing addlicense

`go install github.com/google/addlicense@v1.2.0` (or `which addlicense`). If the
tool is missing, install it rather than hand-writing headers with sed.
