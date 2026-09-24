---
bump: patch
---

cli: every failure after argument parsing now writes exactly one CliResponse error envelope to stderr, whether it happened during startup (unreachable remote locator, unsupported future on-disk version, missing data file, unregistered backend) or inside a command handler. Argument-parsing errors keep clap's plain-text output and exit code 2.
