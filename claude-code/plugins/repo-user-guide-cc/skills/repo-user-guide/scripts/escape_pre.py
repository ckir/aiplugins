#!/usr/bin/env python3
"""HTML-escape the contents of <pre> blocks marked with class "raw" so code can be pasted verbatim.
Usage: escape_pre.py in.html out.html
Write code as <pre class="raw"><code>...unescaped...</code></pre>; this turns & < > into entities and drops the 'raw' marker."""
import re, sys, html
src = open(sys.argv[1], encoding="utf8").read()
def fix(m):
    inner = m.group(2)
    attrs = re.sub(r'\s*class="raw"', "", m.group(1))
    c = re.fullmatch(r"(<code[^>]*>)(.*)(</code>)", inner, flags=re.S)
    body = c.group(1) + html.escape(c.group(2), quote=False) + c.group(3) if c else html.escape(inner, quote=False)
    return f"<pre{attrs}>" + body + "</pre>"
out = re.sub(r'<pre((?:\s[^>]*)?\sclass="[^"]*\braw\b[^"]*"[^>]*|\sclass="raw")>(.*?)</pre>', fix, src, flags=re.S)
open(sys.argv[2], "w", encoding="utf8").write(out)
