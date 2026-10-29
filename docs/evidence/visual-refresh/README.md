# Visual refresh — brighter, inhabited night street

User feedback: the original scene was too dark and monotonous.

Changes: cool hemispheric ambient illumination (also applied at BVH hits), a violet horizon, brighter local practical lights with visible lamp geometry, upper-floor window grids with mixed dark/cool/warm rooms, floor courses and window frames, alternating cyan/magenta shop canopies, road markings/crossings, brighter approximate shop interiors, and three independent vehicle paint colors. Window emission was reduced after inspecting the first pass to retain neon sign emphasis. No global exposure multiplier or extra bloom was used to conceal the original darkness.

Windows, frames, canopies and markings are real static geometry included in the BVH. Existing true reflected geometry and AO remain enabled; the analytic sky only supplies the background/miss environment. Glass interiors remain a disclosed procedural approximation.

Verification:
- 41 Rust tests passed, one additional benchmark ignored; format, WASM check and Trunk release build passed.
- All 10 portable browser-check cases passed using installed Chrome on the local real WebGPU adapter.
- Manually inspected default-camera normal and bloom-disabled frames; checked building details, window hierarchy, road markings and car colors.
- 1080p output / 720p internal, stationary camera and live animation: 10s warmup then 60s per tier. Observed RAF P95 Balanced 8.4ms; Performance 4.3ms. These are NOT GPU pass timings or portable hardware guarantees, and are not a controlled cross-build speedup comparison.
- `street.png` and `street-no-bloom.png`: default camera at simulation time 3, paused rendering with temporal accumulation. `reflection.png`: showcase debug reflection, matching the portable script's view definition (not the same camera as street.png).

Prior benchmark reports describe the earlier build. Full A–F acceptance, moving-route performance and GPU memory accounting remain incomplete.
