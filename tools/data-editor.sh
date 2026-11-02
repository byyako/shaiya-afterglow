#!/bin/bash
# Run the data editor's page (docs/plans/done/data-editing.md section 5) in the background.
#   tools/data-editor.sh start|stop|restart|status|logs
# Reads .env (DATABASE_URL, EDITOR_EXPORT_FILE) from the project root. EDITOR_BIND defaults to 127.0.0.1:30890: reach it from another
# machine with  ssh -L 30890:127.0.0.1:30890 <this box>  and open http://localhost:30890/. A request must be addressed to localhost,
# 127.0.0.1 or [::1], or to a name in EDITOR_HOSTS (docs/server/audit.md R17). Settings can be overridden in the environment.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
logfile="$root/logs/data-editor.log"
bin="$root/target/release/data-editor"
mkdir -p "$root/logs"

# setsid forks, so the server is found by its exact command line instead of a saved PID
pid() { pgrep -f -x "$bin serve" | head -1; }
running() { [ -n "$(pid)" ]; }

case "${1:-status}" in
  start)
    running && { echo "already running (pid $(pid))"; exit 0; }
    [ -x "$bin" ] || (cd "$root" && cargo build --release -p data-editor) || exit 1
    set -a; . "$root/.env"; set +a
    : "${EDITOR_BIND:=127.0.0.1:30890}" "${RUST_LOG:=info}"
    export EDITOR_BIND RUST_LOG
    cd "$root" && setsid nohup "$bin" serve >>"$logfile" 2>&1 < /dev/null &
    sleep 1
    running && echo "started (pid $(pid)), on http://${EDITOR_BIND}/ (from elsewhere: ssh -L 30890:127.0.0.1:30890), log: $logfile" || { echo "failed to start, see $logfile"; exit 1; }
    ;;
  stop)
    if running; then kill "$(pid)"; for _ in $(seq 20); do running || break; sleep 0.25; done; running && { echo "did not stop"; exit 1; }; echo stopped; else echo "not running"; fi
    ;;
  restart) "$0" stop; "$0" start ;;
  status) running && echo "running (pid $(pid))" || echo "not running" ;;
  logs) tail -n "${2:-40}" "$logfile" | sed 's/\x1b\[[0-9;]*m//g' ;;
  *) echo "usage: $0 start|stop|restart|status|logs" ; exit 2 ;;
esac
