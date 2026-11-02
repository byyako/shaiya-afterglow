# Security policy

## Reporting a vulnerability

Please report a vulnerability privately, not in a public issue. On the repository's page, open the **Security** tab and choose
**Report a vulnerability** (GitHub's private vulnerability reporting). Only the maintainer sees the report.

Useful in a report: what you found, how to reproduce it (the packets or commands, the settings), which version or commit, and what
an attacker gains. Please do not attach files from the original game or server (see `CONTRIBUTING.md`); describe where in them
the problem is instead.

You will get an answer as soon as the maintainer can give one; this is a volunteer project, so there is no promised time. A fix is
made in private where it can be, and the report is closed with credit to you if you want it.

## What counts

The programs here are meant to be run on a network by strangers' clients, so these are in scope: anything a connected or unconnected
client can do to crash, hang, flood or corrupt a server, duplicate items or gold, take over an account or a session, or reach data
it should not (the login server, the game server, the data editor's page, the console and `NOTIFY` channel, the operator tools).
`docs/server/audit.md` is the project's own robustness audit and shows what has been looked at already.

Not in scope: bugs of the original game that the server reproduces on purpose (`docs/server/deviations.md` lists what it fixes and
what it keeps), and the original client's own weaknesses.

## Supported versions

Only the latest release and the main branch.
