#!/usr/bin/env bash
# Verify the release plumbing for crate-less skills-only plugin tags
# (aiplugins-devtools-*-v*): skills-bundles.yml exists and is wired correctly,
# and the two workflows that would otherwise swallow those tags exclude them.
#
# Usage: check-skills-release-wiring.sh [--root <dir>]
#   --root  directory holding .github/workflows and agents.json (default: repo
#           root). Lets the checks run against a scratch copy for mutation tests.
set -euo pipefail
cd "$(dirname "$0")/.."

root="."
if [ "${1:-}" = "--root" ]; then
  root="${2:?--root needs a directory}"
fi

if ! python3 -c 'import yaml' 2>/dev/null; then
  echo "ERROR: PyYAML is required: python3 -m pip install --user pyyaml" >&2
  exit 1
fi

wf="$root/.github/workflows"
agents="$root/agents.json"
fail=0
bad() { echo "  FAIL  $*" >&2; fail=1; }

for f in release.yml opencode-bundles.yml skills-bundles.yml; do
  [ -f "$wf/$f" ] || bad "$wf/$f does not exist"
done
[ "$fail" -eq 0 ] || exit 1

# One pattern per line from on.push.tags (PyYAML parses the key `on` as True).
tags_of() {
  python3 - "$1" <<'PY'
import sys, yaml
d = yaml.safe_load(open(sys.argv[1]))
on = d.get("on", d.get(True)) or {}
for t in ((on.get("push") or {}).get("tags") or []):
    print(t)
PY
}

# EMULATION of GitHub's tag-filter semantics, not GitHub's evaluator: a tag is
# selected when the LAST pattern that matches it is positive; a pattern starting
# with `!` is negative. bash `case` globbing stands in for GitHub's glob (`*`
# there does not cross `/`; here it does, but these tags contain no `/`).
selected() { # <tag> <patterns-file>
  local tag="$1" result=no pat neg
  while IFS= read -r pat; do
    [ -n "$pat" ] || continue
    neg=0
    if [ "${pat:0:1}" = "!" ]; then neg=1; pat="${pat:1}"; fi
    # shellcheck disable=SC2254  # the glob in $pat is the point
    case "$tag" in
      $pat) if [ "$neg" -eq 1 ]; then result=no; else result=yes; fi ;;
    esac
  done <"$2"
  [ "$result" = yes ]
}

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
for f in release opencode-bundles skills-bundles; do
  tags_of "$wf/$f.yml" >"$scratch/$f.tags"
done

# (a) each exclusion present and AFTER every positive pattern.
for excl in '!aiplugins-devtools-*-v*' '!repo-user-guide-*-v*'; do
  for f in release opencode-bundles; do
    excl_line="$(grep -n -x -F -e "$excl" "$scratch/$f.tags" | head -1 | cut -d: -f1 || true)"
    if [ -z "$excl_line" ]; then
      bad "$f.yml on.push.tags lacks '$excl'"
      continue
    fi
    last_pos="$(grep -n -v '^!' "$scratch/$f.tags" | tail -1 | cut -d: -f1 || true)"
    if [ -n "$last_pos" ] && [ "$excl_line" -le "$last_pos" ]; then
      bad "$f.yml: '$excl' must come AFTER the positive tag patterns (order matters)"
    fi
  done
done

# (b) skills-bundles.yml structure.
if ! python3 - "$wf/skills-bundles.yml" <<'PY'
import re, sys, yaml
d = yaml.safe_load(open(sys.argv[1]))
errs = []
jobs = d.get("jobs") or {}
for j in ("bundle", "verify"):
    if j not in jobs:
        errs.append("jobs.%s missing" % j)
if "verify" in jobs:
    n = jobs["verify"].get("needs")
    if n not in ("bundle", ["bundle"]):
        errs.append("jobs.verify.needs must be 'bundle', got %r" % (n,))
perm = (d.get("permissions") or {}).get("contents")
if perm != "write":
    errs.append("permissions.contents must be 'write', got %r" % (perm,))
for j in ("bundle", "verify"):
    env = (jobs.get(j) or {}).get("env") or {}
    for k in ("GH_TOKEN", "TAG"):
        if k not in env:
            errs.append("jobs.%s.env lacks %s" % (j, k))
