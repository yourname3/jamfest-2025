# ben's beams

[play online on itch.io](https://some-games-by-bee.itch.io/bens-beams)

ben's beams is a little puzzle game written in Rust. It is a prototype for 
game engine technologies I am working on.

ben's beams compiles both as a desktop application and as a web application.
It targets `wasm32-unknown-unknown` for the web and so uses essentially all-Rust
libraries.

## Project structure

This project currently consists of three crates:

`engine`: the "engine" code. It is meant to be mostly separate from the game,
although due to its nature as a jam game, there are a couple things that are
baked-in a bit (such as the sky shader using a constant background color).

`game`: the actual gameplay code & assets. All of the assets are currently
embedded into the executable using `include_bytes!` and `include_str!`.

`inline_tweak`: a fork of the `inline_tweak` crate that enables it to work when
running `cargo run` from inside the `game` folder.

## Building & running

On Windows or Linux (and presumably Mac): simple `cargo build` and `cargo run`
inside the `game` crate should be sufficient to build & run the game. Of course,
a `--release` build is recommended for the best build of the game.

For the web build, `wasm-pack` is required. Inside `game`, you will want to
run:
```bash
wasm-pack build --target web
cp index.html pkg
```
Then you can run an HTTP server (e.g. `python -m http.server`) and access the
game from the `index.html` inside `pkg`.

Note that `web-build.sh` also performs these same commands.

On Linux, you can also easily cross-build for Windows by:
```bash
rustup target add x86-pc-windows-gnu
cargo build --target x86-pc-windows-gnu
```

You may need to install packages such as `mingw-w64-x86-64-dev` in order to 
complete the cross-compilation.

## Game code structure

The game code is currently split between `lib.rs` and `level.rs`. It could
perhaps stand to be split up more.

The most important structs in the game code are:
- `GameplayLogic`: The root struct that holds all the gameplay state, used to
  implement the `Gameplay` trait from the engine.
- `Assets`: contains a reference to each of the assets we wish to use inside
  the game. 
- `Level`: contains the main grid of devices, and handles moving them around
  and computing laser paths.
- `Selector`: handles the logic for selecting and moving pieces in the level.

Many aspects of the game may require access to the `Engine`, plus several of
these other structs.

### Adding new levels

The levels for this game are created in Tiled. You can start from the level 
`hook_something.tmx` which contains every level object that is currently
implemented or partially implemented.

Any level that is accessed by the game must be made available to the `tiled`
crate that is used for loading them. This is done in the function `load_level`
in level.rs, where you will want to add an additional call to the `tiled_file!`
macro.

```rust
fn load_level(path: &str) -> tiled::Map {
    let mut loader = Loader::with_reader(|path: &std::path::Path| -> std::io::Result<_> {
        tiled_file!(path, "./levels/test.tmx");
        tiled_file!(path, "./levels/my_new_level.tmx"); // <-- add a new line!

        // ...

        tiled_file!(path, "./levels/another_swaps.tmx");

        Err(std::io::ErrorKind::NotFound.into())
    });

    loader.load_tmx_map(path).unwrap()
}
```

You will also want to add it to the `LEVELS` array in lib.rs. It will then
automatically appear on the level select. If enough levels are added, the level
select UI may need to be adjusted.

#### Level object structure

The floor of the level must currently be set up manually. Anywhere there is a
solid floor tile is where level elements can be moved by the player.

Each level element itself is on the objects layer. Note that the tiles only
represent the top-left corner of the object; you simply have to know how big
the objects are when designing the levels.

Each level element may have a `locked` property. Any element where the `locked`
property is true will be unmoveable by the player.

The `emitter` and `goal` objects each have a `color` property. This is the color
that is expected. Note that the main `mix` function is not a simple average;
instead, it averages the input colors and then normalizes the color vector.
This means e.g. red (255, 0, 0) + green (0, 255, 0) = (180, 180, 0).

### Garbage collector

The `Gp` type (garbage-collected pointer) is meant to be used with a garbage
collector. However, the garbage collector is not actually implemented in this
game, so instead garbage-collected objects are simply leaked.

For the most part this is fine, so long as we are sure to leak a finite number
of objects. This is particularly relevant for the `MeshInstance` structs, which
each allocate resources on the GPU and so can exhaust the GPU resources if they
are simply leaked. So instead, the `InstancePool` struct is used to re-use a
small number of `MeshInstance` structs, while still allowing the code to
dynamically build the set of rendered `MeshInstance`s each frame.

