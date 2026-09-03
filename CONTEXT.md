# Spiritty

Spiritty is an AI-powered sysadmin terminal companion pairing an intelligent reasoning agent on the left with an interactive live PTY shell on the right in a split-screen TUI.

## Language

### Terminal & Execution

**TerminalTab**:
An independent virtual terminal instance hosting its own PTY process, virtual VT100 screen buffer, and system context (local or remote SSH/container).
_Avoid_: Window, panel, pane, session tab

**ToolCapture**:
A bounded, asynchronous listening window intercepting PTY output to deliver command results back to the AI agent, resolved by completion sentinel (`OSC 777`) or prompt settle.
_Avoid_: Sniffing, hook recorder, terminal buffer scrape

**InteractionKind**:
A state where a running terminal command requires human input (sudo/password or interactive `[y/n]` confirmation), temporarily commanding keyboard focus.
_Avoid_: Prompt wait, password block, pause state

### Agent & Security

**ApprovalLevel**:
The policy governing human-in-the-loop validation (`Safe`, `Standard`, `Sudo`, `YOLO`, `Off`), determining whether a proposed command runs automatically or requires explicit consent.
_Avoid_: Permission level, execution mode, safety tier

**CommandProposal**:
An executable shell command synthesized by the AI agent, surfaced to the user as an interactive proposal card (`Alt+1..9`) or executed automatically if permitted by the active ApprovalLevel.
_Avoid_: Suggested command, code block, action snippet
