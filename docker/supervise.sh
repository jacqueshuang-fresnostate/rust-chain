#!/usr/bin/env bash

set -Eeuo pipefail

api_pid=""
nginx_pid=""
shutdown_started=0

stop_children() {
    if ((shutdown_started)); then
        return
    fi
    shutdown_started=1

    for pid in "$api_pid" "$nginx_pid"; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
}

trap stop_children HUP INT QUIT TERM

export APP_HOST=127.0.0.1
export APP_PORT=8081

/usr/local/bin/exchange-api &
api_pid=$!

/usr/sbin/nginx -c /etc/nginx/nginx.conf -g "daemon off;" &
nginx_pid=$!

set +e
wait -n "$api_pid" "$nginx_pid"
exit_status=$?
set -e

stop_children

wait "$api_pid" 2>/dev/null || true
wait "$nginx_pid" 2>/dev/null || true

exit "$exit_status"
