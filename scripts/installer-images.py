"""NSIS installer images, derived from the logo: sidebar (164×314) and header (150×57).

Usage: python scripts/installer-images.py   (requires Pillow). Writes to src-tauri/installer/.
NSIS requires 24-bit BMPs, without transparency.
"""
import os
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "src-tauri", "installer")
os.makedirs(OUT, exist_ok=True)
logo = Image.open(os.path.join(ROOT, "assets", "logo.png")).convert("RGBA")

FONTS = os.path.join(os.environ.get("WINDIR", "C:/Windows"), "Fonts")
def font(name, size):
    try:
        return ImageFont.truetype(os.path.join(FONTS, name), size)
    except OSError:
        return ImageFont.load_default()

# Brand colours (deep violet background, #7c5cff accent, gold of the pen).
TOP, BOTTOM = (34, 26, 58), (18, 14, 32)
TEXT, SOFT = (244, 241, 255), (176, 166, 214)

# Sidebar: vertical gradient, logo, name and tagline.
W, H = 164, 314
side = Image.new("RGB", (W, H))
d = ImageDraw.Draw(side)
for y in range(H):
    k = y / (H - 1)
    d.line([(0, y), (W, y)], fill=tuple(round(TOP[i] + (BOTTOM[i] - TOP[i]) * k) for i in range(3)))
icon = logo.resize((84, 84), Image.LANCZOS)
side.paste(icon, ((W - 84) // 2, 70), icon)
f_name = font("segoeuib.ttf", 21)
f_tag = font("segoeui.ttf", 11)
def center(draw, y, text, f, fill, spacing=0):
    w = draw.textlength(text, font=f) + spacing * (len(text) - 1)
    x = (W - w) / 2
    for ch in text:
        draw.text((x, y), ch, font=f, fill=fill)
        x += draw.textlength(ch, font=f) + spacing
center(d, 172, "ELORIA", f_name, TEXT, spacing=4)
center(d, 204, "Your knowledge,", f_tag, SOFT)
center(d, 219, "beautifully connected.", f_tag, SOFT)
side.save(os.path.join(OUT, "sidebar.bmp"))

# Header: white background (NSIS shows it on the right of the header), small logo.
head = Image.new("RGB", (150, 57), (255, 255, 255))
small = logo.resize((44, 44), Image.LANCZOS)
head.paste(small, (150 - 44 - 8, (57 - 44) // 2), small)
head.save(os.path.join(OUT, "header.bmp"))
print("written:", os.path.join(OUT, "sidebar.bmp"), "and header.bmp")
