# Offline sign assets

## Files and loading contract

- `fonts/signs-rgba.png`: actual, fully laid-out sign coverage texture, **RGBA8, 2048×1536, not MSDF/SDF**. Do not use an MSDF median/distance shader.
- `fonts/signs-rgba.json`: 24 permanent `region_id` → UV rectangles, text bounds, atlas/font/catalog hashes. IDs 0–23.
- `signs/catalog.json`: permanent keys/IDs, readable text, `Tube=1`, `Led=2`, `Hologram=3`, `FaultText=4`, linear emission colors, HDR intensity multipliers and independent seeds. Six of each class.
- `signs/contact-sheet.png`: tinted, no-Bloom review sheet only. Not the runtime texture, not an in-engine screenshot, and not physically toned HDR evidence.
- `fonts/neon-city-signs-subset.ttf`: renamed OFL subset of Noto Sans JP SemiBold containing every catalog codepoint. It is a build asset, not downloaded or installed at runtime.
- `licenses/NotoSansJP-OFL.txt` and `licenses/font-provenance.json`: original copyright/license, upstream URLs, source/derived hashes and modifications.
- `signs/verification.json`: machine-verifiable atlas checks, not a claim that final scene visual acceptance is complete.

Load the atlas as `Rgba8Unorm` (linear coverage, not sRGB). RGB is white, including transparent texels. Alpha is antialiased coverage. Sample with linear filtering, clamp-to-edge, **no automatic mipmaps**: a conventional full-atlas mip chain would eventually mix neighbors. Region `rect_px` is exclusive-max; `uv_rect` already has a half-texel inset and top-left origin. Map local UV by `mix(uv_rect.xy, uv_rect.zw, local_uv)`. Use `sample.a * color_linear * intensity` for emissive radiance. The page-relative asset prefix must respect the Trunk document base; never fetch `/assets/...` from the domain root.

The static atlas does not animate. Runtime shaders must apply type-specific temporal behavior, and use the same `region_id`, animation state and UV convention for primary and traced hits. LED grid hints and hologram brackets are baked; animated scanlines, RGB offsets, content switching and flicker remain runtime responsibilities. Tube glyphs are readable solid letterforms rather than simulated glass tubing. Do not label the static texture by itself as a finished four-effect shader.

## Offline regeneration

External build dependencies (not application dependencies): Python 3.13, `Pillow==12.1.1`, `fonttools==4.61.1`. Tested with Python 3.13 on Windows. Use an isolated environment if these packages are shared:

```sh
python -m venv .venv
# Activate using your platform's normal venv activation command.
python -m pip install Pillow==12.1.1 fonttools==4.61.1
python tools/build_sign_atlas.py
python tools/build_sign_atlas.py --verify
```

Builds are offline after packages are installed and use the committed subset. Two local builds produced identical PNG SHA-256 `5be1e260920585ab597018733eb263e36b11ae7443edf5e47b5b9a671bf57fa6`. Pillow/FreeType platform changes can change raster pixels; the generated metadata records package versions and content hashes. No claim of cross-platform byte-identical rasterization is made.

To expand the repertoire, edit the catalog, then explicitly run `python tools/build_sign_atlas.py --bootstrap-font`. Only that flag accesses the network. It downloads the original Noto Sans JP variable font and its OFL, instantiates weight 600, subsets characters, renames the family to **Neon City Signs**, captures provenance, then rebuilds. Upstream URLs are mutable: compare source hashes if bootstrapping again. Existing regions must not be renumbered or removed merely to reorder signs. New entries receive new IDs; the atlas height expands when needed. New characters without a regenerated subset fail validation instead of silently rendering tofu.

## Redistribution

Upstream: Google Fonts `ofl/notosansjp`, Noto Sans JP. Original copyright: **Copyright 2014–2021 Adobe (http://www.adobe.com/), with Reserved Font Name 'Source'**. Font license: **SIL Open Font License 1.1**, included verbatim. The derived font is renamed, retains original copyright/license metadata, and remains OFL-1.1. Bundle the license and copyright with any redistribution of the font; do not sell the font alone. The original download SHA-256 and all derivative changes are recorded in provenance. OFL permits font embedding, modification and redistribution under these terms; it does not require rendered documents/images to use OFL. Original catalog, layout code and rendered graphics are project-authored contributions; no separate external image assets or CDN fonts are used.

## Intentional plan deviation

The architectural proposal named `signs-msdf.*`. These assets deliberately use **`signs-rgba.*`** instead: real grayscale-coverage PNG lettering is inspectable, directly sampleable in raster and BVH-hit shaders, reproducible with a small bundled font subset, and does not falsely claim an unavailable MSDF generator. At extreme magnification it has raster limits. Preserve at least 18 output pixels of primary glyph height at the named inspection camera; an actual MSDF upgrade requires a new declared asset encoding and shader path.
