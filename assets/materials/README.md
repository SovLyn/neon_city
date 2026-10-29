# Neon City PBR materials

Three seamless Poly Haven CC0 materials are provided as 512×512 maps:

| Slot | Asset | Tile size | Source |
|---|---|---:|---|
| concrete | Concrete Wall 001 | 1.5 × 1.5 m | https://polyhaven.com/a/concrete_wall_001 |
| asphalt | Worn Asphalt | 2 × 2 m | https://polyhaven.com/a/worn_asphalt |
| metal | Rusty Metal 05 | 1.25 × 1.25 m | https://polyhaven.com/a/rusty_metal_05 |

The tile dimensions are from each asset's official Poly Haven API `dimensions` metadata, expressed in centimetres and converted to metres. Original 1K upstream JPEG URLs and SHA-256 hashes, derived map hashes, processing details and exact byte sizes are recorded in `manifest.json`.

## License

Poly Haven's official license statement at https://polyhaven.com/license says that all site textures are **CC0 1.0 Universal** (https://creativecommons.org/publicdomain/zero/1.0/). Redistribution, modification and commercial use are permitted without attribution. The material source pages are linked in the table above and recorded per material in the manifest.

## Pixel conventions and packed bindings

- `*-color.png`: RGB sRGB base color.
- `*-roughness.png`: grayscale, linear roughness data.
- `*-normal.png`: tangent-space OpenGL normal (`nor_gl`, +Y), RGB linear data. No green-channel inversion was applied.
- `materials-color.png`: 1536×1023 RGBA mip strip, concrete/asphalt/metal layers. RGB is sRGB color; alpha is 255.
- `materials-detail.png`: 1536×1023 RGBA mip strip, same layer order. RGB is OpenGL normal data; alpha is linear roughness. Pillow writes raw RGBA (no alpha premultiplication).

Mip levels occupy successive rows: size 512 at y=0, 256 at y=512, 128 at y=768, 64 at y=896, then 32/16/8/4/2/1 at y=960/992/1008/1016/1020/1022. Within each level, a layer begins at x=layer×size; remaining pixels are zero. The manifest records every level's offsets. Color mip filtering averages in linear light and re-encodes sRGB; normal filtering decodes vectors to [-1,1], averages and renormalizes; roughness is averaged linearly.

The packed PNGs can be split into three layers and each mip crop loaded separately. The Rust `image` crate's raw PNG decode is suitable; browser canvas is not required. The renderer uploads the strips into two three-layer texture arrays: sRGB color and linear normal/roughness. Raster and BVH hit shaders share world-space projection and explicit mip selection. `?textures=0` disables both paths for comparison. `average_linear_luminance` in the manifest is retained as the mean Rec.709 luminance calculated after IEC sRGB decoding, for brightness-preserving shader compensation.

## Rebuild

With Python and Pillow installed, run from repository root:

```sh
python tools/prepare_materials.py
```

Rebuild is fully offline and uses the nine committed 512×512 maps as inputs. It checks their SHA-256 values against the manifest before producing the packed images; it does not redownload upstream sources. The original map provenance (official URLs, source hashes and license evidence) remains in the manifest.

The two packed runtime PNGs total **4,209,652 bytes**. All eleven PNGs (including nine source maps retained for rebuild) total **7,012,189 bytes**. `runtime_png_bytes` denotes only the two packed images; `total_png_bytes` denotes every PNG in this folder.
