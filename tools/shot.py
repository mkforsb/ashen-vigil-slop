"""Headless browser harness: loads the built game, optionally sends input, takes screenshots.

Usage: shot.py URL OUT_PREFIX [--wait SECS] [--script JSON_FILE] [--size WxH]

The script is a JSON list of steps:
  {"wait": 1.5}                      sleep (game keeps running)
  {"key": "KeyW", "down": true}      key down / up (down: false)
  {"press": "Space", "hold": 0.1}    tap a key
  {"click": [x, y]}                  mouse click at canvas coords
  {"mouse": [dx, dy]}                relative mouse move (dispatched as movementX/Y)
  {"button": 0, "down": true}        mouse button down/up
  {"shot": "name"}                   save OUT_PREFIX_name.png
  {"eval": "js"}                     run JS, print result
"""
import asyncio, json, sys, argparse
from playwright.async_api import async_playwright

async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("url"); ap.add_argument("out")
    ap.add_argument("--wait", type=float, default=4.0)
    ap.add_argument("--script")
    ap.add_argument("--size", default="960x540")
    a = ap.parse_args()
    w, h = map(int, a.size.split("x"))
    steps = json.load(open(a.script)) if a.script else [{"wait": a.wait}, {"shot": "final"}]
    async with async_playwright() as p:
        b = await p.chromium.launch(args=["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader",
                                          "--autoplay-policy=no-user-gesture-required", "--ignore-gpu-blocklist"])
        pg = await b.new_page(viewport={"width": w, "height": h})
        pg.on("console", lambda m: print(f"[console.{m.type}] {m.text}"))
        pg.on("pageerror", lambda e: print(f"[pageerror] {e}"))
        await pg.goto(a.url)
        for st in steps:
            if "wait" in st: await asyncio.sleep(st["wait"])
            elif "press" in st:
                await pg.keyboard.down(st["press"]); await asyncio.sleep(st.get("hold", 0.08)); await pg.keyboard.up(st["press"])
            elif "key" in st:
                await (pg.keyboard.down if st.get("down", True) else pg.keyboard.up)(st["key"])
            elif "click" in st: await pg.mouse.click(*st["click"])
            elif "button" in st:
                btn = ["left", "middle", "right"][st["button"]]
                await (pg.mouse.down if st.get("down", True) else pg.mouse.up)(button=btn)
            elif "mouse" in st:
                dx, dy = st["mouse"]
                await pg.evaluate(f"window.dispatchEvent(new MouseEvent('mousemove', {{movementX: {dx}, movementY: {dy}}}))")
            elif "eval" in st: print("[eval]", await pg.evaluate(st["eval"]))
            elif "shot" in st:
                path = f"{a.out}_{st['shot']}.png"
                await pg.screenshot(path=path); print("saved", path)
        await b.close()

asyncio.run(main())
