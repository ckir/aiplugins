#!/usr/bin/env python3
"""Pre-render check for <pre> blocks. Usage: check_pre.py guide.html [--max-width 110]

Reports, with line numbers in the HTML file:
  * lines wider than --max-width characters inside <pre class="... wide ...">  (they are not re-wrapped and
    would be cut off by the page; shorten them or drop the `wide` class so they wrap)
  * text that still shows entities such as &lt; &amp; &gt; once the HTML is decoded, which means the block
    was escaped twice (a raw block must hold unescaped text)
  * <pre> blocks or inline <code> elements still marked `raw` (escape_pre.py has not been run)
  * a command <pre> followed by an output <pre> that is not wrapped in <div class="ex"> (page breaks could split them)
Exit code 0 = clean, 1 = findings."""
import html
import re
import sys

path = sys.argv[1]
limit = int(sys.argv[sys.argv.index("--max-width") + 1]) if "--max-width" in sys.argv else 110
src = open(path, encoding="utf8").read()
bad = 0
PRE_RE = re.compile(r"<pre(\s[^>]*)?>(.*?)</pre>", re.S)
for m in PRE_RE.finditer(src):
    attrs, inner = m.group(1) or "", m.group(2)
    line0 = src.count("\n", 0, m.start()) + 1
    cm = re.search(r'\sclass\s*=\s*"([^"]*)"', attrs)
    classes = cm.group(1).split() if cm else []
    text = html.unescape(re.sub(r"<[^>]+>", "", inner))
    if "raw" in classes:
        print(f"{path}:{line0}: <pre> is still marked raw (run escape_pre.py)"); bad += 1
    if re.search(r"&(lt|gt|amp|quot|#\d+);", text):
        print(f"{path}:{line0}: decoded text still contains an HTML entity (escaped twice?)"); bad += 1
    if "wide" in classes:
        for i, ln in enumerate(text.split("\n")):
            if len(ln) > limit:
                print(f"{path}:{line0 + i}: wide line is {len(ln)} chars (> {limit}); shorten it or drop `wide`"); bad += 1
# A command <pre> immediately followed by an output <pre> must sit in <div class="ex"> so a page break cannot
# separate them (the template's .ex rule keeps the pair together).
for m in re.finditer(r'</pre>\s*<pre(\s[^>]*)?>', src):
    cm = re.search(r'\sclass\s*=\s*"([^"]*)"', m.group(1) or "")
    if not (cm and "output" in cm.group(1).split()):
        continue
    before = src[: m.start()]
    last = None
    for t in re.finditer(r'<div(\s[^>]*)?>|</div>', before):
        last = t
    wrapped = bool(last and last.group(0) != "</div>" and re.search(r'class\s*=\s*"[^"]*\bex\b', last.group(0)))
    if not wrapped:
        print(f"{path}:{src.count(chr(10), 0, m.end()) + 1}: command block followed by an output block is not wrapped in <div class=\"ex\">"); bad += 1
# <code class="raw"> that escape_pre.py has not processed yet (inline, or inside a <pre>).
for m in re.finditer(r"<code(\s[^>]*)?>", src):
    cm = re.search(r'\sclass\s*=\s*"([^"]*)"', m.group(1) or "")
    if cm and "raw" in cm.group(1).split():
        print(f"{path}: inline <code class=\"raw\"> is still marked raw (run escape_pre.py)"); bad += 1
print("check_pre: clean" if not bad else f"check_pre: {bad} finding(s)")
sys.exit(1 if bad else 0)
