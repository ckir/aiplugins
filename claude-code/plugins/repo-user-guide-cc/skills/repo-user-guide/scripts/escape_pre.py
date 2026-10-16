#!/usr/bin/env python3
"""HTML-escape the contents of elements marked with the class token "raw", so code can be pasted verbatim.
Usage: escape_pre.py in.html out.html

Two forms are handled:
  <pre class="raw"><code>...unescaped...</code></pre>   code block (extra classes are kept:
                                                            <pre class="output wide raw">; <code> is optional)
  <code class="raw">...unescaped...</code>                   inline code in prose, e.g. <code class="raw"><title></code>
`&`, `<` and `>` become entities and the "raw" token is dropped, so running the script again is a no-op. Only
the exact class token "raw" counts (a class named "no-raw" or "rawdata" is left alone). Never mark content that
is already escaped as raw: it would be escaped twice."""
import html
import re
import sys

CLASS = re.compile(r'\sclass\s*=\s*"([^"]*)"')
PRE = re.compile(r"<pre(\s[^>]*)?>(.*?)</pre>", re.S)
INLINE = re.compile(r"<code(\s[^>]*)?>(.*?)</code>", re.S)


def strip_raw(attrs):
    """Return attrs without the raw token, or None if the element is not marked raw."""
    cm = CLASS.search(attrs)
    if not cm or "raw" not in cm.group(1).split():
        return None
    rest = " ".join(t for t in cm.group(1).split() if t != "raw")
    return attrs[: cm.start()] + (f' class="{rest}"' if rest else "") + attrs[cm.end():]


def fix_pre(m):
    attrs = strip_raw(m.group(1) or "")
    if attrs is None:  # pre not marked raw; its <code class="raw"> wrapper may be
        return INLINE.sub(fix_inline, m.group(0))
    inner = m.group(2)
    c = re.fullmatch(r"(<code[^>]*>)(.*)(</code>)", inner, flags=re.S)
    body = (c.group(1) + html.escape(c.group(2), quote=False) + c.group(3)) if c else html.escape(inner, quote=False)
    return f"<pre{attrs}>{body}</pre>"


def fix_inline(m):
    attrs = strip_raw(m.group(1) or "")
    if attrs is None:
        return m.group(0)
    return f"<code{attrs}>{html.escape(m.group(2), quote=False)}</code>"


src = open(sys.argv[1], encoding="utf8").read()
# Pre blocks first (their inner <code> is handled there), then inline <code> outside any <pre>.
out, pos = [], 0
for m in PRE.finditer(src):
    out.append(INLINE.sub(fix_inline, src[pos:m.start()]))
    out.append(fix_pre(m))
    pos = m.end()
out.append(INLINE.sub(fix_inline, src[pos:]))
open(sys.argv[2], "w", encoding="utf8").write("".join(out))
