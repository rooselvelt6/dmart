#!/bin/bash
# dMart UCI Server - Script de inicio con auto-restart

APP_NAME="dmart-server"
APP_DIR="/home/tdy/Escritorio/dmart"
APP_BIN="$APP_DIR/target/release/dmart-server"
PID_FILE="$APP_DIR/dmart.pid"
LOG_FILE="$APP_DIR/server.log"

start() {
    if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "$APP_NAME ya está corriendo (PID: $(cat $PID_FILE))"
        return 1
    fi

    echo "Iniciando $APP_NAME..."
    cd "$APP_DIR"
    export DMART_PORT=3000
    nohup "$APP_BIN" >> "$LOG_FILE" 2>&1 &
    echo $! > "$PID_FILE"

    sleep 2

    if kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "$APP_NAME iniciado (PID: $(cat $PID_FILE))"
        curl -s -o /dev/null -w "Verificando... HTTP %{http_code}\n" http://localhost:3000/
        return 0
    else
        echo "Error al iniciar $APP_NAME"
        return 1
    fi
}

stop() {
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if kill -0 "$PID" 2>/dev/null; then
            echo "Deteniendo $APP_NAME (PID: $PID)..."
            kill "$PID"
            sleep 2
            if ! kill -0 "$PID" 2>/dev/null; then
                echo "$APP_NAME detenido"
                rm -f "$PID_FILE"
                return 0
            fi
        fi
        rm -f "$PID_FILE"
    fi
    pkill -f "$APP_NAME" 2>/dev/null
    echo "$APP_NAME detenido"
    return 0
}

status() {
    if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "$APP_NAME corriendo (PID: $(cat $PID_FILE))"
        curl -s -o /dev/null -w "HTTP Status: %{http_code}\n" http://localhost:3000/ 2>/dev/null || echo "Sin respuesta"
        return 0
    else
        echo "$APP_NAME no está corriendo"
        return 1
    fi
}

restart() {
    stop
    sleep 2
    start
}

case "$1" in
    start)   start ;;
    stop)    stop ;;
    status)  status ;;
    restart) restart ;;
    *)
        echo "Uso: $0 {start|stop|status|restart}"
        exit 1
        ;;
esac