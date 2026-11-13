#!/usr/bin/env python3
"""Original night-expressway illustrations for the launcher's hub.

Dev-time asset authoring only (the outputs, assets/art/*.jpg, are committed;
nothing in the build or the package runs this). Every scene is drawn here
from shapes and a fixed seed: no photos, no game files, no official art.

usage: python3 -I scripts/art/scenes.py <out_dir>   (needs rsvg-convert, Pillow)
"""
import math
import os
import random
import subprocess
import sys

W, H = 2560, 1440
HORIZON = 760


def hexrgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def mix(a, b, t):
    a, b = hexrgb(a), hexrgb(b)
    return '#%02x%02x%02x' % tuple(round(x + (y - x) * t) for x, y in zip(a, b))


class Scene:
    def __init__(self, slug, seed, sky_top, sky_glow, light, accent, landmark, rain=False, car='#151b24'):
        self.slug, self.seed = slug, seed
        self.sky_top, self.sky_glow, self.light, self.accent = sky_top, sky_glow, light, accent
        self.landmark, self.rain, self.car = landmark, rain, car


SCENES = [
    # Wangan Midnight R: Ginza-style neon canyon, magenta night
    Scene('wangan-r', 2, '#0a0716', '#5b1d5e', '#ff7ad9', '#ffd1f0', 'canyon'),
    # Maximum Tune: the Bayshore Route straight under sodium lamps
    Scene('mt1', 4, '#07101a', '#6b3a12', '#f2a33a', '#ffd9a0', 'straight'),
    # MT3: suspension bridge over the bay, cyan
    Scene('mt3', 7, '#06121c', '#14506a', '#57d4ff', '#c9f3ff', 'suspension'),
    # MT3DX+: cable-stayed bridge, steel blue dusk
    Scene('mt3dxp', 11, '#0b1020', '#3a4a7a', '#9fb4ff', '#e3e9ff', 'cablestay'),
    # MT5: C1 tunnel, green fluorescent tiles
    Scene('mt5', 13, '#06120e', '#0e3b2c', '#5cf2a8', '#d6ffe9', 'tunnel'),
    # MT5DX+: mountain pass before dawn, rose
    Scene('mt5dxp', 17, '#140a18', '#7a3550', '#ff8fa3', '#ffe0e6', 'mountain'),
    # MT6: truss bridge, deep red
    Scene('mt6', 19, '#120709', '#5e1414', '#ff5a4a', '#ffd4cc', 'truss'),
    # MT6R: rain on the expressway, violet reflections
    Scene('mt6r', 23, '#0b0818', '#3b2a6e', '#b18cff', '#efe6ff', 'straight', rain=True),
    # MT6RR: spiral ramp junction, gold
    Scene('mt6rr', 29, '#0f0b05', '#6a4a10', '#ffcc4d', '#fff1c7', 'spiral'),
]


def defs(s):
    return f'''<defs>
  <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="{s.sky_top}"/>
    <stop offset="0.55" stop-color="{mix(s.sky_top, s.sky_glow, 0.55)}"/>
    <stop offset="1" stop-color="{s.sky_glow}"/>
  </linearGradient>
  <linearGradient id="road" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="{mix('#0b0f15', s.sky_glow, 0.35)}"/>
    <stop offset="1" stop-color="#05070a"/>
  </linearGradient>
  <radialGradient id="vig" cx="0.5" cy="0.5" r="0.75">
    <stop offset="0.55" stop-color="#000" stop-opacity="0"/>
    <stop offset="1" stop-color="#000" stop-opacity="0.75"/>
  </radialGradient>
  <linearGradient id="carbody" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="{mix(s.car, s.light, 0.18)}"/>
    <stop offset="0.5" stop-color="{s.car}"/>
    <stop offset="1" stop-color="#05070a"/>
  </linearGradient>
  <filter id="blur8" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="8"/></filter>
  <filter id="blur24" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="24"/></filter>
  <filter id="blur60" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="60"/></filter>
</defs>'''


