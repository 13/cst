#!/usr/bin/env python3
"""Turn a terminal capture with ANSI colours (tmux capture-pane -e -p) into
an SVG on a monospace grid. Standard library only.

    tmux capture-pane -e -p | scripts/ansi2svg.py --bg '#313244' > out.svg
    scripts/ansi2svg.py --selftest
"""
import re
import sys
import unicodedata
from html import escape

CW, LH, FS = 8.4, 18, 14  # cell width, line height, font size (px)
SGR = re.compile(r"\x1b\[([0-9;]*)m")
OTHER = re.compile(r"\x1b\[[0-9;?]*[A-Za-ln-z]|\x1b\][^\x07]*\x07|\x1b[()][A-Za-z0-9]")
BASE16 = ["000000", "800000", "008000", "808000", "000080", "800080", "008080", "c0c0c0",
          "808080", "ff0000", "00ff00", "ffff00", "0000ff", "ff00ff", "00ffff", "ffffff"]


def xterm256(n):
    if n < 16:
        return "#" + BASE16[n]
    if n < 232:
        n -= 16
        steps = [0, 95, 135, 175, 215, 255]
        return "#%02x%02x%02x" % (steps[n // 36], steps[n // 6 % 6], steps[n % 6])
    v = 8 + 10 * (n - 232)
    return "#%02x%02x%02x" % (v, v, v)


def cells(ch):
    if unicodedata.category(ch) in ("Mn", "Me", "Cf"):
        return 0
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def apply_sgr(params, st, fg0):
    p = [int(x) if x else 0 for x in params.split(";")] if params else [0]
    i = 0
    while i < len(p):
        c = p[i]
        if c == 0:
            st.update(fg=fg0, bg=None, bold=False)
        elif c == 1:
            st["bold"] = True
        elif c == 22:
            st["bold"] = False
        elif c in (38, 48) and i + 1 < len(p):
            key = "fg" if c == 38 else "bg"
            if p[i + 1] == 2 and i + 4 < len(p):
                st[key] = "#%02x%02x%02x" % tuple(p[i + 2:i + 5])
                i += 4
            elif p[i + 1] == 5 and i + 2 < len(p):
                st[key] = xterm256(p[i + 2])
                i += 2
        elif c == 39:
            st["fg"] = fg0
        elif c == 49:
            st["bg"] = None
        elif 30 <= c <= 37:
            st["fg"] = xterm256(c - 30)
        elif 90 <= c <= 97:
            st["fg"] = xterm256(c - 90 + 8)
        elif 40 <= c <= 47:
            st["bg"] = xterm256(c - 40)
        i += 1


def convert(text, bg0="#1e1e2e", fg0="#cdd6f4"):
    lines = text.rstrip("\n").split("\n")
    st = {"fg": fg0, "bg": None, "bold": False}
    body, width = [], 0
    for y, line in enumerate(lines):
        col = 0
        for k, part in enumerate(SGR.split(OTHER.sub("", line))):
            if k % 2 == 1:
                apply_sgr(part, st, fg0)
                continue
            if not part:
                continue
            n = sum(cells(c) for c in part)
            x, top = col * CW, y * LH
            if st["bg"]:
                body.append(f'<rect x="{x:.1f}" y="{top}" width="{n * CW:.1f}" height="{LH}" fill="{st["bg"]}"/>')
            if part.strip():
                bold = ' font-weight="bold"' if st["bold"] else ""
                body.append(f'<text x="{x:.1f}" y="{top + LH - 5}" fill="{st["fg"]}"{bold} '
                            f'textLength="{n * CW:.1f}" lengthAdjust="spacingAndGlyphs">{escape(part)}</text>')
            col += n
        width = max(width, col)
    w, h = width * CW, len(lines) * LH
    font = "ui-monospace, 'JetBrains Mono', 'DejaVu Sans Mono', Menlo, monospace"
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0f}" height="{h}" viewBox="0 0 {w:.0f} {h}" '
            f'font-family="{font}" font-size="{FS}" xml:space="preserve">\n'
            f'<rect width="100%" height="100%" rx="10" fill="{bg0}"/>\n' + "\n".join(body) + "\n</svg>\n")


def selftest():
    svg = convert("\x1b[1;38;2;255;0;0mHi\x1b[0m x\n\x1b[48;5;196m 日 \x1b[0m", bg0="#000000")
    assert 'fill="#ff0000" font-weight="bold"' in svg, svg   # truecolour fg + bold
    assert ">Hi<" in svg and "> x<" in svg, svg
    assert 'fill="#ff0000"/>' in svg, svg                     # 256-colour 196 background rect
    assert 'width="33.6"' in svg, svg                          # " 日 " is 4 cells of 8.4px
    assert svg.startswith("<svg") and svg.rstrip().endswith("</svg>"), svg
    print("ansi2svg selftest OK")


def main():
    args = sys.argv[1:]
    if "--selftest" in args:
        selftest()
        return
    bg = args[args.index("--bg") + 1] if "--bg" in args else "#1e1e2e"
    sys.stdout.write(convert(sys.stdin.read(), bg0=bg))


if __name__ == "__main__":
    main()
