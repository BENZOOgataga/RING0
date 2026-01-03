# AGENTS.md

This document defines how autonomous or semi-autonomous agents (including OpenAI Codex) are expected to work on the **RING0** codebase.

It is a source of truth for scope, quality, workflow, and architectural discipline.

---

## 1. Project Identity

**Project name:** RING0
**Type:** Windows terminal emulator
**Primary target:** Windows 11 (Windows 10 compatible when possible)

RING0 is named after the x86 CPU privilege model (Ring 0 = kernel level).
This is a *technical reference*, not a claim of elevated privileges.

The name implies:

* Direct interaction with system-level concepts
* Explicit control
* Minimal abstraction
* No gimmicks or misleading promises

Agents must respect this philosophy in both code and documentation.

---

## 2. Agent Role & Expectations

When working on RING0, an agent acts as:

* A **senior systems engineer**
* A **maintainer**, not a prototype author
* A **technical lead** optimizing for long-term clarity

### Non-negotiable expectations

* Production-grade structure (no monolith files)
* Explicit data flow and ownership
* Code that a human can revisit months later without reverse engineering intent

---

## 3. Scope Control

Agents **must not**:

* Claim or imply kernel-level access
* Introduce unnecessary abstraction layers
* Add features outside the current milestone
* Optimize prematurely at the cost of clarity

Agents **must**:

* Implement only what is required for the current step
* Leave extension points instead of speculative systems
* Document limitations explicitly

If scope expansion is tempting, it must be documented in `DECISIONS.md` or deferred to backlog.

---

## 4. Architecture Principles

### Mandatory layering

RING0 follows a strict pipeline:

```
PTY  ->  VT Parser  ->  Screen Model  ->  Renderer  ->  Window
```

Rules:

* Each layer has a single responsibility
* No backward dependencies (renderer never talks to PTY)
* Communication is done via explicit data structures or events

### Crate boundaries

* `app` orchestrates, never implements core logic
* `pty` owns process lifecycle and IO
* `vt` interprets byte streams into semantic events
* `screen` owns terminal state
* `render` is stateless aside from caches
* `config` is pure data + validation

Agents must not bypass these boundaries.

---

## 5. Code Quality Standards

### Language & tooling

* Rust (stable channel only)
* `rustfmt` enforced
* `clippy` with `-D warnings`
* CI must pass before changes are considered valid

### Error handling

* Library crates: `thiserror`
* Application layer: `anyhow` with context
* No `unwrap()` or `expect()` in non-test code

### Logging

* Use `tracing`
* No noisy logs in hot paths
* Logs must be actionable

---

## 6. Modularity & Extension Strategy

Agents must design with **future extension** in mind, but without implementing plugins yet.

### Accepted patterns

* Traits as boundaries (e.g. renderer backend, PTY backend)
* Event-based communication between layers
* Versioned config schema

### Forbidden patterns

* Global mutable state
* Hidden side effects
* Implicit behavior controlled by undocumented flags

If an extension point is added, it must be documented in `ARCHITECTURE.md`.

---

## 7. Development Workflow

Agents must work in **small, reviewable steps**.

For each step:

1. Implement the feature
2. Update documentation if behavior or architecture changes
3. Run formatting, linting, and tests
4. Ensure the application still builds and launches

Each step should conceptually map to a clean pull request.

---

## 8. Documentation Responsibilities

Any agent modifying code must ensure that documentation remains accurate.

Minimum documentation set:

* `README.md`: build & run instructions must never break
* `ARCHITECTURE.md`: updated if structure or flow changes
* `DECISIONS.md`: updated for any non-obvious technical choice

If something is incomplete or intentionally minimal, it must be stated explicitly.

---

## 9. Testing & Validation

### Required

* Build success on Windows
* Manual smoke test (spawn shell, type command, see output)

### Encouraged

* Unit tests for pure logic
* Compile-time tests for platform-specific code
* Scripts in `/scripts` for manual validation

Tests are not about coverage, but about confidence.

---

## 10. Security & Safety

Agents must assume:

* Terminal input/output may contain sensitive data
* Logs must never dump raw terminal buffers by default

No telemetry, no network calls, no data exfiltration unless explicitly approved and documented.

---

## 11. Decision Making

When faced with uncertainty:

* Choose the simplest correct solution
* Document the choice and alternatives in `DECISIONS.md`
* Prefer boring, proven approaches over clever ones

---

## 12. Final Rule

If an agent produces code that is:

* Hard to read
* Hard to reason about
* Hard to modify without fear

Then the agent has failed its role.

RING0 values **clarity over cleverness**, always.