def skyline(r, s, base, layers=3):
    out = []
    for layer in range(layers):
        shade = mix('#020306', s.sky_glow, 0.10 + 0.12 * (layers - 1 - layer))
        x = -40
        while x < W + 40:
            bw = r.randint(40, 140)
            bh = r.randint(60, 380 - layer * 80) * (1.3 if r.random() < 0.08 else 1)
            top = base - bh - layer * 10
            out.append(f'<rect x="{x}" y="{top:.0f}" width="{bw}" height="{base - top + 4:.0f}" fill="{shade}"/>')
            if layer == layers - 1 or r.random() < 0.6:
                for wy in range(int(top) + 10, base - 8, 14):
                    for wx in range(x + 6, x + bw - 6, 11):
                        if r.random() < 0.18:
                            c = s.accent if r.random() < 0.7 else s.light
                            out.append(f'<rect x="{wx}" y="{wy}" width="5" height="6" fill="{c}" opacity="{r.uniform(0.35, 0.9):.2f}"/>')
            if bh > 300 and r.random() < 0.6:
                out.append(f'<circle cx="{x + bw / 2:.0f}" cy="{top - 4:.0f}" r="4" fill="#ff3b30"/>')
            x += bw + r.randint(0, 18)
    return '\n'.join(out)


def road(r, s, vx):
    """One-point perspective expressway from the bottom edge to (vx, HORIZON)."""
    out = [f'<rect x="0" y="{HORIZON}" width="{W}" height="{H - HORIZON}" fill="{mix("#040507", s.sky_glow, 0.10)}"/>',
           f'<polygon points="-400,{H} {W + 400},{H} {vx + 30},{HORIZON} {vx - 30},{HORIZON}" fill="url(#road)"/>']
    # side walls
    for side in (-1, 1):
        x0 = -400 if side < 0 else W + 400
        out.append(f'<polygon points="{x0},{H} {x0},{H - 560} {vx + side * 34},{HORIZON - 14} {vx + side * 30},{HORIZON}" '
                   f'fill="{mix("#06080b", s.sky_glow, 0.12)}"/>')
        out.append(f'<line x1="{x0}" y1="{H - 560}" x2="{vx + side * 34}" y2="{HORIZON - 14}" stroke="{s.light}" stroke-width="5" opacity="0.55"/>')
    # lane dashes
    for lane in (-1.5, -0.5, 0.5, 1.5):
        for i in range(26):
            t0, t1 = (i / 26) ** 2.2, ((i + 0.45) / 26) ** 2.2
            def pt(t):
                y = HORIZON + (H - HORIZON) * t
                x = vx + lane * 900 * t
                return x, y
            (x0, y0), (x1, y1) = pt(t0), pt(t1)
            w0, w1 = 1 + 14 * t0, 1 + 14 * t1
            out.append(f'<polygon points="{x0 - w0},{y0} {x0 + w0},{y0} {x1 + w1},{y1} {x1 - w1},{y1}" fill="#e9edf2" opacity="{0.25 + 0.5 * t1:.2f}"/>')
    # sodium lamps on both walls
    for side in (-1, 1):
        for i in range(1, 16):
            t = (i / 16) ** 2.0
            x = vx + side * (34 + 1500 * t)
            y = HORIZON - 12 - 340 * t
            pole_base = HORIZON - 14 + (H - 560 - HORIZON + 14) * t
            out.append(f'<line x1="{x:.0f}" y1="{y:.0f}" x2="{x:.0f}" y2="{pole_base:.0f}" stroke="#0c0f14" stroke-width="{1 + 6 * t:.1f}"/>')
            rad = 3 + 40 * t
            out.append(f'<circle cx="{x:.0f}" cy="{y:.0f}" r="{rad * 3:.0f}" fill="{s.light}" opacity="0.18" filter="url(#blur24)"/>')
            out.append(f'<circle cx="{x:.0f}" cy="{y:.0f}" r="{rad * 0.35:.1f}" fill="{s.accent}"/>')
            # pool of light on the road
            out.append(f'<ellipse cx="{vx + side * 700 * t:.0f}" cy="{HORIZON + (H - HORIZON) * t:.0f}" rx="{220 * t + 10:.0f}" ry="{50 * t + 3:.0f}" '
                       f'fill="{s.light}" opacity="0.10" filter="url(#blur24)"/>')
    return '\n'.join(out)


