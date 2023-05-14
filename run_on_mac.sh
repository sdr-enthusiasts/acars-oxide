#!/bin/bash

# With thanks https://prateeknischal.github.io/posts/i-c-and-so-does-rust/

env RUSTFLAGS="-L/opt/homebrew/lib/" cargo run -- -l --sdr1serial 00013305  --sdr1freqs 13300000 --sdr1decoding-type acars