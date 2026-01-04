# RING0

RING0 is a modern terminal emulator for Windows.

The name **RING0** is a reference to the x86 CPU privilege model, where Ring 0 represents the kernel execution level.
It is used here as a technical reference rather than a claim of elevated privileges.

The name reflects the idea of a terminal positioned close to system-level concepts:
direct, controlled, and without unnecessary abstraction.

---

## Project overview

RING0 is a terminal emulator, not a shell.

It hosts existing shells (PowerShell, cmd, WSL) inside a modern, GPU-accelerated window.
RING0 does not interpret commands, manage sessions, or provide scripting facilities on its own.

Version 0.1 focuses on correctness, clarity, and architecture rather than features.
PowerShell is launched without PSReadLine to avoid ANSI cursor control sequences that v0.1 does not yet parse.

---

## Goals (v0.1)

- Spawn and control a shell using Windows **ConPTY**
- Display shell output in a window using a monospaced font
- Forward keyboard input to the PTY
- Handle window resize → PTY resize
- Provide a clean, modular Rust architecture suitable for long-term evolution

---

## Non-goals (v0.1)

The following are intentionally out of scope:

- Tabs or split panes
- Full ANSI / VT support (colors, cursor movement, scroll regions)
- Selection, clipboard, or search
- Theming, blur, transparency, animations
- Plugin or extension system
- Packaging or installers

These features are tracked separately and must not leak into v0.1.

---

## Target platform

- Windows 11 (Windows 10 best-effort)
- Rust stable toolchain only
- GPU rendering via `wgpu`

## Prerequisites

- Cascadia Code font recommended (used for terminal rendering)
  - If missing, the app prompts inside the terminal window to download it from the RING0 repository (requires user consent and network access).
  - If you decline, it falls back to Consolas (or Lucida Console if needed).
  - Downloaded fonts are cached under `%LOCALAPPDATA%\RING0\fonts`.
  - Manual options: Microsoft Store ("Cascadia Code") or https://github.com/microsoft/cascadia-code/releases

---

## Repository documents

- **AGENTS.md** — rules for automated agents (Codex) and contributors
- **SPEC.md** — functional scope and constraints
- **ARCHITECTURE.md** — system design and crate boundaries
- **DECISIONS.md** — design decisions and rationale
- **CONTRIBUTING.md** — development workflow
- **SECURITY.md** — security and data-handling principles
- **docs/BACKLOG.md** — post-v0.1 roadmap
- **PLAN_v0.2.md** — planned v0.2 scope

These documents are normative.
Code must conform to them.

---

## Status

RING0 is currently in **design and bootstrap phase**.
No guarantees of stability are made yet.
