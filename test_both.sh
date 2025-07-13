#!/usr/bin/env bash
set -e

echo "Starting echo server..."
cargo run --example echo_server -- --nocapture &
SERVER_PID=$!

sleep 2
echo "Server started with PID: $SERVER_PID"

sleep 1
echo "Running echo client..."
cargo run --example echo_client -- --nocapture

echo "Stopping server..."
kill $SERVER_PID 2>/dev/null || true
echo "Test completed"
