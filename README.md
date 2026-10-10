# Ashen Vigil — the Hollow Asylum

A PS1-style demake of the opening area of a well-known dark fantasy action RPG:
wake in a cell, escape the asylum, and face the **Warden Demon**.

Everything is procedural: all textures, models, animations, sound effects and the
boss theme are generated in Rust at start-up. There are no asset files.

Built with **Rust + Dioxus 0.7** (UI), **WebGL2** (via `glow`) and **WebAudio**.

## Running

```sh
# one-time setup
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli        # or: cargo binstall dioxus-cli

# from this directory (not from crates/web: dx resolves workspace paths from here)
dx serve --platform web -p ashen-web            # dev server with hot reload
./tools/build-release.sh                         # publishable bundle in ./dist
```

### Publishing

```sh
./tools/build-release.sh
```

This produces `dist/` (`index.html` + `assets/*.js` + `assets/*.wasm`, about 1.6 MB).
Upload the **contents** of `dist/` to any folder on any static web server, e.g.
`https://example.com/` or `https://example.com/games/ashen/`. Paths are relative,
so no base-path configuration is needed, and there is no separate stylesheet:
the CSS is compiled into the wasm. Make sure the server sends `.wasm` files as
`application/wasm` (most do by default). A WebGL2-capable browser is required.

### Controls

| Action | Keyboard & mouse | Gamepad |
|---|---|---|
| Move / camera | WASD / mouse | sticks |
| Light attack (chain by pressing again) | Left click | RB |
| Heavy attack | Shift + Left click, or F | RT |
| Block (hold) | Right click | LB |
| Roll / backstep (tap), sprint (hold) | Space | B |
| Jump (while sprinting) | C | L3 |
| Lock on | Q / Tab / Middle click | R3 |
| Drink Ember Flask | R | X |
| Interact | E | A |
| Pause | Esc / P | Start |

Special attacks: attack while sprinting (running attack), right after a roll
(rolling attack), right after a backstep, or while falling (plunging attack —
use it from the ledge above the demon).

### URL parameters (development)

- `?start=new` / `?start=continue` skips the title screen.
- `?test=1` assumes pointer lock (for headless testing).
- `?debug=cmd;cmd` runs developer commands after start: `warp x y z yawDeg`,
  `key Cell|East|Great`, `flag <name>`, `flasks <n>`, `heal`, `god`, `reset`,
  `encounter`, `fps`. Use `+` for spaces, e.g. `?start=new&debug=god;warp+0+3+18.5+0`.

## Name changes

| Original | Here |
|---|---|
| Undead Asylum | the Hollow Asylum |
| Asylum Demon | Warden Demon |
| Bonfire | Pyre ("PYRE KINDLED") |
| Estus Flask | Ember Flask |
| Souls | Cinders |
| Hollows | Husks |
| Oscar of Astora | Sir Corvin of Valemark |
| Fog gate | Mist veil |
| Soapstone messages | Etched runes |
| YOU DIED / VICTORY ACHIEVED | YOU PERISHED / FOE VANQUISHED |
| Big Pilgrim's Key | Great Asylum Key |

## Architecture

```
crates/engine   platform-independent engine (native-testable)
  render/       WebGL2 renderer: low-res target, vertex snapping, affine UVs,
                Gouraud lighting, per-vertex fog, 15-bit dithered colour
  mesh.rs       procedural mesh builder (boxes, tapers, cylinders, lathes...)
  texture.rs    texture data + k-means CLUT-style quantisation
  skeleton.rs   rigid skeletons (one mesh per bone, PSX style)
  anim.rs       clips, crossfades, masked overlays, two-bone IK
  collision.rs  triangle grid, raycasts, character mover, toggleable groups
  synth.rs      offline DSP for procedural audio

crates/game     the game (no web dependencies; `cargo test -p ashen-game`)
  assets/       textures, models, props and sounds/music recipes
  rig/, anims/  humanoid + demon rigs; clips authored as IK key poses
  combat.rs     data-driven attacks, weapons, shields, hurtboxes
  actor.rs      shared action state machine (player and enemies)
  ai.rs         husk, archer and demon brains
  player.rs     input -> intent (tap roll / hold sprint, lock-on...)
  level.rs      level-building DSL (rooms, openings, stairs, zones, chunks)
  asylum.rs     the map
  world.rs      simulation, hit resolution, events
  script.rs     interactions, pyres, boss encounter, boulder, death/respawn
  view.rs       render-list construction
  game.rs       facade used by the shell (modes, fixed 60 Hz step, audio, HUD)

crates/web      Dioxus shell: canvas, input, WebAudio, menus and HUD
tools/          headless screenshot harness (Playwright) and input scripts
```

### Extending

- **New weapon**: add `Atk` variants and their `AttackDef`s in `combat.rs`
  (clip, timings, hit windows, root motion, cues), a `WeaponKind` + `WeaponDef`
  moveset, and a mesh in `assets/props.rs`. Animation clips go in
  `anims/humanoid.rs` as IK key poses.
- **New enemy**: give it a body/model, an `AnimSet`, a moveset and a `Brain`
  in `ai.rs`; it uses the same action system as the player.
- **New area**: write a map module like `asylum.rs` with the `LevelBuilder`
  DSL and spawn/prop lists. Progress is stored as flags in `save.rs`.
