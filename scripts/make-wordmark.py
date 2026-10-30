"""Writes assets/wordmark.svg from the outlines of a font.

The font is Playfair Display Italic, under the SIL Open Font License 1.1.
The repository does not hold the font. Get it from
https://github.com/google/fonts/tree/main/ofl/playfairdisplay

Usage:

    pip install fonttools
    python scripts/make-wordmark.py "PlayfairDisplay-Italic[wght].ttf" > assets/wordmark.svg
"""

import sys
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

font = instantiateVariableFont(TTFont(sys.argv[1]), {"wght": 900})
gs = font.getGlyphSet()
cmap = font.getBestCmap()
upm = font["head"].unitsPerEm
W, H = 420, 190


def num(v):
    return f"{v:.1f}".rstrip("0").rstrip(".")


def line(text, size, baseline, tracking):
    s = size / upm
    names = [cmap[ord(c)] for c in text]
    adv = [gs[n].width * s for n in names]
    total = sum(adv) + tracking * (len(text) - 1)
    x = (W - total) / 2
    parts = []
    for n, a in zip(names, adv):
        pen = SVGPathPen(gs, ntos=num)
        gs[n].draw(TransformPen(pen, (s, 0, 0, -s, x, baseline)))
        parts.append(pen.getCommands())
        x += a + tracking
    return " ".join(parts)


top = line("DLGE", 104, 104, 6)
bottom = line("PLAYER", 46, 164, 14)
print(f'''<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" role="img" aria-label="DLGE Player">
  <!-- The letters are outlines of Playfair Display Black Italic, by the
       Playfair Display Project Authors, under the SIL Open Font License 1.1. -->
  <defs>
    <linearGradient id="blood" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#f43f5e"/>
      <stop offset="0.55" stop-color="#be123c"/>
      <stop offset="1" stop-color="#6b0f1a"/>
    </linearGradient>
    <filter id="shadow" x="-10%" y="-20%" width="120%" height="150%">
      <feDropShadow dx="4" dy="6" stdDeviation="3" flood-color="#000000" flood-opacity="0.55"/>
    </filter>
  </defs>
  <g fill="url(#blood)" filter="url(#shadow)">
    <path d="{top}"/>
    <path d="{bottom}"/>
  </g>
</svg>''')
