## RING0 – Decisions log

### 2026-01-03: Language and platform

- Rust (stable)
- Windows only
- ConPTY for PTY

Reason: safety, performance, correct Windows integration.

---

### 2026-01-03: Minimal VT in v0.1

Only newline, carriage return, backspace, printable chars.

Reason: VT is complex; architecture comes first.

---

### 2026-01-03: Name “RING0”

Technical reference to x86 privilege model.
No implication of elevated privileges.
