# Native SSH jump routes

Reuse connect_hop/JumpHandles, configured hop order, existing validation,
credentials, per-hop trust, cancellation and connection deadlines. Each handler
uses isolated known_hosts, including carriers. Move credentials once and retain
carriers in native ownership through channel/SFTP destruction. Drop target then
reverse-order hops. Preserve completion and close budgets. No schema or auth
policy changes.

Proof: in-process route success, separate trust/auth, cancellation, validation,
target SFTP and downstream-first teardown; native regression and full CI.
