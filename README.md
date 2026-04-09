# 🐦‍⬛ Jackdaw 🐦‍⬛

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/jbuehler23/jackdaw#license)
[![Crates.io](https://img.shields.io/crates/v/jackdaw.svg)](https://crates.io/crates/jackdaw)
[![Downloads](https://img.shields.io/crates/d/jackdaw.svg)](https://crates.io/crates/jackdaw)
[![Docs](https://docs.rs/jackdaw/badge.svg)](https://docs.rs/jackdaw/latest/jackdaw/)
[![Discord](https://img.shields.io/discord/1486394042563428388.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/NgyEVW5tWG)


A 3D editor built for and with [Bevy](https://bevyengine.org/).
Very early in dev, expect bugs and changes! A BSN-friendly branch exists on the bsn-editor branch (the flow is to read/write to the BSN AST and then sync to the ECS for rendering the UI and viewport).

We have also recently refactored our UX/UI to be _very_ similar to the official Bevy Editor Figma design, to keep things consistent. [Link here](https://www.figma.com/design/fkYfFPSBgnGkhbQd3HOMsL/Bevy-Editor?node-id=90-2)

<img width="1899" height="1014" alt="image" src="https://github.com/user-attachments/assets/3a6611b3-0974-42dc-af78-a6087c222c4d" />




https://github.com/user-attachments/assets/cfe6fb2f-faa2-48be-b3d0-50dee0d0b753



https://github.com/user-attachments/assets/929c893a-4959-4cc1-bec7-5217c4d33eba



## Features

- **Brush-based geometry** draw, edit, and CSG-combine convex brushes with vertex/edge/face/clip editing modes
- **Material system** VERY wip - texture browser, material definitions with ORM auto-detection, per-face application
- **Terrain** heightmap sculpting and texture painting, very WIP :)
- **Scene serialization** save/load scenes in the `.jsn` format with full asset references. Ideally to be replaced with BSN once ready. 
- **Transform tools** translate, rotate, scale with grid snapping and axis constraints
- **Undo/redo** full command history - some bugs atm with this
- **Extensible** register custom components, add inspector panels, integrate with your game

## Usage

Standalone install:
```sh
cargo install jackdaw
```
Open Jackdaw, and point to a new project!
<img width="943" height="1018" alt="image" src="https://github.com/user-attachments/assets/3bda18cc-9cad-4d2c-b976-ca2e6e454314" />


Add `jackdaw` to your project:

```sh
cargo add jackdaw
```

Then add the `EditorPlugin` to your app:

```rust
use bevy::prelude::*;
use jackdaw::EditorPlugin;

fn main() -> AppExit {
    App::new()
        .add_plugins((DefaultPlugins, EditorPlugin))
        ...
        .run()
}
```

See the [examples](examples/) for more advanced usage.

## Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| RMB + Drag | Look around |
| WASD | Move |
| Q / E | Move up / down |
| Shift | Double speed |
| Scroll | Dolly forward / back |
| F | Focus selected |

### Editing

| Key | Action |
|-----|--------|
| Esc / R / T | Translate / Rotate / Scale mode |
| B / C | Draw brush (add / cut) |
| 1-4 | Brush edit: Vertex / Edge / Face / Clip |
| Ctrl+D | Duplicate |
| Delete | Delete selected |
| Ctrl+Z / Ctrl+Shift+Z | Undo / Redo |
| Ctrl+S | Save scene |

For the full shortcuts reference, see the [book](https://jbuehler23.github.io/jackdaw/user-guide/keyboard-shortcuts.html).

## Documentation

- [Book](https://jbuehler23.github.io/jackdaw/) - user guide, developer guide, and reference
- [Examples](examples/) - runnable examples
- [API Docs](https://docs.rs/jackdaw) - rustdoc

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and PR guidelines.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
