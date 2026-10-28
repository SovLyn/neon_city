# Neon City — art direction and visual acceptance

## Status and measurement conventions

This document specifies final scene targets, **not proof they have been achieved**. The delivered asset sheet/atlas checks establish only glyph coverage, legal provenance, and valid UV regions. Real WebGPU captures, timings, and 30-second recordings remain integration acceptance work.

Default seed: `0x4E454F4E`. Right-handed world, Y up, metres; street along Z, total length 120 m, width about 24 m, six buildings per side. Camera looks along local −Z. Proposed staging uses street centre `(0,0,0)` and endpoints `z=±60`; if the generator uses another origin, translate all following cameras/fixtures together and record the transform. Output 1920×1080, internal 1280×720, reflection 640×360 (performance 320×180). Every capture records all three dimensions, quality, seed, simulation time, browser/GPU, exposure and debug toggles.

Visual hierarchy: pink/magenta shop fronts dominate the midground, cyan punctuates depth and traffic, violet accents upper holograms. Keep pavement and equipment visible at fixed exposure. No Bloom/atmosphere setting may excuse unreadable signs, absent reflections or empty architecture.

## Display and diagnostic cameras

Use 55° vertical field of view, near 0.1 m, far 220 m, unless recorded otherwise. Explicit `look_at` avoids Euler-order ambiguity.

| Name | Position → look-at | Evidence purpose |
|---|---|---|
| Hero | `(0,1.65,46)` → `(0,3,-18)` | Wet/damp/dry ground simultaneously, both street sides, readable pink/cyan hierarchy; timestamp 12 s |
| Sign inspection | `(1,1.7,22)` → `(10,3.0,18)` | Put a 3.2×1.6 m primary sign at the target, facing street; distance ≈10 m. Check Chinese/Japanese/English and all four classes without Bloom |
| Surface comparison | `(0,1.35,24)` → `(2,0,12)` | Puddle adjacent to damp asphalt and dry raised threshold; roughness/normal views and consistent reflected sign |
| Off-screen reflection | `(0,1.1,18)` → `(0,0,2)` | Place sign behind the viewing direction at `(8,4,28)` facing the puddle. Confirm its geometry is outside primary frustum but visible via BVH reflection; do not assume this pose passes without checking frusta |
| Atmosphere | `(-3,1.6,35)` → `(2,2,-30)` | Three steam vents at different phases, depth-occluded low fog and two remote beams |
| Dressing | `(-2,1.8,8)` → `(-10,4,-4)` | Disable all emission: façade equipment, pipes, runoff and story/volume variation remain readable |

The sign aspect ratio is about 2.1:1 after UV gutter exclusion. At 10 m a 1.6 m sign with 60 px glyphs inside a 232 px high region yields around 40 output pixels of text height, but final projection and angle must be measured. At 8–15 m require **≥18 output pixels glyph height**, even when Bloom is off. Do not scale signs into thin strips to make the frame appear denser.

Camera route for a 30-second motion review: hold Hero for 5 s, dolly from `(0,1.65,46)` to `(0,1.65,8)` over 15 s, hold for 5 s, then pan 45° over 5 s. Capture a separate rapid camera-cut/disocclusion check. A repeatable fixed-60-Hz simulation is required; no manual camera run substitutes for timing evidence.

## A. Wet-surface layering

- [ ] Main camera's visible pavement: puddles **15–30%**, damp asphalt **45–65%**, remainder dry edges. This is image coverage, not the count of material IDs. Save a surface-class mask.
- [ ] Puddle roughness **0.03–0.12**; damp **0.20–0.45**; dry **0.65–0.90**. Save roughness and normals separately. Avoid assigning the entire street a single mirror material.
- [ ] Puddles follow low points/drainage. Door thresholds/raised curbs stay drier. Damp asphalt retains fine granular normals; puddles use smooth long-wave perturbation, not crushed noise.
- [ ] Same sign is recognizable in a puddle, blurred over damp asphalt and weak on a dry edge. Static camera reflection shows mild local ripples rather than synchronized whole-road wobble.
- [ ] Ripple births are spatially random, restricted to puddle mask, lifetime **0.6–1.8 s**, independent phase and decaying amplitude.
- [ ] Off-screen sign appears in both puddle and glass reflections. Submit primary-frustum + traced-hit/debug views to prove real world geometry tracing, not SSR or a static environment image.

## B. Four light-source classes and readable content

