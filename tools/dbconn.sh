# How the shell tools reach the databases; the same rules as tools/dbconn.py. Source it:
#   . "$root/tools/dbconn.sh"
#   psql_cmd < file.sql             # psql with -v ON_ERROR_STOP=1 -q; more psql arguments may follow
#   docker exec "$MSSQL_CONTAINER" ...
# - .env at the repository root is read for any variable not already in the environment (KEY=value, # comments, optional quotes);
# - PG_CONTAINER (default newshaiya-postgres), MSSQL_CONTAINER (default newshaiya-mssql);
# - the PostgreSQL user and database come from DATABASE_URL;
# - NEWSHAIYA_PSQL = docker | host | auto (default auto): docker runs psql inside PG_CONTAINER, host runs the host's
#   psql "$DATABASE_URL", auto uses docker when that container is running, else host when psql is on the PATH.

_dbconn_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

_dbconn_load_env() {
    local line key value
    [ -f "$_dbconn_root/.env" ] || return 0
    while IFS= read -r line || [ -n "$line" ]; do
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        case "$line" in ''|'#'*) continue ;; *=*) ;; *) continue ;; esac
        key="${line%%=*}"; value="${line#*=}"
        key="${key#export }"; key="${key//[[:space:]]/}"
        value="${value#"${value%%[![:space:]]*}"}"
        if [ "${#value}" -ge 2 ]; then
            case "$value" in \"*\"|\'*\') value="${value:1:${#value}-2}" ;; esac
        fi
        [[ "$key" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || continue
        [ -n "${!key+x}" ] || export "$key=$value"
    done < "$_dbconn_root/.env"
}
_dbconn_load_env

PG_CONTAINER="${PG_CONTAINER:-newshaiya-postgres}"
MSSQL_CONTAINER="${MSSQL_CONTAINER:-newshaiya-mssql}"

# user and database name from DATABASE_URL (postgres://user:password@host:port/db)
_dbconn_parse_url() {
    if [ -z "${DATABASE_URL:-}" ]; then
        echo "DATABASE_URL is not set: copy .env.example to .env and fill it in (README step 1)" >&2
        return 1
    fi
    local rest="${DATABASE_URL#*://}"
    rest="${rest%%\?*}"
    local auth="" hostpart="$rest"
    case "$rest" in *@*) auth="${rest%@*}"; hostpart="${rest##*@}" ;; esac
    PG_USER="${auth%%:*}"; PG_USER="${PG_USER:-postgres}"
    PG_DB="${hostpart#*/}"; [ "$PG_DB" = "$hostpart" ] && PG_DB=""; PG_DB="${PG_DB:-postgres}"
}

_dbconn_mode=""
psql_mode() {
    if [ -n "$_dbconn_mode" ]; then echo "$_dbconn_mode"; return 0; fi
    local mode="${NEWSHAIYA_PSQL:-auto}"
    case "$mode" in
        docker|host) ;;
        auto)
            if command -v docker > /dev/null && [ "$(docker inspect -f '{{.State.Running}}' "$PG_CONTAINER" 2> /dev/null)" = true ]; then
                mode=docker
            elif command -v psql > /dev/null; then
                mode=host
            else
                echo "cannot reach PostgreSQL: the container '$PG_CONTAINER' is not running and psql is not on the PATH." \
                     "Start the container (docker compose up -d postgres; PG_CONTAINER names it) or install the PostgreSQL client" \
                     "and set NEWSHAIYA_PSQL=host" >&2
                return 1
            fi ;;
        *) echo "NEWSHAIYA_PSQL='$mode': use docker, host or auto" >&2; return 1 ;;
    esac
    _dbconn_mode="$mode"
    echo "$mode"
}

# Runs psql with -v ON_ERROR_STOP=1 -q and the given arguments; stdin is passed through.
psql_cmd() {
    local mode
    mode="$(psql_mode)" || return 1
    _dbconn_mode="$mode"
    _dbconn_parse_url || return 1
    if [ "$mode" = docker ]; then
        docker exec -i "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -v ON_ERROR_STOP=1 -q "$@"
    else
        psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -q "$@"
    fi
}
