#!/usr/bin/env python3
"""Offline rebuild of packed mip-strip PNGs from the committed 512px maps."""
from __future__ import annotations
import hashlib, json, math
from pathlib import Path
from PIL import Image

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/"assets/materials"
LAYERS=("concrete","asphalt","metal")
LEVEL_SIZES=tuple(512 >> i for i in range(10))
def sha(data): return hashlib.sha256(data).hexdigest()
def load_rgb(path):
    with Image.open(path) as im:
        if im.size != (512,512): raise ValueError(f"{path}: expected 512x512, got {im.size}")
        return list(im.convert("RGB").get_flattened_data())
def load_l(path):
    with Image.open(path) as im:
        if im.size != (512,512): raise ValueError(f"{path}: expected 512x512, got {im.size}")
        return list(im.convert("L").get_flattened_data())
def srgb_to_linear(v):
    x=v/255.0
    return x/12.92 if x<=0.04045 else ((x+0.055)/1.055)**2.4
def linear_to_srgb(v):
    x=12.92*v if v<=0.0031308 else 1.055*(v**(1/2.4))-0.055
    return max(0,min(255,int(round(x*255))))
def down_color(src,w):
    nw=w//2; out=[]
    for y in range(nw):
      for x in range(nw):
        ids=(2*y*w+2*x,2*y*w+2*x+1,(2*y+1)*w+2*x,(2*y+1)*w+2*x+1)
        cs=[src[i] for i in ids]
        out.append(tuple(linear_to_srgb(sum(srgb_to_linear(c[k]) for c in cs)*.25) for k in range(3)))
    return out

def down_detail(norm,rough,w):
    nw=w//2; no=[]; ro=[]
    for y in range(nw):
      for x in range(nw):
        ids=(2*y*w+2*x,2*y*w+2*x+1,(2*y+1)*w+2*x,(2*y+1)*w+2*x+1)
        vec=[sum((norm[i][k]/127.5-1.0) for i in ids)*.25 for k in range(3)]
        length=math.sqrt(sum(v*v for v in vec))
        if length>1e-12: vec=[v/length for v in vec]
        else: vec=[0.0,0.0,1.0]
        no.append(tuple(max(0,min(255,int(round((v+1)*127.5)))) for v in vec))
        ro.append(int(round(sum(rough[i] for i in ids)*.25)))
    return no,ro

def main():
    manifest=json.loads((OUT/"manifest.json").read_text(encoding="utf-8"))
    colors=[]; normals=[]; roughs=[]
    for name in LAYERS:
      m=manifest["materials"][name]["maps"]
      for key in ("color","normal","roughness"):
        p=OUT/m[key]["output"]
        if sha(p.read_bytes()) != m[key]["output_sha256"]: raise ValueError(f"Input hash mismatch: {p}")
      colors.append(load_rgb(OUT/m["color"]["output"]))
      normals.append(load_rgb(OUT/m["normal"]["output"]))
      roughs.append(load_l(OUT/m["roughness"]["output"]))
    strip_h=sum(LEVEL_SIZES)
    color_img=Image.new("RGBA",(1536,strip_h),(0,0,0,0))
    detail_img=Image.new("RGBA",(1536,strip_h),(0,0,0,0))
    offsets=[]; y=0
    for level,size in enumerate(LEVEL_SIZES):
      offsets.append({"level":level,"size":size,"y_offset":y,"height":size,"layers":[{"layer":idx,"material":name,"x_offset":idx*size,"width":size} for idx,name in enumerate(LAYERS)]})
      for idx in range(3):
        for row in range(size):
          start=row*size
          cp=[(*colors[idx][start:start+size][col],255) for col in range(size)]
          dp=[(*normals[idx][start:start+size][col],roughs[idx][start:start+size][col]) for col in range(size)]
          color_img.paste(Image.frombytes("RGBA",(size,1),bytes(c for pixel in cp for c in pixel)),(idx*size,y+row))
          detail_img.paste(Image.frombytes("RGBA",(size,1),bytes(c for pixel in dp for c in pixel)),(idx*size,y+row))
      if size>1:
        colors=[down_color(p,size) for p in colors]
        pairs=[down_detail(normals[i],roughs[i],size) for i in range(3)]
        normals=[v[0] for v in pairs]; roughs=[v[1] for v in pairs]
      y+=size
    specs=(("materials-color.png",color_img,"packed_color"),("materials-detail.png",detail_img,"packed_detail"))
    for filename,image,key in specs:
      path=OUT/filename
      image.save(path,format="PNG",optimize=True)
      data=path.read_bytes(); manifest["output"][key].update({"sha256":sha(data),"bytes":len(data)})
    manifest["output"].update({"packed_dimensions":[1536,strip_h],"mip_count":len(LEVEL_SIZES),"mip_filtering":"Each level averages 2x2 samples from previous level; color averages in linear light then encodes sRGB; normal vectors decode to [-1,1], average and renormalize; roughness averaged linearly.","mip_levels":offsets,"runtime_png_bytes":sum((OUT/f).stat().st_size for f,_,_ in specs),"total_png_bytes":sum(p.stat().st_size for p in OUT.glob("*.png"))})
    manifest["output"]["packed_color"].update({"dimensions":[1536,strip_h],"layout":"RGBA mip strip. Each mip row has concrete/asphalt/metal horizontally at x=layer*size; unused pixels zero. RGB sRGB; A=255."})
    manifest["output"]["packed_detail"].update({"dimensions":[1536,strip_h],"layout":"RGBA mip strip. Each mip row has concrete/asphalt/metal horizontally at x=layer*size; unused pixels zero. RGB=OpenGL/+Y normal data, A=linear roughness; raw non-premultiplied RGBA."})
    (OUT/"manifest.json").write_text(json.dumps(manifest,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"runtime_png_bytes":manifest["output"]["runtime_png_bytes"],"total_png_bytes":manifest["output"]["total_png_bytes"],"mip_levels":offsets},indent=2))
if __name__=="__main__": main()