Delivered catalog has **24 signs, six each** of the following classes. Class encoding is `Tube=1`, `Led=2`, `Hologram=3`, `FaultText=4`. Region IDs are permanent, independent of scene instance/material IDs. See `../assets/README.md` for the exact sampling/ABI handoff.

| Class | Catalog intensity range | Shape/motion target | Bloom starting point (internal pixels, not frozen shader API) |
|---|---:|---|---|
| Tube | 9–12 | Tight luminous letter/core and thin rounded border. Rare 2–9 s irregular flicker, short extinction, not constant sine pulsing | Tight 2–6 px core halo, moderate 10–20 px outer glow |
| Led | 4.5–6 | Stable block lettering, subtle pixel grid, content change every 3–7 s | Restrained 2–5 px, avoid washing out grid/text |
| Hologram | 3.5–4.5 | Broken brackets, transparent coverage, scan/fault events 0.5–2 Hz, separate projection base | Wider 8–24 px with low gain and limited energy |
| FaultText | 6–7 | Legible base word, brief channel offsets/scanlines/local afterimage; independent faults, not full-time corruption | 2–8 px; preserve glyph counters and baseline |

Intensity values are relative linear HDR radiance multipliers, not physical cd/m². Adjust global exposure once under Hero, then lock it for A/B comparisons. Do not claim these constants guarantee final luminance on every display.

- [ ] At least 18 actual scene signs, ≥3 per class; prefer all 24 assets. Text includes `夜市`, `ラーメン`, `OPEN 24H` and other Chinese/Japanese/English catalog entries.
- [ ] At sign camera, ≥18 output pixels primary glyph height and no missing glyph boxes. Save no-Bloom, atlas-UV and material/bloom-class views.
- [ ] RGB separation on faulty signs is **local to that source**, typically 1–3 internal pixels for <150 ms; keep intact base coverage. Optional whole-screen chromatic aberration is separately constrained below.
- [ ] Static atlas only supplies coverage/layout and subtle ornamentation. Shader effects must actually differ by class. Tube letters are currently solid raster letterforms, not a geometric tube simulation; refine shader/core treatment if necessary.
- [ ] Primary and reflected signs sample identical regions and the same time-dependent material state. Baked contact sheet is not evidence of runtime animation.

## C. Atmosphere: three different spatial/time scales

- [ ] Low fog concentrated in **0–1.5 m** band, density decays with height; drift target **0.08–0.25 m/s**. It should create distance separation, not replace street geometry.
- [ ] ≥3 steam vents with independent **4–13 s** emission intervals. Stage 1 (0–0.5 s): directed upward jet; stage 2 (0.5–2 s): buoyant rise/side spread; stage 3 (2–4 s): dilution and soft extinction. These are art starting values, tune without dropping phases.
- [ ] Steam rises about **0.5–1.5 m/s**, then slows/diffuses. Noise perturbs velocity along the plume; do not distribute particles uniformly in a sphere. Vent directions correspond to visible pipes/grates.
- [ ] ≥2 far beams beyond the focal shops: visible thickness, distance falloff, depth occlusion, slower/no movement compared to steam. A bright unoccluded rectangle is not a volume.
- [ ] Low fog, steam and beams have separately adjustable density/speed/debug isolation. Save each alone plus combined.
- [ ] Atmosphere respects opaque depth, with depth-guided upsampling if half resolution. Inspect building silhouettes for leaks. Near equipment and glyphs remain legible.

## D. Human traces and density

- [ ] Exactly 12 buildings over about 120 m; ≥3 distinct massing/setback patterns, not twelve equal-height boxes.
- [ ] ≥4 categories of nonemissive detail **per building**, and ≥240 small instances or equivalent merged geometry/decal coverage overall.
- [ ] Global categories include AC condensers, pipes, supported cables, vents, drains, runoff stains, stickers/posters, vending machines and refuse bins.
- [ ] Condensers sit near windows/equipment floors; vertical pipes follow wall corners; cables connect supports; stains descend from drains; refuse/vending cluster near doors, not in traffic lanes.
- [ ] Limited variations in scale, paint, rotation and wear break exact repetition without random floating clutter. Record per-building counts/category coverage.
- [ ] Emission-off view still conveys a lived-in district. No reliance on luminous rectangles to conceal featureless façades.

## E. Deliberately offset motion

