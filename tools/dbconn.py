"""How the tools reach the databases: one set of rules for every import script (standard library only).

- `.env` at the repository root is read for any variable not already in the environment (`KEY=value` lines, `#` comments,
  optional quotes), as the servers read it.
- `PG_CONTAINER` (default `newshaiya-postgres`) and `MSSQL_CONTAINER` (default `newshaiya-mssql`) name the containers.
- The PostgreSQL user, password, host, port and database come from `DATABASE_URL`.
- `NEWSHAIYA_PSQL` = `docker` | `host` | `auto` (default `auto`): `docker` runs psql inside `PG_CONTAINER`; `host` runs the host's
  `psql "$DATABASE_URL"`; `auto` uses `docker` when that container is running, else `host` when `psql` is on the PATH.

`tools/dbconn.sh` follows the same rules for the shell scripts.
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent.parent
_mode_cache: str | None = None


def _load_env() -> None:
    path = ROOT / ".env"
    if not path.is_file():
        return
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        if key.startswith("export "):
            key = key[len("export "):].strip()
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        if key and key not in os.environ:
            os.environ[key] = value


_load_env()

PG_CONTAINER = os.environ.get("PG_CONTAINER") or "newshaiya-postgres"
MSSQL_CONTAINER = os.environ.get("MSSQL_CONTAINER") or "newshaiya-mssql"


def database_url() -> str:
    url = os.environ.get("DATABASE_URL")
    if not url:
        sys.exit("DATABASE_URL is not set: copy .env.example to .env and fill it in (README step 1)")
    return url


def pg_params() -> dict:
    """user, password, host, port and dbname from DATABASE_URL."""
    parts = urlsplit(database_url())
    return {
        "user": unquote(parts.username or "postgres"),
        "password": unquote(parts.password or ""),
        "host": parts.hostname or "127.0.0.1",
        "port": parts.port or 5432,
        "dbname": unquote(parts.path.lstrip("/")) or "postgres",
    }


def _container_running(name: str) -> bool:
    if not shutil.which("docker"):
        return False
    out = subprocess.run(["docker", "inspect", "-f", "{{.State.Running}}", name], capture_output=True, text=True)
    return out.returncode == 0 and out.stdout.strip() == "true"


def psql_mode() -> str:
    """`docker` or `host`, by NEWSHAIYA_PSQL."""
    global _mode_cache
    if _mode_cache:
        return _mode_cache
    mode = (os.environ.get("NEWSHAIYA_PSQL") or "auto").strip().lower()
    if mode not in ("docker", "host", "auto"):
        sys.exit(f"NEWSHAIYA_PSQL={mode!r}: use docker, host or auto")
    if mode == "auto":
        if _container_running(PG_CONTAINER):
            mode = "docker"
        elif shutil.which("psql"):
            mode = "host"
        else:
            sys.exit(f"cannot reach PostgreSQL: the container {PG_CONTAINER!r} is not running and psql is not on the PATH. "
                     f"Start the container (docker compose up -d postgres; PG_CONTAINER names it) or install the PostgreSQL client "
                     f"and set NEWSHAIYA_PSQL=host")
    _mode_cache = mode
    return mode


def psql_cmd(extra_env: dict | None = None, strict: bool = True) -> list[str]:
    """The psql command line (without -c or the input), with `-v ON_ERROR_STOP=1 -q` when `strict`; `extra_env` is set for psql
    (`docker exec -e` inside the container, `env` on the host)."""
    flags = ["-v", "ON_ERROR_STOP=1", "-q"] if strict else []
    pairs = [f"{key}={value}" for key, value in (extra_env or {}).items()]
    if psql_mode() == "docker":
        p = pg_params()
        return (["docker", "exec", "-i"] + [x for pair in pairs for x in ("-e", pair)]
                + [PG_CONTAINER, "psql", "-U", p["user"], "-d", p["dbname"]] + flags)
    return (["env"] + pairs if pairs else []) + ["psql", database_url()] + flags