for j in ("bundle", "verify"):
    tag = str(((jobs.get(j) or {}).get("env") or {}).get("TAG", ""))
    if "inputs.tag" not in tag:
        errs.append("jobs.%s.env.TAG must contain inputs.tag (workflow_dispatch), got %r" % (j, tag))

def runs(job):
    return "\n".join(st["run"] for st in (job.get("steps") or []) if "run" in st)

# The bundle job must create a missing release before it uploads to it, and the
# upload must overwrite (re-cuts).
b = runs(jobs.get("bundle") or {})
pv, pc, pu = b.find("gh release view"), b.find("gh release create"), b.find("gh release upload")
if pv < 0 or pc < 0 or pu < 0 or not (pv < pc < pu):
    errs.append("jobs.bundle run: needs `gh release view`, then `gh release create`, then `gh release upload`")
if pu >= 0 and "--clobber" not in b[pu:].split("\n", 1)[0]:
    errs.append("jobs.bundle: `gh release upload` must pass --clobber")

# Every $(jq -r ...) capture must strip CR (the Windows image's jq writes CRLF).
def substitutions(text):
    i = 0
    while True:
        i = text.find("$(", i)
        if i < 0:
            return
        j, depth, q = i + 2, 1, None
        while j < len(text) and depth:
            c = text[j]
            if q:
                if c == q:
                    q = None
            elif c in "'\"":
                q = c
            elif c == "(":
                depth += 1
            elif c == ")":
                depth -= 1
            j += 1
        yield text[i + 2:j - 1]
        i += 2

for jn in ("bundle", "verify"):
    for body in substitutions(runs(jobs.get(jn) or {})):
        if body.lstrip().startswith("jq -r") and not re.search(r"\|\s*tr -d '\\r'\s*$", body):
            errs.append("jobs.%s: jq -r capture lacks `| tr -d '\\r'`: %s" % (jn, " ".join(body.split())[:70]))
for e in errs:
    print("  FAIL  skills-bundles.yml: " + e, file=sys.stderr)
sys.exit(1 if errs else 0)
PY
then fail=1; fi

# (c) tag-filter emulation.
expect() { # <yes|no> <workflow> <tag> <file-base>
  local want="$1" name="$2" tag="$3" base="$4" got=no
  if selected "$tag" "$scratch/$base.tags"; then got=yes; fi
  [ "$got" = "$want" ] || bad "$name filter: tag $tag selected=$got, expected $want"
}
while IFS= read -r k; do
  expect yes skills-bundles.yml "$k-v0.1.0" skills-bundles
  expect no release.yml "$k-v0.1.0" release
done < <(jq -r '.agents[].skillsOnly[]?' "$agents")
expect no opencode-bundles.yml aiplugins-devtools-opencode-v0.1.0 opencode-bundles
expect no opencode-bundles.yml repo-user-guide-opencode-v0.1.0 opencode-bundles
expect yes opencode-bundles.yml rtk-mcp-opencode-v0.7.3 opencode-bundles
expect no release.yml rtk-mcp-opencode-v0.7.3 release
expect yes release.yml rtk-mcp-cc-v0.8.0 release
expect no skills-bundles.yml rtk-mcp-cc-v0.8.0 skills-bundles

# (d) run: blocks parse as bash and carry no ${{ }} interpolation.
python3 - "$wf/skills-bundles.yml" "$scratch" <<'PY'
import sys, yaml
d = yaml.safe_load(open(sys.argv[1]))
n = 0
for jn, job in (d.get("jobs") or {}).items():
    for i, st in enumerate(job.get("steps") or []):
        if "run" in st:
            open("%s/run-%s-%d.raw" % (sys.argv[2], jn, i), "w").write(st["run"])
            n += 1
PY
for raw in "$scratch"/run-*.raw; do
  [ -e "$raw" ] || { bad "skills-bundles.yml has no run: blocks"; break; }
  # shellcheck disable=SC2016  # literal ${{ is the thing searched for
  if grep -q '\${{' "$raw"; then
    bad "$(basename "$raw" .raw): \${{ }} interpolated into a run: script body (use env:)"
  fi
  sed -E 's/\$\{\{[^}]*\}\}/X/g' "$raw" >"$raw.sh"
  bash -n "$raw.sh" 2>/dev/null || bad "$(basename "$raw" .raw): run: block fails bash -n"
done

if [ "$fail" -ne 0 ]; then exit 1; fi
echo "Skills release wiring consistent."
