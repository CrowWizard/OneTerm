# Native OSC 52 clipboard

## Ownership and accepted design

Use pinned ghostty_terminal_vt_write_until OSC start/end boundaries from the
native parser, not a second VT engine. Retain only bounded OSC bytes across
reads. On a completed non-cancelled OSC 52, decode a bounded text payload or
emit a read event. Capture events in stream order with title/directory/bell.
Keep native synchronous clipboard callbacks disabled: they cannot await GPUI
consent and return an empty read response before the existing asynchronous reply.
No borrowed pointer or callback reply closure escapes a native call.

OscRouter applies existing local/remote policy. Remote reads/writes remain
default-deny. The native pane checks live read permission and replies only
through the requesting session, never paste/fan-out; exit/shutdown suppresses
side effects. Malformed base64, invalid UTF-8, cancelled and oversized strings
produce no clipboard operation. Storage, dependencies and close budgets do
not change. Other clipboard protocols remain disabled.

## Proof

All fragmentation splits; BEL/ST and cancellation; bounds and malformed data;
local allow, remote deny/allow; no premature native reply; originating-session
reply and shutdown regression. Run native affected-crate tests, strict native
Clippy and full bundled CI before completion.
