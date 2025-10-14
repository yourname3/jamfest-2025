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