def trails(r, s, vx):
    out = []
    for k in range(9):
        lane = r.choice([-1.2, -0.9, -0.4, 0.4, 0.9, 1.2])
        color = '#ff3b30' if lane > 0 else '#fff5d6'
        t0, t1 = r.uniform(0.05, 0.25), r.uniform(0.55, 1.0)
        pts = []
        for j in range(24):
            t = t0 + (t1 - t0) * j / 23
            pts.append(f'{vx + lane * 900 * t:.1f},{HORIZON + (H - HORIZON) * t - 18 - 60 * t:.1f}')
        d = 'M' + ' L'.join(pts)
        out.append(f'<path d="{d}" stroke="{color}" stroke-width="{r.uniform(6, 14):.1f}" fill="none" opacity="0.55" filter="url(#blur8)"/>')
        out.append(f'<path d="{d}" stroke="{color}" stroke-width="2.5" fill="none" opacity="0.9"/>')
    return '\n'.join(out)


def landmark(r, s, vx):
    L = s.landmark
    col = mix('#020306', s.sky_glow, 0.18)
    out = []
    if L == 'suspension':
        towers = [vx - 520, vx + 560]
        for tx in towers:
            out.append(f'<rect x="{tx - 16}" y="{HORIZON - 420}" width="32" height="420" fill="{col}"/>')
            out.append(f'<rect x="{tx - 26}" y="{HORIZON - 300}" width="52" height="14" fill="{col}"/>')
            out.append(f'<circle cx="{tx}" cy="{HORIZON - 428}" r="5" fill="#ff3b30"/>')
        a, b = towers
        for off in (0, 10):
            out.append(f'<path d="M{a - 900},{HORIZON - 40} Q{(a - 900 + a) / 2},{HORIZON - 120} {a},{HORIZON - 420 + off} '
                       f'Q{(a + b) / 2},{HORIZON - 60} {b},{HORIZON - 420 + off} Q{(b + b + 900) / 2},{HORIZON - 120} {b + 900},{HORIZON - 40}" '
                       f'stroke="{s.light}" stroke-width="3" fill="none" opacity="0.8"/>')
        for i in range(60):
            x = a + (b - a) * i / 59
            t = (x - a) / (b - a)
            y = HORIZON - 420 + (360) * (1 - (2 * t - 1) ** 2)
            out.append(f'<circle cx="{x:.0f}" cy="{y:.0f}" r="2.5" fill="{s.accent}"/>')
    elif L == 'cablestay':
        tx = vx + 380
        out.append(f'<polygon points="{tx - 40},{HORIZON} {tx - 6},{HORIZON - 520} {tx + 6},{HORIZON - 520} {tx + 40},{HORIZON}" fill="{col}"/>')
        for i in range(16):
            for side in (-1, 1):
                out.append(f'<line x1="{tx}" y1="{HORIZON - 500 + i * 14}" x2="{tx + side * (80 + i * 60)}" y2="{HORIZON - 30}" '
                           f'stroke="{s.light}" stroke-width="1.6" opacity="0.6"/>')
        out.append(f'<circle cx="{tx}" cy="{HORIZON - 528}" r="6" fill="#ff3b30"/>')
    elif L == 'truss':
        y0 = HORIZON - 260
        out.append(f'<rect x="{vx - 1100}" y="{y0}" width="2200" height="12" fill="{s.light}" opacity="0.7"/>')
        for i in range(23):
            x = vx - 1100 + i * 100
            out.append(f'<line x1="{x}" y1="{y0}" x2="{x + 100}" y2="{HORIZON - 40}" stroke="{s.light}" stroke-width="3" opacity="0.55"/>')
            out.append(f'<line x1="{x + 100}" y1="{y0}" x2="{x}" y2="{HORIZON - 40}" stroke="{s.light}" stroke-width="3" opacity="0.55"/>')
    elif L == 'mountain':
        pts = [f'0,{HORIZON}']
        for i in range(0, W + 1, 80):
            pts.append(f'{i},{HORIZON - 160 - 180 * math.sin(i / 520) - r.randint(0, 50)}')
        pts.append(f'{W},{HORIZON}')
        out.append(f'<polygon points="{" ".join(pts)}" fill="{col}"/>')
        for i in range(160):
            out.append(f'<circle cx="{r.randint(0, W)}" cy="{r.randint(40, HORIZON - 340)}" r="{r.uniform(0.8, 2.2):.1f}" fill="#fff" opacity="{r.uniform(0.2, 0.8):.2f}"/>')
    elif L == 'spiral':
        cx, cy = vx - 460, HORIZON - 40
        for k in range(4):
            y, rx, ry = cy - k * 70, 380 - k * 10, 80 - k * 4
            # deck: dark slab, lit edge, lamps along the outer rim
            out.append(f'<ellipse cx="{cx}" cy="{y}" rx="{rx}" ry="{ry}" fill="none" stroke="{col}" stroke-width="26"/>')
            out.append(f'<ellipse cx="{cx}" cy="{y - 12}" rx="{rx}" ry="{ry}" fill="none" stroke="{s.light}" stroke-width="3" opacity="0.8"/>')
            for a in range(0, 360, 24):
                lx = cx + rx * math.cos(math.radians(a)); ly = y - 12 + ry * math.sin(math.radians(a))
                out.append(f'<circle cx="{lx:.0f}" cy="{ly:.0f}" r="3.5" fill="{s.accent}"/>')
        for a in range(0, 360, 45):
            px = cx + 360 * math.cos(math.radians(a)); py = cy + 76 * math.sin(math.radians(a))
            out.append(f'<rect x="{px - 9:.0f}" y="{py - 220:.0f}" width="18" height="{220 + 40:.0f}" fill="{col}"/>')
    elif L == 'canyon':
        for side in (-1, 1):
            for i in range(9):
                x = vx + side * (300 + i * 170)
                h = r.randint(500, 700)
                wbw = 150
                out.append(f'<rect x="{x - wbw / 2:.0f}" y="{HORIZON - h}" width="{wbw}" height="{h}" fill="{mix("#05030a", s.sky_glow, 0.25)}"/>')
                for j in range(r.randint(1, 3)):
                    sy = HORIZON - h + r.randint(40, h - 120)
                    out.append(f'<rect x="{x - 40:.0f}" y="{sy}" width="80" height="{r.randint(60, 140)}" fill="{r.choice([s.light, s.accent, "#5ad7ff"])}" opacity="0.85"/>')
                    out.append(f'<rect x="{x - 60:.0f}" y="{sy - 20}" width="120" height="180" fill="{s.light}" opacity="0.2" filter="url(#blur24)"/>')
    return '\n'.join(out)


