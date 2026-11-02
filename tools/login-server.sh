#!/bin/bash
# Run the login server in the background.
#   tools/login-server.sh start|stop|restart|status|logs
# Reads .env (DATABASE_URL) from the project root. Settings can be overridden in the environment.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
logfile="$root/logs/login-server.log"
bin="$root/target/release/login-server"
mkdir -p "$root/logs"

# setsid forks, so the server is found by its exact command line instead of a saved PID
pid() { pgrep -f -x "$bin" | head -1; }
running() { [ -n "$(pid)" ]; }

case "${1:-status}" in
  start)
    running && { echo "already running (pid $(pid))"; exit 0; }
    [ -x "$bin" ] || (cd "$root" && cargo build --release -p login-server) || exit 1
    set -a; . "$root/.env"; set +a
    : "${LOGIN_BIND:=0.0.0.0:30800}" "${RUST_LOG:=info,login_server=debug}"
    export LOGIN_BIND RUST_LOG
    ulimit -n 65536 2>/dev/null || ulimit -n "$(ulimit -Hn)" 2>/dev/null
    cd "$root" && setsid nohup "$bin" >>"$logfile" 2>&1 < /dev/null &
    sleep 2
    running && echo "started (pid $(pid)), listening on $LOGIN_BIND, log: $logfile" || { echo "failed to start, see $logfile"; exit 1; }
    ;;
  stop)
    if running; then kill "$(pid)"; for _ in 1 2 3 4 5 6 7 8 9 10; do running || break; sleep 0.5; done; running && { echo "did not stop"; exit 1; }; echo stopped; else echo "not running"; fi
    ;;
  restart) "$0" stop; "$0" start ;;
  status) running && echo "running (pid $(pid))" || echo "not running" ;;
  logs) tail -n "${2:-40}" "$logfile" | sed 's/\x1b\[[0-9;]*m//g' ;;
  *) echo "usage: $0 start|stop|restart|status|logs" ; exit 2 ;;
esac
