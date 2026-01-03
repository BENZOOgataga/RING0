# STYLE.md
## RING0 – Visual and Interaction Style Guide

This document defines the **intended visual and interaction style** of the RING0 terminal.

It is a **design reference**, not an implementation mandate.
No part of this document overrides `SPEC.md`.

Unless explicitly stated otherwise, everything described here applies to **v0.2 and later**.

---

## 1. Status and scope

- This document is **informational** for v0.1.
- No visual styling beyond basic text rendering is expected in v0.1.
- Any implementation of styles, effects, panels, or theming is **out of scope for v0.1**.

The purpose of this document is to:
- Align long-term visual direction
- Avoid accidental design drift
- Prevent premature UI decisions in early code

---

## 2. High-level inspiration

RING0’s visual direction is **inspired by eDEX-UI**, but **not a replica**.

Key distinction:
- eDEX-UI is a *full-screen system dashboard*
- RING0 is a *terminal emulator*

The inspiration is conceptual, not literal.

---

## 3. Core design principles

### 3.1 Technical, not theatrical

- No “hacker aesthetics”
- No fake system metrics
- No decorative noise

Every UI element must have:
- a purpose
- a data source
- a clear justification

---

### 3.2 Minimal first, layered later

The terminal content is always the primary focus.

Any additional UI (panels, overlays) must:
- be optional
- be collapsible
- never obscure core terminal usage

---

### 3.3 Information density over decoration

- Compact layouts
- Tight spacing
- Clear alignment
- No excessive padding

RING0 should feel **dense but readable**, not “spacious”.

---

## 4. Color philosophy

### 4.1 Base palette

- Dark background by default
- Neutral, low-saturation base
- Accent colors used sparingly

No “neon everywhere”.

### 4.2 ANSI colors

- ANSI colors should be configurable
- Default palette should be subdued and readable
- High contrast must be preserved for accessibility

### 4.3 Backgrounds

- Solid color by default
- Optional subtle gradients or textures later
- No background images by default

---

## 5. Typography

### 5.1 Fonts

- Monospaced fonts only for terminal content
- Clear glyph distinction (0/O, 1/l/I)
- Ligatures optional and disabled by default

Examples (non-binding):
- JetBrains Mono
- Fira Code
- Cascadia Mono

### 5.2 Metrics

- Stable line height
- Predictable character cell size
- DPI-aware scaling

Text rendering must prioritize correctness over effects.

---

## 6. Layout and structure (future)

### 6.1 Core terminal area

- Always dominant
- Full-height by default
- No overlays unless explicitly enabled

### 6.2 Optional panels (v0.2+)

Inspired by eDEX-UI, but constrained:

Possible panels:
- System info
- Session metadata
- Logs or diagnostics

Rules:
- Panels must be toggleable
- Panels must not be required for basic usage
- Panels must never fake data

---

## 7. Effects and animations

### 7.1 Allowed effects (optional)

- Subtle cursor animations
- Soft glow on cursor or text (very restrained)
- Smooth scrolling

### 7.2 Forbidden or discouraged

- CRT scanlines by default
- Heavy bloom
- Animated noise
- Constant motion UI elements

Motion must:
- be minimal
- be optional
- respect reduced-motion settings

---

## 8. Accessibility

- Sufficient color contrast
- No critical information conveyed by color alone
- Motion can be disabled entirely
- Keyboard-only usage must remain first-class

Accessibility is not optional.

---

## 9. Interaction philosophy

- Keyboard-first
- Mouse as a complement, not a requirement
- No hidden gestures
- No “Easter egg” interactions

Everything must be discoverable or documented.

---

## 10. Implementation guidance (non-binding)

When style implementation begins (v0.2+):

- Visual styling belongs primarily in `render/`
- Layout and UI chrome belong in `app/`
- Styling must not leak into:
  - `pty`
  - `vt`
  - `screen`

Rendering effects must be:
- modular
- disableable
- data-driven

---

## 11. Explicit non-goals

RING0 is **not**:

- A system monitor
- A fake cyber dashboard
- A full-screen mandatory UI
- A replacement for desktop environments

The terminal remains the product.

---

## 12. Summary

RING0’s style should feel:

- Technical
- Controlled
- Intentional
- Calm

Inspired by eDEX-UI’s clarity and density,
without copying its spectacle.

If a visual element does not improve usability or understanding,
it does not belong in RING0.