All streams derive from scene seed + system ID + instance ID. Adding a vehicle must not alter all sign flicker events. Simulate fixed **1/60 s**, interpolate rendering, and discard pause backlog.

| System | Acceptance cadence |
|---|---|
| Rain | Continuous high-frequency births, randomized ages/phases; no synchronized sheet resets |
| Tube/other neon | Independent **2–9 s** irregular event intervals; short, nonsinusoidal faults |
| Holograms | Local **0.5–2 Hz** scan/fault; content **3–7 s**, not full-screen flashes |
| Vehicles | **5–12 m/s**, spawn intervals **1.5–4 s**, independent lane offsets |
| Drones | Smooth hover noise, **3–11 s** behavior changes, independent indicator timing |
| Steam | Per-vent **4–13 s** intervals, not coupled to sign faults |
| Ripples | Independent random spatial births and **0.6–1.8 s** age decay |

- [ ] In a continuous 30 s recording no global periodic in-phase event is visible.
- [ ] Vehicle geometry, ray proxy and headlights share transforms. Deliberate headlight trail **0.1–0.25 s** is an explicit effect, not temporal-filter ghosting.
- [ ] After a moving reflected car leaves, wrong history is visibly gone within roughly **100 ms**. Dynamic hits accumulate only **2–4** effective frames; static reflections about **16** max.
- [ ] Separate motion/previous-depth/temporal-rejection views show camera and instance motion. Teleport, resize, pause resume and quality changes clear history.

## F. Color, exposure and postprocessing

- [ ] Perceived emissive weight roughly **55–65% pink/magenta**, **25–35% cyan/blue**, **~10% violet**. Delivered asset palette is 14 pink, 7 cyan, 3 violet, a starting point only: count ≠ image-energy weighting. Frame composition and intensity must establish the target.
- [ ] Preserve dark texture separation on road, walls and equipment; avoid crushing everything to black or tinting all shadows magenta. Save emission-off and fixed-exposure no-Bloom views.
- [ ] Source-class-specific Bloom extraction/strength/radius; direct signs, **traced reflections and transparent holograms all participate**. G2 class alone cannot classify all post-lighting energy; composite a dedicated seed.
- [ ] Tube core stays colored/tight, LED remains legible, hologram gain stays limited, faulty text remains decipherable. Avoid large white clipped patches and full-frame haze.
- [ ] Optional whole-screen chromatic aberration ≤**1 output pixel at 1080p**, disable toggle mandatory. This is different from local faulty-source RGB displacement.
- [ ] Optional vignette corner dimming ≤**15%**, disable toggle mandatory. No film grain/strong aberration used to disguise low-res noise or ghosting.

## Required final evidence and performance

For A–F save **normal, no-Bloom, and relevant debug** captures at recorded seed/time/camera/exposure. Also save no-atmosphere and emission-off images, a 30 s animation recording, off-screen-reflection fixture, and close-up glyph view. Do not mark a checkbox from a contact sheet alone.

On RTX 3060-class reference desktop GPU (record exact GPU/driver/OS/browser), warm up 10 s then capture the fixed route for 60 s: median/P95/P99 frame time, output/internal/reflection sizes, per-pass GPU time if supported. **P95 ≤33.3 ms** is the target at 1080p output/720p internal. Lack of timestamp queries must be labeled; CPU submission time is not GPU time. Performance mode can reduce reflection from 640×360 to 320×180, but cannot remove real BVH reflection, AO, surface classes or readable text. Stable 10-minute run must show no growing GPU resources or validation errors.

## Asset verification already performed

- Atlas opens as RGBA8 **2048×1536**, 24 disjoint in-bounds regions.
- All **71 unique codepoints** (including space) exist in the bundled font cmap; no `.notdef` mappings.
- All UVs within `[0,1]`, match half-texel-inset pixel rectangles; all glyph/ornament bounds stay inside region gutters.
- OFL copyright/license retained and hashes checked; subset renamed to avoid reserved-name ambiguity.
- Offline rebuild and `--verify` pass; repeated PNG SHA-256 matches `5be1e260920585ab597018733eb263e36b11ae7443edf5e47b5b9a671bf57fa6`.
- Manually inspected the generated no-Bloom tinted contact sheet: Chinese/Japanese/English glyphs readable, no clipping/tofu at asset resolution. Final projected readability is still unverified.
- Intentional deviation: **simple RGBA coverage atlas, not MSDF**. Detailed integration and license instructions live in `assets/README.md`.
