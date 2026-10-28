# Acceptance and verification

This document separates checks that can run in CI from evidence that requires a real WebGPU browser and GPU. A successful Rust build, unit test, WGSL validation, or software-rendered screenshot is **not** proof of GPU rendering correctness, image quality, or performance.

## CI checks (not GPU evidence)

The Pages workflow validates formatting, host tests, and compilation for `wasm32-unknown-unknown`, then builds the Trunk site with the Pages base path. These checks establish that code compiles and the deployable artifact can be produced; they do not establish that a browser can initialize WebGPU or that the renderer meets visual or frame-time targets. Pull requests and manual `workflow_dispatch` runs build with `/neon_city/` and do not call Pages APIs or deploy. Only a push to `master` configures Pages and deploys; its public URL is the configured Pages base path, or `/` when Pages reports a root site.

CI and pre-release checks must additionally cover, as they become available:

- Rust CPU tests for deterministic scene generation, ABI sizes/offsets, BVH-vs-brute-force intersections, and boundary/degenerate inputs.
- WGSL parsing/validation and shader interface consistency.
- Asset URL/path checks against a project-site base path (`/neon_city/`), including HTML, JS/WASM glue, font/sign atlases, and manifests. No asset fetch may assume the domain root (for example, `/assets/...`).
- The workflow performs a basic generated-HTML check for `script src` and resource `link` tags (`stylesheet`, `modulepreload`, `preload`, `prefetch`): project-site absolute paths must start with the configured public URL, relative paths must not escape it, and mapped files must exist under `dist/`. The `<base>` tag and ordinary anchors are excluded. It also checks that at least one JS and WASM file exists. This is not exhaustive validation of runtime fetches, CSS-referenced assets, fonts, or all nested resources.
- Deployment smoke checks for HTTPS, expected JS/WASM MIME types, and successful HTTP retrieval of the HTML, WASM, and atlas assets.

These automated checks remain complementary to browser/GPU verification, not a substitute for it.

## Browser and GPU evidence required for release

Record the exact GPU model, driver, OS, browser and version, power mode, quality tier, output/internal/reflection resolutions, and commit. Keep raw captures/logs with the report in `docs/benchmarks/` when that directory is introduced. Identify any unavailable GPU timestamp-query measurements; do not label CPU submission or wall-clock frame times as GPU pass timings.

### A. Wet surfaces

- [ ] A fixed showcase view visibly contains puddles (approximately 15–30% of visible street area), wet asphalt (45–65%), and comparatively dry edges.
- [ ] Puddles, wet asphalt, and dry areas use distinct roughness/normal response; the practical target roughness ranges are 0.03–0.12, 0.2–0.45, and 0.65–0.9 respectively.
- [ ] Water collects plausibly near low points/drains; puddle normals are broad and calm, wet asphalt retains fine grain, and dry edges do not become broad mirrors.
- [ ] Ripples occur only in puddles, at independent randomized positions, with 0.6–1.8 s lifetimes and visible decay. No whole-road in-phase wobble or regular ripple grid.
- [ ] A recognizable sign reflection remains legible in a puddle, becomes blurrier on wet asphalt, and weakens substantially on dry ground.
- [ ] Show a sign moved outside the camera view that is still reflected in a puddle. This is the required evidence that reflection is not SSR; include the relevant BVH/debug view.
- [ ] Provide normal, Bloom-disabled, and relevant material/debug captures.

### B. Neon source types and legibility

- [ ] Bare tubes, LED displays, holographic ads, and faulted/CRT-like text have visibly distinct structure, radiance, and timing. Faulted text has restrained RGB separation and scan-line behavior while retaining its main glyphs.
- [ ] At least 18 signs are present, with at least three examples of each source type.
- [ ] Signs contain readable Chinese, Japanese, and English (including examples such as “夜市”, “ラーメン”, and “OPEN 24H”); glyphs are bundled locally, with redistribution licensing recorded.
- [ ] From the fixed 8–15 m showcase distance at 1920×1080 output, primary text is approximately 18 px high or larger and readable with Bloom disabled.
- [ ] Rasterized signs and ray-hit sign emission share the same atlas region and current animation state.
- [ ] Provide normal, Bloom-disabled, and source-class/debug captures; do not use overexposure as evidence of legibility.

### C. Atmosphere

- [ ] Low fog is concentrated from ground level to about 1.5 m and drifts slowly; it does not erase facade detail.
- [ ] At least three independent steam outlets show distinct intervals and a visible rise → lateral spread → dissipation lifecycle; steam is not a uniform particle ball.
- [ ] At least two distant light shafts show volumetric thickness, distance attenuation, and occlusion rather than flat emissive planes.
- [ ] Fog, steam, and shafts have distinct density and motion. Atmospheric layers respect depth/occlusion at nearby geometry.
- [ ] Half-resolution atmosphere uses depth-guided upsampling without conspicuous light leaks at building edges.
- [ ] Provide normal, atmosphere-disabled, and relevant depth/atmosphere debug captures.

### D. Non-emissive lived-in detail

- [ ] The 12-building, 120 m street contains at least four non-emissive detail categories per building and at least 240 small instances (or a documented equivalent coverage using merged geometry/decals).
- [ ] Across the district, include air conditioners, pipes, cables, vents, drains, stains, stickers, vending machines, and trash bins.
- [ ] Placement is structurally plausible: air conditioners near windows/service bands, cables follow supports, downpipes follow corners, and stains follow drainage.
- [ ] Street props do not obstruct the principal driving path. Repeated assets vary in scale, color, orientation, or wear; buildings exhibit at least three massing/setback patterns.
- [ ] Scene structure and lived-in detail remain discernible with emission disabled.
- [ ] Provide a wide view and close facade captures with emission disabled.

