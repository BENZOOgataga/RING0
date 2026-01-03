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
  - carriage r
