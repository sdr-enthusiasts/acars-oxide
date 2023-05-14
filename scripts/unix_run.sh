#!/bin/bash

echo "Running the command: cargo run --" "$@"

pushd ../ || exit
env cargo run -- "$@"
