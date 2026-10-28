#!/usr/bin/env python3
"""Offline RGBA sign atlas. Python 3.13 + Pillow==12.1.1 + fonttools==4.61.1.

    py -3.13 tools/build_sign_atlas.py --bootstrap-font  # one-time network
    py -3.13 tools/build_sign_atlas.py                   # offline rebuild
    py -3.13 tools/build_sign_atlas.py --verify          # no writes

Only bootstrap downloads a font; normal builds use the committed OFL subset.
No system fonts or browser font rendering are used. This is NOT an MSDF.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import math
from pathlib import Path
import urllib.request
from collections import Counter

from PIL import Image, ImageChops, ImageDraw, ImageFont, __version__ as PILLOW_VERSION
from fontTools import subset, __version__ as FONTTOOLS_VERSION
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"
FONT = ASSETS / "fonts/neon-city-signs-subset.ttf"
ATLAS = ASSETS / "fonts/signs-rgba.png"
MAPPING = ASSETS / "fonts/signs-rgba.json"
CATALOG = ASSETS / "signs/catalog.json"
KINDS = {"Tube": 1, "Led": 2, "Hologram": 3, "FaultText": 4}
CELL_W, CELL_H, COLS, PAD = 512, 256, 4, 12
UPSTREAM = "https://raw.githubusercontent.com/google/fonts/main/ofl/notosansjp/"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load_signs() -> list[dict]:
    signs = json.loads(CATALOG.read_text(encoding="utf-8"))["signs"]
    assert len(signs) >= 18
    ids = [s["region_id"] for s in signs]
    assert len(ids) == len(set(ids)), "duplicate region ID"
    assert len({s["key"] for s in signs}) == len(signs)
    assert all(isinstance(i, int) and i >= 0 for i in ids)
    assert all(s["kind_id"] == KINDS[s["kind"]] for s in signs)
    assert all(sum(s["kind"] == k for s in signs) >= 3 for k in KINDS)
    return sorted(signs, key=lambda s: s["region_id"])


def charset(signs: list[dict]) -> str:
    return "".join(sorted(set("".join(s["text"] for s in signs))))


def bootstrap_font(signs: list[dict]) -> None:
    """Explicit online operation; provenance hashes bind mutable upstream URLs."""
    font_url = UPSTREAM + "NotoSansJP%5Bwght%5D.ttf"
    license_url = UPSTREAM + "OFL.txt"
    raw = urllib.request.urlopen(font_url, timeout=120).read()
    license_data = urllib.request.urlopen(license_url, timeout=60).read()
    assert b"SIL OPEN FONT LICENSE Version 1.1" in license_data
    assert b"Copyright" in license_data
    font = TTFont(io.BytesIO(raw), recalcTimestamp=False)
    font = instantiateVariableFont(font, {"wght": 600}, inplace=True)
    opts = subset.Options()
    opts.recalc_timestamp = False
    opts.name_IDs = [0, 1, 2, 3, 4, 5, 6, 13, 14]
    opts.name_legacy = True
    opts.name_languages = [0x409]
    sub = subset.Subsetter(options=opts)
    sub.populate(text=charset(signs))
    sub.subset(font)
    # Modified font gets a distinct family, avoiding reserved-name ambiguity.
    names = {1: "Neon City Signs", 2: "SemiBold", 3: "NeonCitySigns-SemiBold-Subset1",
             4: "Neon City Signs SemiBold", 6: "NeonCitySigns-SemiBold"}
    for record in font["name"].names:
        if record.nameID in names:
            record.string = names[record.nameID].encode(record.getEncoding())
    FONT.parent.mkdir(parents=True, exist_ok=True)
    font.save(FONT)
    licenses = ASSETS / "licenses"
    licenses.mkdir(parents=True, exist_ok=True)
    (licenses / "NotoSansJP-OFL.txt").write_bytes(license_data)
    provenance = {
        "upstream_family": "Noto Sans JP", "modified_family": "Neon City Signs",
        "font_url": font_url, "license_url": license_url,
        "source_font_sha256": sha(raw), "license_sha256": sha(license_data),
        "subset_font_sha256": sha(FONT.read_bytes()), "license": "OFL-1.1",
        "modification": "Instantiate wght=600; subset catalog characters; rename family.",
        "characters": charset(signs), "fonttools": FONTTOOLS_VERSION,
        "note": "URLs are mutable. Offline rebuild uses the committed, hash-verified subset, not upstream."
    }
    (licenses / "font-provenance.json").write_text(
        json.dumps(provenance, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def check_font(signs: list[dict]) -> list[int]:
    with TTFont(FONT) as font:
        cmap = font.getBestCmap()
        missing = [ord(c) for c in charset(signs) if ord(c) not in cmap or cmap[ord(c)] == ".notdef"]
    assert not missing, f"Missing font characters: {missing}"
    provenance = json.loads((ASSETS / "licenses/font-provenance.json").read_text(encoding="utf-8"))
    assert sha(FONT.read_bytes()) == provenance["subset_font_sha256"]
    assert sha((ASSETS / "licenses/NotoSansJP-OFL.txt").read_bytes()) == provenance["license_sha256"]
    return missing


def draw_cell(sign: dict) -> tuple[Image.Image, dict]:
    mask = Image.new("L", (CELL_W, CELL_H), 0)
    draw = ImageDraw.Draw(mask)
    kind = sign["kind"]
    # All ornamentation stays well clear of the text. Runtime shaders supply
    # actual LED pixels, temporal flicker, hologram scan and CRT RGB offsets.
    if kind == "Tube":
        draw.rounded_rectangle((24, 38, 487, 217), radius=20, outline=220, width=3)
        draw.line((52, 201, 136, 201), fill=255, width=3)
    elif kind == "Led":
        draw.rectangle((23, 36, 488, 219), outline=150, width=2)
        for y in range(46, 216, 8):
            for x in range(32, 488, 8):
                draw.point((x, y), fill=28)
    elif kind == "Hologram":
        for x in (25, 452):
            draw.line((x, 66, x, 40, x + 34, 40), fill=190, width=2)
            draw.line((x, 190, x, 216, x + 34, 216), fill=190, width=2)
        draw.line((60, 202, 450, 202), fill=72, width=1)
        draw.line((115, 208, 395, 208), fill=40, width=1)
    else:
        draw.rectangle((24, 38, 487, 217), outline=120, width=2)
        draw.line((42, 196, 124, 196), fill=150, width=2)
        draw.line((377, 59, 470, 59), fill=170, width=2)
    size = 68
    while True:
        font = ImageFont.truetype(str(FONT), size, layout_engine=ImageFont.Layout.BASIC)
        box = draw.textbbox((0, 0), sign["text"], font=font)
        if box[2] - box[0] <= 412 and box[3] - box[1] <= 106:
            break
        size -= 1
        assert size >= 18
    x = (CELL_W - (box[2] - box[0])) // 2 - box[0]
    y = (CELL_H - (box[3] - box[1])) // 2 - box[1]
    text_mask = Image.new("L", mask.size)
    ImageDraw.Draw(text_mask).text((x, y), sign["text"], font=font, fill=255)
    mask = ImageChops.lighter(mask, text_mask)
    # White RGB even under zero alpha prevents dark linear-filter fringes.
    rgba = Image.new("RGBA", mask.size, (255, 255, 255, 0))
    rgba.putalpha(mask)
    return rgba, {"font_size_px": size, "text_ink_rect_px": [x + box[0], y + box[1], x + box[2], y + box[3]]}


def build(signs: list[dict]) -> None:
    check_font(signs)
    rows = max(s["region_id"] for s in signs) // COLS + 1
    width, height = COLS * CELL_W, rows * CELL_H
    image = Image.new("RGBA", (width, height), (255, 255, 255, 0))
    preview = Image.new("RGB", image.size, (10, 13, 22))
    regions = []
    for s in signs:
        x, y = (s["region_id"] % COLS) * CELL_W, (s["region_id"] // COLS) * CELL_H
        cell, metrics = draw_cell(s)
        image.paste(cell, (x, y))
        tint = tuple(round(max(0, min(1, c)) ** (1 / 2.2) * 255) for c in s["color_linear"])
        colored = Image.new("RGB", cell.size, tint)
        preview.paste(colored, (x, y), cell.getchannel("A"))
        # Half-texel inset samples texel centers; excludes transparent gutters.
        rect = [x + PAD, y + PAD, x + CELL_W - PAD, y + CELL_H - PAD]
        uv = [(rect[0] + .5) / width, (rect[1] + .5) / height,
              (rect[2] - .5) / width, (rect[3] - .5) / height]
        regions.append({"region_id": s["region_id"], "key": s["key"], "text": s["text"],
                        "kind": s["kind"], "kind_id": s["kind_id"], "rect_px": rect,
                        "uv_rect": uv, "cell_rect_px": [x, y, x + CELL_W, y + CELL_H], **metrics})
    image.save(ATLAS, optimize=False, compress_level=9)
    preview.save(ASSETS / "signs/contact-sheet.png", compress_level=9)
    mapping = {"schema_version": 1, "encoding": "rgba8unorm-white-rgb-straight-alpha-coverage",
               "not_msdf": True, "image": "signs-rgba.png", "width": width, "height": height,
               "origin": "top-left", "rect_px_convention": "x0,y0,x1,y1; exclusive max",
               "uv_convention": "texel-center-inset; local UV 0..1 maps by mix(uv.xy,uv.zw,local_uv)",
               "sampling": "linear min/mag; clamp-to-edge; no automatic atlas mipmaps",
               "shader_usage": "mask=textureSample(atlas,sampler,uv).a; emission=color_linear*intensity*mask",
               "regions": regions, "characters": charset(signs),
               "font_sha256": sha(FONT.read_bytes()), "catalog_sha256": sha(CATALOG.read_bytes()),
               "atlas_sha256": sha(ATLAS.read_bytes()), "pillow": PILLOW_VERSION,
               "fonttools": FONTTOOLS_VERSION}
    MAPPING.write_text(json.dumps(mapping, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def verify(signs: list[dict]) -> dict:
    check_font(signs)
    m = json.loads(MAPPING.read_text(encoding="utf-8"))
    image = Image.open(ATLAS)
    assert image.mode == "RGBA"
    w, h = image.size
    assert (w, h) == (m["width"], m["height"])
    assert sha(ATLAS.read_bytes()) == m["atlas_sha256"]
    assert sha(CATALOG.read_bytes()) == m["catalog_sha256"]
    assert sha(FONT.read_bytes()) == m["font_sha256"]
    assert m["characters"] == charset(signs)
    assert len(m["regions"]) == len(signs)
    seen = []
    for s, r in zip(signs, m["regions"]):
        assert (s["region_id"], s["key"], s["text"], s["kind"], s["kind_id"]) == (r["region_id"], r["key"], r["text"], r["kind"], r["kind_id"])
        x0, y0, x1, y1 = r["rect_px"]
        assert 0 <= x0 < x1 <= w and 0 <= y0 < y1 <= h
        assert all(x1 <= a or x0 >= c or y1 <= b or y0 >= d for a, b, c, d in seen)
        seen.append((x0, y0, x1, y1))
        uv = r["uv_rect"]
        expected = [(x0 + .5) / w, (y0 + .5) / h, (x1 - .5) / w, (y1 - .5) / h]
        assert uv == expected
        assert all(0 <= q <= 1 for q in uv)
        bounds = image.getchannel("A").crop((x0, y0, x1, y1)).getbbox()
        assert bounds is not None
        a, b, c, d = r["text_ink_rect_px"]
        assert PAD <= a < c <= CELL_W - PAD and PAD <= b < d <= CELL_H - PAD
        # No glyphs or decoration can bleed beyond the declared region.
        cell = image.getchannel("A").crop(r["cell_rect_px"])
        left, top, right, bottom = cell.getbbox()
        assert PAD <= left < right <= CELL_W - PAD and PAD <= top < bottom <= CELL_H - PAD
    return {"result": "PASS", "size": [w, h], "format": "RGBA8",
            "regions": len(signs), "kinds": dict(Counter(s["kind"] for s in signs)),
            "unique_characters": len(charset(signs)), "missing_characters": [],
            "region_bounds": "PASS", "region_overlap": "none", "uv_bounds": "PASS",
            "font_license_and_hash": "PASS", "atlas_sha256": sha(ATLAS.read_bytes()),
            "glyph_coverage_note": "cmap presence checked for every codepoint; not a claim of screenshot legibility at arbitrary distance"}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bootstrap-font", action="store_true", help="Explicit network download, subset and license capture")
    parser.add_argument("--verify", action="store_true", help="Verify existing files without writing")
    args = parser.parse_args()
    signs = load_signs()
    if args.bootstrap_font:
        bootstrap_font(signs)
    if not args.verify:
        build(signs)
    report = verify(signs)
    if not args.verify:
        (ASSETS / "signs/verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=True, indent=2))


if __name__ == "__main__":
    main()
