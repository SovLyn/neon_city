# Neon City

Rust + wgpu WebGPU browser project. The native target is used for CPU-side tests; rendering is implemented for browser WebGPU (`wasm32-unknown-unknown`). Local browser checks and performance sampling are recorded in [docs/benchmarks.md](docs/benchmarks.md). Full visual, compatibility, and long-duration acceptance remains in progress. The ray tracer uses a software BVH in wgpu shaders; it is not hardware ray tracing. Browsers without usable WebGPU (or with insufficient adapter limits) show an error and do not fall back to WebGL.

## Requirements

- Rust **1.95.0**, pinned in `rust-toolchain.toml`; rustup installs `wasm32-unknown-unknown` from that file.
- Trunk **0.21.14**.
- A browser with WebGPU enabled and a compatible GPU/driver. For local development, serve over `localhost`.

The atlas and font are committed runtime assets. Python 3.13, Pillow 12.1.1 and fonttools 4.61.1 are only needed to regenerate/verify them; see [assets/README.md](assets/README.md). They are not Cargo dependencies. The bundled Noto Sans JP subset is under SIL OFL 1.1; see `assets/licenses/` and the asset documentation for provenance and redistribution terms.

## Run and build

```sh
rustup toolchain install 1.95.0 --profile minimal --target wasm32-unknown-unknown
cargo install trunk --locked --version 0.21.14
trunk serve
```

Open the localhost URL printed by Trunk. Production build:

```sh
trunk build --release --locked --public-url /
```

For a project Pages site, build with its path prefix, for example `trunk build --release --locked --public-url /neon_city/`. The GitHub Actions workflow derives the actual Pages base path for pushes to `master`; PR/manual builds use `/neon_city/` as a build-path check. Trunk copies `assets/` from `index.html` into the output. Do not hard-code root-relative runtime asset URLs: the atlas is fetched relative to the document base.

**Windows Git Bash:** when supplying `--public-url` from Git Bash, MSYS may rewrite slash-prefixed arguments as filesystem paths. Disable that conversion for the command:

```sh
MSYS_NO_PATHCONV=1 trunk build --release --locked --public-url /neon_city/
```

## GitHub Pages

In the repository settings, set **Settings → Pages → Build and deployment → Source → GitHub Actions**. A push to `master` runs validation/build and deploys the artifact using the Pages workflow. PR and manual runs build/check but do not deploy. Deployment target: **https://sovlyn.github.io/neon_city/**. Check the [Actions workflow](https://github.com/SovLyn/neon_city/actions/workflows/pages.yml) for the current deployment result.

## Controls and URL options

- **W/A/S/D**: move; drag on the canvas to look around.
- **Space**: pause/resume simulation. `?paused=1` starts paused.
- **Q**: toggle Balanced/Performance quality.

Options are query parameters; e.g. `?seed=123&camera=showcase&bloom=0&atmosphere=0&time=12&paused=1`. `seed` accepts decimal or `0x` hexadecimal. `camera=showcase`, `camera=side`, and `camera=reflection` select preset starts (other values use the default). The reflection preset looks downward for off-screen reflection inspection. `debug` accepts `albedo`, `normal`, `roughness`, `depth`, `motion`, `reflection`, `ao`, `shadow`, `history`, or `bvh`; unknown values select final view. `bloom=0` and `atmosphere=0` disable those effects; otherwise enabled. `time` sets a finite nonnegative starting simulation time in seconds. Invalid seed/time values fall back to defaults.

## Browser evidence tools

The optional Playwright scripts in `tools/` inspect an already-running local build; they do not build or start the app. They use Node's normal module resolution (including `NODE_PATH`) and Playwright's bundled Chromium by default. Set `CHROME_PATH` to use a locally installed Chrome/Chromium instead. `BASE_URL` defaults to `http://127.0.0.1:4173/` and should include any project path prefix, such as `http://127.0.0.1:4173/neon_city/`. `EVIDENCE_DIR` defaults to `docs/evidence`.

Install a pinned Playwright version into a temporary prefix, without adding project dependencies:

```sh
npm install --prefix "$HOME/.cache/neon-city-playwright" playwright@1.64.1
node "$HOME/.cache/neon-city-playwright/node_modules/playwright/cli.js" install chromium
NODE_PATH="$HOME/.cache/neon-city-playwright/node_modules" node tools/browser-check.cjs
NODE_PATH="$HOME/.cache/neon-city-playwright/node_modules" node tools/browser-benchmark.cjs
```

PowerShell example:

```powershell
npm install --prefix "$env:LOCALAPPDATA\\neon-city-playwright" playwright@1.64.1
$env:NODE_PATH = "$env:LOCALAPPDATA\\neon-city-playwright\\node_modules"
node "$env:NODE_PATH\\playwright\\cli.js" install chromium
node tools/browser-check.cjs
node tools/browser-benchmark.cjs
```

`browser-check.cjs` saves fixed-time paused-view screenshots and a JSON report, and exercises quality, resize, pause/resume, no-adapter, missing-atlas, and destroyed-device failure paths. Failures produce a nonzero exit status. `browser-benchmark.cjs` measures Balanced and Performance tiers with 10 seconds of warmup and 60 seconds of RAF samples per tier by default; override with `SAMPLE_SECONDS` (for example `SAMPLE_SECONDS=15`). It reports observed sample duration. RAF intervals and `cpu_ms` are browser/application observations, **not GPU timings**. Neither tool changes application code.

## Project status

The current code builds the scene and software BVH and runs a WebGPU rendering loop, but passing compile/tests is not evidence of final image correctness or performance. Release acceptance—including real-browser/GPU visual checks, long-session stability, and measured frame-time targets—is tracked in [docs/acceptance.md](docs/acceptance.md); unchecked items are not claimed complete. Shared CPU/GPU layout notes are in [docs/contracts.md](docs/contracts.md).
