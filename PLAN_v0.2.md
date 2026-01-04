# PLAN_v0.2.md
## RING0 v0.2 Plan

This plan defines the scope for v0.2 features that are explicitly out of scope for v0.1.

---

## v0.2 scope

- Text selection (mouse drag and keyboard)
- Clipboard copy/paste
  - Ctrl+C for copy when selection is active
  - Ctrl+V for paste
  - Right click paste

---

## Notes

- Implement strictly within the existing pipeline (PTY -> VT -> Screen -> Renderer -> Window).
- No new features beyond the listed scope without an updated plan.
