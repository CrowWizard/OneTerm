# Native SSH forwarding

Reuse Local/Remote/Dynamic config, target handler, start_forwards and handle
request service. Preserve loopback defaults, explicit endpoints, warning-on-bind
failure and agent-forwarding rejection. Native session cancellation owns all
listeners/relays; observe their destruction before completing target/carriers.
Preserve five-second pane/window and 200 ms app-quit budgets. No implicit binds,
credentials, schema or dependency changes.

Proof: in-process traffic for all three kinds, occupied binds, cancellation and
listener/relay destruction, strict native regression and full CI.
