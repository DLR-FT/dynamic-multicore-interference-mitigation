`git clone ...`  
`cd dynamic-multicore-interference-mitigation`

`cargo build -p wasm-runner --release`  
`cargo build -p wasm-payload --release  --target wasm32-unknown-unknown`  
`RUST_LOG=trace cargo run -p example-linux --release -- --config ./example-linux/example.toml`  
