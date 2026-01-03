## RING0 Architecture

RING0 is designed as a layered system with strict responsibilities.

---

## Data flow

`PTY → VT → Screen → Renderer → Window`


- PTY produces bytes
- VT parses bytes into events
- Screen updates terminal state
- Renderer draws state
- Window presents result

---

## Crate boundaries

- `pty`: ConPTY wrapper, Windows only
- `vt`: ANSI/VT parsing, pure logic
- `screen`: grid, cursor, scrollback
- `render`: GPU text rendering
- `app`: event loop and orchestration
- `config`: configuration loading

No crate may violate these boundaries.

---

## Extension points

- VT event enum
- Screen model traits
- Renderer backend trait

These are for future versions only.
