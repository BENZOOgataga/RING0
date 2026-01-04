# PLAN.md
## RING0 – Agent Execution Plan

This document is the **single authoritative execution plan** for the agent responsible for building RING0.

You must read **this file first**, then read all referenced documents, and only then start writing code.

Failure to follow this plan is considered an error.

---

## 0. Context and authority

Project name: **RING0**  
Project type: **Windows terminal emulator**  
Language: **Rust (stable only)**  
Target platform: **Windows 11**

RING0 is a terminal emulator, not a shell.

The name RING0 is a reference to the x86 CPU privilege model.
It does **not** imply kernel access or elevated privileges.
Do not claim, imply, or suggest otherwise anywhere in the code or documentation.

This repository already contains **normative documents**.
They are not suggestions.

---

## 1. Documents you MUST read (in this order)

Before writing any code, you must read and internalize:

1. `README.md`  
   Project framing, goals, and non-goals.

2. `AGENTS.md`  
   Rules governing your behavior, scope discipline, and quality bar.

3. `SPEC.md`  
   Functional scope of version 0.1.  
   Anything not listed there is out of scope.

4. `ARCHITECTURE.md`  
   Mandatory layering and crate responsibilities.

5. `DECISIONS.md`  
   Existing design decisions you must respect.

6. `CONTRIBUTING.md`  
   Workflow and expectations.

If any document appears contradictory, **do not guess**.
Add a note to `DECISIONS.md` and proceed conservatively.

---

## 2. Your role

You are acting as:

- A senior systems engineer
- A long-term maintainer
- A technical lead, not a prototype author

Your goal is to create a **clean, understandable, extensible codebase**, not a demo.

You optimize for:
- clarity
- correctness
- architecture
- future evolution

You do NOT optimize for:
- feature count
- cleverness
- visual polish
- shortcuts

---

## 3. Scope: what you are allowed to build (v0.1)

You are authorized to implement **only** the following:

- A Rust workspace
- A PTY backend using Windows ConPTY
- A minimal VT layer:
  - printable characters
  - newline
  - carriage return
  - backspace
- A screen/grid model
- A renderer that displays monospaced text in a window
- Keyboard input forwarded to the PTY
- Window resize → PTY resize

Anything else must be deferred.

---

## 4. Explicit non-goals (do NOT implement)

You must not implement or partially implement:

- Tabs or split panes
- Selection or clipboard
- ANSI colors or styling
- Cursor addressing
- Themes, blur, transparency
- Plugin or extension system
- Configuration UI
- Installers or packaging

Do not “prepare” these features in code.
Only leave **clean extension points** where appropriate.

---

## 5. Mandatory architecture

You must follow the exact pipeline:

`PTY → VT → Screen → Renderer → Window`


Rules:

- Each layer has one responsibility
- No backward dependencies
- No layer skipping
- No global mutable state

Crate responsibilities (non-negotiable):

- `pty`      – process lifecycle, IO, Windows-specific
- `vt`       – byte stream → semantic events
- `screen`   – terminal state (grid, cursor)
- `render`   – drawing only (wgpu-based)
- `app`      – orchestration and event loop
- `config`   – data + validation only (can be minimal or stubbed)

---

## 6. Technical constraints

- Rust stable only (no nightly)
- No `unwrap()` / `expect()` outside tests
- Use `thiserror` for library errors
- Use `anyhow` in the application layer
- Use `tracing` for logging
- No telemetry
- No network calls (except if the user tells the opposite)

Platform-specific code must be isolated.

---

## 7. Development strategy

You must work in **small, incremental steps**, equivalent to clean pull requests.

Recommended order:

1. Workspace bootstrap
2. PTY crate (spawn, read, write, resize)
3. Window creation + basic render loop
4. VT minimal parser
5. Screen model
6. Integration of all layers
7. Documentation updates

After each step:
- Code builds
- Application launches
- Documentation remains accurate

---

## 8. Testing expectations

Testing is about confidence, not coverage.

Required:
- Application builds on Windows
- Manual smoke test:
  - shell spawns
  - input works
  - output appears
  - resize works

Encouraged:
- Unit tests for pure logic
- Compile-time checks for Windows-only code
- Simple scripts under `scripts/`

---

## 9. Documentation duties

Whenever you:
- introduce a new module
- make a design choice
- defer a feature

You must update one of:
- `ARCHITECTURE.md`
- `DECISIONS.md`
- inline module documentation

Documentation must never lag behind code.

---

## 10. When in doubt

When you are unsure:

1. Choose the simplest correct solution
2. Avoid speculative abstractions
3. Document the decision
4. Defer features rather than half-implementing them

---

## 11. Definition of success

Your work is successful if:

- A human can understand the codebase without reverse engineering intent
- Each layer is clearly separated
- The system can realistically evolve into a full terminal emulator
- Nothing exists that “should probably be rewritten later”

If something feels hacky, stop and redesign.

---

## 12. Final instruction

Do not start coding until you have read and understood this plan and all referenced documents.

RING0 values **clarity, restraint, and correctness**.

Proceed accordingly.
