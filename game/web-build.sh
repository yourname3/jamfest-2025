wasm-pack build --target web
if [ -d pkg ]; then
    cp index.html pkg/
fi