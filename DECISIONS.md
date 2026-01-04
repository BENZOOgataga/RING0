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

---

### 2026-01-03: Bitmap text rendering for v0.1

- Renderer uses font8x8 bitmap glyphs scaled to 8x16 cells.
- Text is drawn into a CPU RGBA buffer and uploaded to a wgpu texture each frame.
- Cell size drives screen grid sizing and PTY resize.

Reason: keep rendering deterministic and asset-free while meeting monospaced text requirements.

---

### 2026-01-03: Scrollback and default sizing

- Screen keeps up to 1000 lines of scrollback.
- Mouse wheel scrolls the view; typing snaps back to the bottom.
- Window default size targets 120x30 cells with 12px padding on each side.

Reason: match common terminal defaults and enable basic scrolling without introducing styling features.

---

### 2026-01-03: Modern text rendering

- Renderer uses fontdue to rasterize Cascadia Code when available, with Consolas fallback.
- Cell size increased to 10x20 with 12px padding to improve readability.
- Background and text colors updated for a softer, modern contrast.
- Cursor renders as a blinking bar when not scrolled.
- Glyphs are aligned using font line metrics to keep a consistent baseline.

Reason: improve legibility without adding external assets or breaking the render pipeline.

---

### 2026-01-03: Cascadia Code download prompt

- If Cascadia Code is missing, the app shows an in-window prompt asking for consent to download it.
- Downloads come from the RING0 repository `install/Cascadia_Code.zip` and are cached under `%LOCALAPPDATA%\RING0\fonts`.
- If the user declines, the app continues with Consolas (or Lucida Console) fallback.

Reason: keep the default experience modern while requiring explicit user consent for network access.

---

### 2026-01-03: Disable PSReadLine in v0.1

- PowerShell is launched with PSReadLine removed.

Reason: PSReadLine uses ANSI cursor control sequences that v0.1 does not parse yet, causing display/input desync.