def tunnel(r, s, vx):
    """Replaces sky and skyline: a tiled tunnel converging on the vanishing point."""
    out = [f'<rect width="{W}" height="{H}" fill="{mix("#031008", s.sky_glow, 0.4)}"/>']
    for i in range(30, 0, -1):
        t = (i / 30) ** 2.2
        hw, hh = 60 + 1700 * t, 40 + 900 * t
        y = HORIZON - 30
        shade = mix('#020604', s.sky_glow, 0.25 + 0.5 * (1 - t))
        out.append(f'<path d="M{vx - hw},{H + 200} L{vx - hw},{y - hh * 0.4} Q{vx},{y - hh * 1.3} {vx + hw},{y - hh * 0.4} L{vx + hw},{H + 200} Z" '
                   f'fill="{shade}" stroke="{mix(shade, s.light, 0.25)}" stroke-width="{1 + 3 * t:.1f}"/>')
    for i in range(1, 22):
        t = (i / 22) ** 2.2
        y = HORIZON - 30 - (40 + 900 * t) * 0.9
        out.append(f'<rect x="{vx - 8 - 60 * t:.0f}" y="{y:.0f}" width="{16 + 120 * t:.0f}" height="{3 + 10 * t:.0f}" fill="{s.accent}"/>')
        out.append(f'<rect x="{vx - 40 - 200 * t:.0f}" y="{y - 20:.0f}" width="{80 + 400 * t:.0f}" height="{40 + 60 * t:.0f}" fill="{s.light}" opacity="0.18" filter="url(#blur24)"/>')
    return '\n'.join(out)


