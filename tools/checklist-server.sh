#!/bin/bash
# Run the live-test checklist page (tools/checklist_server.py) in the background.
#   tools/checklist-server.sh start|stop|restart|status|logs|report
# CHECKLIST_BIND (default 0.0.0.0:30880) and CHECKLIST_RESULTS (default logs/live-test-results.json) can be set in the environment.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
logfile="$root/logs/checklist-server.log"
script="$root/tools/checklist_server.py"
mkdir -p "$root/logs"

# setsid forks, so the server is found by its command line instead of a saved PID
pid() { pgrep -f "python3 $script" | head -1; }
running() { [ -n "$(pid)" ]; }

case "${1:-status}" in
  start)
    running && { echo "already running (pid $(pid))"; exit 0; }
    cd "$root" && setsid nohup python3 "$script" >>"$logfile" 2>&1 < /dev/null &
    sleep 1
    running && echo "started (pid $(pid)), on ${CHECKLIST_BIND:-0.0.0.0:30880}, log: $logfile" || { echo "failed to start, see $logfile"; exit 1; }
    ;;
  stop)
    if running; then kill "$(pid)"; for _ in $(seq 20); do running || break; sleep 0.25; done; running && { echo "did not stop"; exit 1; }; echo stopped; else echo "not running"; fi
    ;;
  restart) "$0" stop; "$0" start ;;
  status) running && echo "running (pid $(pid))" || echo "not running" ;;
  logs) tail -n "${2:-40}" "$logfile" ;;
  report) python3 "$script" --report ;;
  *) echo "usage: $0 start|stop|restart|status|logs|report" ; exit 2 ;;
esac
