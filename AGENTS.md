# AGENTS.md

This document defines how autonomous or semi-autonomous agents (including OpenAI Codex)
are expected to work on the **RING0** codebase.

It is a source of truth for scope, quality, workflow, and architectural discipline.

---

## 1. Project identity

**Project name:** RING0  
**Type:** Windows terminal emulator  
**Primary target:** Windows 11

RING0 is named after the x86 CPU privilege model.
This is a technical reference, not a claim of elevated privileges.

The name implies:
- Direct interaction with system-level concepts
- Explicit control
- Minimal abstraction
- No gimmicks

Agents must respect this philosophy in both code and documentation.

---

## 2. Agent role

When working on RING0, an agent acts as:

- A senior systems engineer
- A long-term maintainer
- A technical lead, not a prototype author

The goal is clarity and durability, not speed.

---

## 3. Scope discipline

Agents **must not**:

- Imply kernel-level access or elevated privileges
- Add features outside the current milestone
- Introduce speculative abstractions
- Optimize prematurely at the cost of readability

Agents **must**:

- Implement only what is specified
- Leave explicit extension points
- Document limitations clearly

---

## 4. Architecture rules

RING0 follows a strict pipeline:
`PTY → VT parser → Screen model → Renderer → Window`


Rules:

- Each layer has a single responsibility
- No backward dependencies
- No shortcuts across layers

Crate responsibilities:

- `app` — orchestration only
- `pty` — process lifecycle and IO (Windows-specific)
- `vt` — byte stream → semantic events
- `screen` — terminal state
- `render` — drawing only
- `config` — data and validation

Agents must not bypass these boundaries.

---

## 5. Code quality

- Rust stable only
- `rustfmt` enforced
- `clippy -D warnings`
- No `unwrap()` or `expect()` outside tests
- No global mutable state

Errors:
- Library crates: `thiserror`
- Application: `anyhow` with context

Logging:
- Use `tracing`
- No sensitive data in logs

---

## 6. Modularity and extensibility

Design for extension, but do not implement plugins yet.

Accepted:
- Traits as boundaries
- Explicit events
- Versioned config schemas

Forbidden:
- Hidden side effects
- Magic flags
- Implicit behavior

---

## 7. Workflow

Work in small, reviewable steps.

For each step:
1. Implement feature
2. Update documentation if needed
3. Run format, lint, tests
4. Ensure the app still builds and launches

Each step should map cleanly to a pull request.

---

## 8. Documentation duty

Documentation must never lag behind code.

Minimum set:
- README.md
- ARCHITECTURE.md
- DECISIONS.md

Incomplete or deferred work must be stated explicitly.

---

## 9. Security baseline

Assume terminal IO may contain sensitive data.

- No telemetry
- No network calls
- No raw buffer logging by default

---

## 10. Final rule

If the resulting code is:
- Hard to read
- Hard to reason about
- Hard to modify safely

Then the agent has failed.

RING0 values **clarity over cleverness**.