def car(s):
    """A generic low coupe seen from behind, in the right third; no real model."""
    cx, by = 1840, 1330
    body = (f'M{cx - 360},{by} L{cx - 372},{by - 120} Q{cx - 360},{by - 190} {cx - 280},{by - 205} '
            f'L{cx - 200},{by - 290} Q{cx},{by - 330} {cx + 200},{by - 290} L{cx + 280},{by - 205} '
            f'Q{cx + 360},{by - 190} {cx + 372},{by - 120} L{cx + 360},{by} Z')
    glass = (f'M{cx - 175},{by - 215} L{cx - 130},{by - 280} Q{cx},{by - 305} {cx + 130},{by - 280} L{cx + 175},{by - 215} Z')
    out = [
        f'<ellipse cx="{cx}" cy="{by + 6}" rx="430" ry="40" fill="#000" opacity="0.7" filter="url(#blur24)"/>',
        f'<path d="{body}" fill="url(#carbody)"/>',
        f'<path d="{glass}" fill="#06080c" opacity="0.92"/>',
        f'<path d="M{cx - 380},{by - 196} L{cx + 380},{by - 196}" stroke="{s.car}" stroke-width="12"/>',  # wing
    ]
    for side in (-1, 1):
        x = cx + side * 250
        out.append(f'<rect x="{x - 95}" y="{by - 165}" width="190" height="22" rx="10" fill="#ff2a1f"/>')
        out.append(f'<rect x="{x - 140}" y="{by - 200}" width="280" height="90" fill="#ff2a1f" opacity="0.45" filter="url(#blur24)"/>')
        out.append(f'<rect x="{x - 60}" y="{by - 32}" width="120" height="70" rx="12" fill="#030406"/>')
    out.append(f'<rect x="{cx - 70}" y="{by - 110}" width="140" height="34" rx="4" fill="#d9dde3" opacity="0.85"/>')
    out.append(f'<path d="M{cx - 330},{by - 140} Q{cx},{by - 128} {cx + 330},{by - 140}" stroke="{s.light}" stroke-width="2" fill="none" opacity="0.5"/>')
    return '\n'.join(out)


def rain(r):
    out = []
    for _ in range(900):
        x, y = r.randint(0, W), r.randint(0, H)
        out.append(f'<line x1="{x}" y1="{y}" x2="{x - 6}" y2="{y + 34}" stroke="#cfd8ff" stroke-width="1.2" opacity="{r.uniform(0.08, 0.3):.2f}"/>')
    return '\n'.join(out)


def svg(s):
    r = random.Random(s.seed)
    vx = W // 2 - 160
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">', defs(s)]
    if s.landmark == 'tunnel':
        parts.append(tunnel(r, s, vx))
    else:
        parts.append(f'<rect width="{W}" height="{H}" fill="url(#sky)"/>')
        parts.append(f'<ellipse cx="{vx}" cy="{HORIZON}" rx="1400" ry="260" fill="{s.light}" opacity="0.16" filter="url(#blur60)"/>')
        if s.landmark != 'canyon':
            parts.append(skyline(r, s, HORIZON - 4))
        parts.append(landmark(r, s, vx))
    parts.append(road(r, s, vx))
    parts.append(trails(r, s, vx))
    if s.rain:
        parts.append(rain(r))
    parts.append(car(s))
    parts.append(f'<rect width="{W}" height="{H}" fill="url(#vig)"/>')
    parts.append('</svg>')
    return '\n'.join(parts)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    from PIL import Image, ImageFilter
    for s in SCENES:
        svg_path = os.path.join(out, s.slug + '.svg')
        png_path = os.path.join(out, s.slug + '.png')
        with open(svg_path, 'w') as f:
            f.write(svg(s))
        subprocess.run(['rsvg-convert', '-o', png_path, svg_path], check=True)
        im = Image.open(png_path).convert('RGB')
        # light film grain
        noise = Image.effect_noise(im.size, 18).convert('RGB')
        im = Image.blend(im, noise, 0.035)
        im.resize((1920, 1080), Image.LANCZOS).save(os.path.join(out, s.slug + '.jpg'), quality=86, optimize=True, progressive=True)
        im.resize((512, 288), Image.LANCZOS).save(os.path.join(out, s.slug + '-tile.jpg'), quality=86)
        os.remove(png_path)
        os.remove(svg_path)
        print(s.slug)


if __name__ == '__main__':
    main()
