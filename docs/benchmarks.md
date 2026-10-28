# Browser verification and performance

## Measurement status

This is a development report, **not final acceptance**. The local measurements below precede the initial implementation commit. Consult GitHub Actions for the current hosted deployment status.

Initial baseline: Windows, headless Chrome 156, WebGPU adapter reports NVIDIA / Lovelace. The browser does not expose a specific GPU model in this probe. Output: 1920×1080; internal G-buffer: 1280×720. Balanced rays: 640×360; Performance rays: 320×180.

Method: 10 seconds warm-up, then 60 seconds sampling RAF intervals at the stationary default camera with live animation. These are observed presentation intervals, **not GPU pass timings**. No timestamp queries or GPU overflow counter readback are implemented. This is not yet the planned moving-camera benchmark.

| Initial baseline | Median | P95 | P99 | Mean CPU submission |
|---|---:|---:|---:|---:|
| Balanced | 48.8 ms | 59.6 ms | 62.2 ms | 0.76 ms |
| Performance | 22.6 ms | 34.7 ms | 39.0 ms | 0.93 ms |

Neither tier passed the P95 ≤33.3 ms target in this initial measurement. BVH front-to-back traversal, skipping reflections on very rough surfaces, range/cone light culling, and spatial filtering were developed afterward.

## Optimized local retest

The same resolution and stationary-camera method, with 10 seconds warm-up and 60 seconds sampling per tier, passed the local P95 target:

| Optimized build | Median | P95 | P99 | Mean CPU submission |
|---|---:|---:|---:|---:|
| Balanced | 8.2 ms | 22.1 ms | 24.6 ms | 0.47 ms |
| Performance | 8.2 ms | 8.3 ms | 8.6 ms | 0.49 ms |

`evidence/browser-benchmark.json` is the final local tool report; `evidence/benchmark.json` is an intermediate optimized run. Variation between runs means these are not a portable GPU guarantee. No fixed moving-camera route or specific mid-range GPU certification is implied.

Local release checks also passed: 39 Rust tests (one additional benchmark ignored), formatting, WASM check, release Trunk build, and all 10 cases in `evidence/browser-check.json`. The `/neon_city/` path prefix was separately built and loaded successfully in Chrome at localhost.

## Observed correctness issues fixed during real browser testing

- Conditional implicit-LOD sampling was rejected by Chrome although Naga validation passed: switched to explicit LOD.
- Sign planes were behind facade surfaces; then U coordinates were mirrored and fixtures obscured lettering. Corrected coordinates, U direction, and placement exclusions.
- Initial paused mode never rendered: simulation can now freeze while render sampling continues.
- Actual canvas configuration is `bgra8unorm`, not an sRGB attachment. Added conditional linear-to-sRGB output conversion rather than increasing exposure to conceal incorrect transfer.
- Error-free CPU tests alone did not detect any of the above visual/runtime defects.

## Remaining acceptance work

- Re-run optimized performance with a reproducible moving-camera route and identify the specific GPU/driver.
- Validate temporal rejection and dynamic reflection ghost duration quantitatively.
- Complete ten-minute memory/device stability and 30-second desynchronization recordings.
- Confirm measured water coverage and every A–F item. Glass combines real BVH reflection with an explicitly approximate shallow procedural shop interior; it is not full scene transmission or refraction.
- AO is genuinely ray traced. A selected-light ray-traced shadow is **not implemented**; its debug output is neutral, not evidence of shadowing.
- Device loss stops with an error; automatic device recreation is not implemented. wgpu 24 browser callback lifetime may surface as a global JavaScript error, which must still halt the app.
- Test the actual hosted Pages URL and a clean CI build after publication is authorized.

## Local artifacts

`docs/evidence/` holds browser screenshots and machine-readable smoke/health/performance results. These are development evidence, not a checked-off acceptance certificate. Screenshots can become stale after edits; capture new ones from the final build.
