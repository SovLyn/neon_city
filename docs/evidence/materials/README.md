# CC0 surface-material integration

Poly Haven Concrete Wall 001, Worn Asphalt and Rusty Metal 05 replace flat surface shading on selected building shells, pavement and metal fittings. Asset provenance and CC0 links are in `assets/materials/manifest.json` and `assets/materials/README.md`.

Two bundled 1536×1023 mip strips (4,209,652 bytes total) upload into sRGB color and linear normal/roughness texture arrays. Each contains three 512px base layers and ten mip levels. Color mip filtering is linear-light; normal mip vectors are renormalized. All 11 asset PNG hashes were checked, and offline rebuild produced byte-identical runtime PNGs.

Raster and BVH hit shading call the same world-projected surface function. Fixed source-average luminance normalization preserves the existing palette. Wet asphalt roughness is modulated around its wet baseline rather than saturated to the source's dry roughness. Water, glass, signs, windows, car paint and drones are excluded. This does not add displacement or parallax; dominant-axis projection can show orientation transitions on curved geometry. Reflected-hit mip selection uses an approximate ray footprint, not full ray differentials.

## Verification

- 45 Rust tests passed, one additional benchmark ignored; formatting, WASM check and release Trunk build passed.
- 12 browser-check cases passed, including matched textures-on/off views and fatal handling for a missing material image. The normal cases had no console errors.
- Manually inspected facade and street comparisons with bloom disabled; the supplied concrete joints, metal wear and asphalt grain are visible, without replacing neon text or puddle reflections.
- Latest local 10s warmup + 60s sampling per tier at 1920×1080 output / 1280×720 internal: Balanced median 24.6ms / P95 32.7ms / P99 32.8ms; Performance median 8.3ms / P95 16.4ms / P99 18.7ms. These are observed RAF intervals, NOT GPU query timings. Values vary between runs and are not portable hardware guarantees. Full moving-route and GPU-memory acceptance remain outstanding.

`street.png` / `street-off.png`: default camera, time=3, paused accumulation, bloom=0; second also has textures=0. `no-bloom.png` / `textures-off.png`: same comparison from showcase camera. Debug images use the showcase camera and the corresponding debug parameter. The JSON reports contain exact query strings and recorded metrics.

Implementation was delegated to lower-cost 6-luna subagents for source acquisition/preparation, loader/classification and renderer/shader work. Parent integration review corrected normalization and tangent-basis defects before browser validation.
