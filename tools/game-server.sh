#!/bin/bash
# Run the game server in the background.
#   tools/game-server.sh start|stop|restart|status|logs
# Reads .env (DATABASE_URL; GAME_SERVER_ID defaults to 1) from the project root. Settings can be overridden in the environment.
# The server runs in a loop that starts it again after any non-zero exit status: 75 is the console's `restart`
# (docs/plans/done/data-editing.md section 7), the others are crashes. The loop gives up when the server exits more than 5 times within a
# minute (docs/server/audit.md R5). A clean exit (status 0) and `stop` end the loop.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
logfile="$root/logs/game-server.log"
bin="$root/target/release/game-server"
mkdir -p "$root/logs"

# setsid forks, so the server is found by its exact command line instead of a saved PID
pid() { pgrep -f -x "$bin" | head -1; }
loop_pid() { pgrep -f -x "bash -c .* game-server-loop $bin" | head -1; }
running() { [ -n "$(pid)" ]; }

case "${1:-status}" in
  start)
    running && { echo "already running (pid $(pid))"; exit 0; }
    [ -x "$bin" ] || (cd "$root" && cargo build --release -p game-server) || exit 1
    set -a; . "$root/.env"; set +a
    : "${GAME_BIND:=0.0.0.0:30810}" "${RUST_LOG:=info,game_server=debug}"
    export GAME_BIND RUST_LOG
    # Many connections need many descriptors; the soft limit is often 1024.
    ulimit -n 65536 2>/dev/null || ulimit -n "$(ulimit -Hn)" 2>/dev/null
    loop='starts=(); while true; do "$1"; code=$?; [ $code -eq 0 ] && exit 0
      now=$(date +%s); recent=(); for t in "${starts[@]}"; do [ $((now - t)) -lt 60 ] && recent+=("$t"); done; recent+=("$now"); starts=("${recent[@]}")
      if [ ${#starts[@]} -gt 5 ]; then echo "exit status $code: ${#starts[@]} exits within a minute, giving up"; exit 1; fi
      echo "restarting after exit status $code"; sleep 1; done'
    cd "$root" && setsid nohup bash -c "$loop" game-server-loop "$bin" >>"$logfile" 2>&1 < /dev/null &
    sleep 2
    running && echo "started (pid $(pid)), listening on $GAME_BIND, log: $logfile" || { echo "failed to start, see $logfile"; exit 1; }
    ;;
  stop)
    [ -n "$(loop_pid)" ] && kill "$(loop_pid)"
    if running; then kill "$(pid)"; for _ in $(seq 30); do running || break; sleep 0.5; done; running && { echo "did not stop"; exit 1; }; echo stopped; else echo "not running"; fi
    ;;
  restart) "$0" stop; "$0" start ;;
  status) running && echo "running (pid $(pid))" || echo "not running" ;;
  logs) tail -n "${2:-40}" "$logfile" | sed 's/\x1b\[[0-9;]*m//g' ;;
  *) echo "usage: $0 start|stop|restart|status|logs" ; exit 2 ;;
esac
