#!/bin/bash

# With thanks https://prateeknischal.github.io/posts/i-c-and-so-does-rust/

echo "Running the command: cargo run --" "$@"

pushd ../ || exit
env RUSTFLAGS="-L/opt/homebrew/lib/" cargo run -- "$@"
