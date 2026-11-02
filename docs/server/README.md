# server: running this server

How the new server is put together and run: what each program does, every setting, how to deploy it, where it knowingly differs
from the original, and the review of its robustness against hostile clients. The behaviour of the original it follows is in
[`../original/`](../original/README.md).

- [`architecture.md`](architecture.md): the programs, how a login reaches the world, the game server's inside, the database schemas.
- [`settings.md`](settings.md): every environment variable with its default; the ones that change gameplay default to the original.
- [`deploy.md`](deploy.md): running it on a VPS (the run scripts, the service units, the firewall).
- [`deviations.md`](deviations.md): every place the server differs from the original, by kind, with its setting.
- [`audit.md`](audit.md): the robustness audit of `crates/` and what was done about each finding.
