# 1. PTY Lifecycle Protection and Process Isolation

## Context

Spiritty maintains an interactive root shell process (`$SHELL`) in the live terminal panel while allowing the AI agent to inject, execute, and capture diagnostic commands. When proposed commands include scripts with `exit`, `set -e`, or blocking network operations (such as unconstrained `ssh` sessions), injecting them directly into the root shell risks either immediately terminating the master PTY or leaving hanging background processes that intercept subsequent input.

## Decision

1. **Subshell Isolation for Destructive Controls**: Any command or multi-line script containing `exit`, `set -e`, `<<` heredocs, or shell-specific constructs is automatically wrapped in an isolated non-interactive subshell (`bash -c '...'`). The exit code and output are captured cleanly without terminating the parent interactive shell.
2. **Immediate SIGINT on Cancellation**: Whenever an agent tool capture or generation is halted by the user (`Esc`, `Ctrl+C`, or a new chat turn), Spiritty sends a raw `\x03` (`Ctrl+C`) byte to the active PTY. This immediately kills any foreground process that stalled the pipeline and returns the terminal to a clean interactive prompt.

## Consequences

- Prevents accidental exit of the root PTY session and unexpected application termination.
- Ensures hanging or misbehaved diagnostic commands do not lock the terminal or corrupt future tool captures.
- Environmental side effects (such as `export FOO=bar` or `cd`) executed inside an isolated subshell do not persist in the parent interactive shell, which is an accepted trade-off to guarantee stability.