### E. Desynchronized motion

- [ ] Rain is continuously emitted at a high rate without synchronized batch respawns.
- [ ] Neon uses independent irregular events (roughly 2–9 s intervals); hologram scan/fault behavior is local (about 0.5–2 Hz) and content changes every 3–7 s without full-screen flashing.
- [ ] Vehicles travel at approximately 5–12 m/s with staggered 1.5–4 s spacing. Body transforms, headlights, dynamic ray proxies, and reflections agree. Headlight trails represent only about 0.1–0.25 s of motion.
- [ ] Drones transition between hover and movement with independent 3–11 s state changes; indicator lights do not synchronize across drones.
- [ ] Steam outlets have independent approximately 4–13 s event intervals. Ripples are spatially random and have visible lifetimes.
- [ ] Random streams derive from scene seed + system ID + instance ID. Adding a vehicle must not change existing sign flicker sequences.
- [ ] A fixed seed and fixed simulation step replay consistently. A continuous 30-second capture shows no broad phase synchronization.
- [ ] Fast camera movement and dynamic-object departure do not leave stale reflection trails; a departed dynamic reflection visibly clears within about 100 ms.
- [ ] Provide a 30-second video and motion/debug view demonstrating these checks.

### F. Color, exposure, and post-processing

- [ ] The visual balance is approximately 55–65% magenta/pink, 25–35% cyan/blue, and about 10% purple accent by perceived neon emphasis, not object counts.
- [ ] Dark facades, street, and equipment retain readable detail; they are not clipped to featureless black.
- [ ] Bloom is separated by source class and scale: tight tube cores, restrained LED bloom, energy-limited broader hologram bloom, and fault text that remains legible.
- [ ] Direct, reflected, and transparent-layer emissive sources enter the intended Bloom path.
- [ ] Fixed exposure preserves hue in bright signs; large areas do not become featureless white. Provide Bloom-disabled comparison.
- [ ] Chromatic aberration and vignette are independently disableable; if enabled, edge displacement is at most about 1 px at 1080p and corner luminance reduction at most about 15%.
- [ ] No strong grain or excessive aberration is used to hide low-resolution reflection or temporal artifacts.

## Required ray-tracing demonstrations

The following are explicit real-GPU/browser captures, not CPU/BVH unit-test substitutes:

1. Move an emissive sign fully outside the camera view and show its image in a puddle via world-space BVH reflection.
2. Drive a vehicle into and through the reflection region. Show the ray-traced headlight reflection tracking the same transform as the rasterized body/lights; include a dynamic-proxy or ray debug view.
3. Show AO and the selected local key-light shadow separately. Confirm the shadow is not incorrectly applied to every light.
4. Move the camera rapidly and change occlusion; demonstrate history rejection without persistent ghosting.
5. Show the reflection output with Bloom disabled so post-processing cannot mask a missing or unstable ray-traced result.

Record reflection/AO/soft-shadow debug outputs and any BVH overflow counters. A screenshot without the off-screen emitter test does not establish that the effect is not SSR.

## Performance evidence

- [ ] Benchmark at 1920×1080 output, 1280×720 internal resolution, and the documented reflection resolution (640×360 balanced; 320×180 performance tier).
- [ ] Use the same published fixed camera route; warm up for 10 seconds, then sample for 60 seconds. Report median, P95, and P99 frame times plus per-pass GPU timings when timestamp queries are supported.
- [ ] Record actual GPU, driver, OS, browser version, quality tier, and measured resolutions. State whether browser power-saving mode was disabled.
- [ ] Target P95 steady-state frame time is at most 33.3 ms on the declared target device. This is a measured acceptance target, not an assumption or guarantee.
- [ ] If GPU timestamps are unavailable, report that limitation and separate CPU submission time from observed frame time. Do not invent per-pass GPU measurements.
- [ ] Report device startup, tab background/resume, resize, and device-loss recovery separately from steady-state measurements.
- [ ] Demonstrate a 10-minute session without growing memory, persistent validation errors, or BVH stack overflows.
- [ ] Quality degradation may reduce reflection resolution, candidate-light count, atmosphere steps, distant detail/particles, and Bloom layers; it may not remove real BVH reflections, ray-traced AO, the three surface classes, four sign types, or readable text.

## Browser failure paths and Pages subpath

- [ ] On a browser without WebGPU, or when adapter/device creation fails, the page shows a visible, actionable error panel identifying the missing capability. It must not silently switch to WebGL/SSR and imply success.
- [ ] Exercise denied/unavailable adapter, unsupported format/limit, device loss, resize, pause/resume, and quality-tier change. History is invalidated when required; errors remain diagnosable.
- [ ] Test the deployed project URL under its actual Pages base path (commonly `/neon_city/`), not only at `/`. Verify document, JS glue, WASM, sign atlas, and other assets resolve below that base path and return successful HTTP responses with appropriate MIME types.
- [ ] Also verify a root base path (`/`) where applicable. Ensure no leading-root asset URL breaks a project-site deployment. The workflow simulates Trunk-style project and root HTML references and rejects `/evil.js` for the project base; this is a basic path check, not exhaustive validation of runtime fetches, CSS, fonts, or all nested assets.
- [ ] Confirm HTTPS and WebGPU startup on the deployed site. A successful CI artifact build or HTTP-only smoke test is not browser/GPU verification.
