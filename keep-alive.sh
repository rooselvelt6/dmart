#!/bin/bash
# Mantiene el servidor dmart corriendo permanentemente

while true; do
    if ! pgrep -f "dmart-server" > /dev/null 2>&1; then
        echo "[$(date)] Iniciando dmart-server..."
        cd /home/tdy/Escritorio/dmart
        ./target/release/dmart-server > /dev/null 2>&1 &
        sleep 3
    fi
    sleep 5
done