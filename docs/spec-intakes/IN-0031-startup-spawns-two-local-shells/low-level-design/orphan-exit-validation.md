# Orphan probe exit validation

Owns: US-0247 (test-only validation seam).

The probe still captures only newly created direct children, holds process handles
before session drop, and terminates only measured survivors after five seconds.
Production teardown and DEC-0016 remain unchanged.

QueryFullProcessImageNameW on the already-open handle supplies identity. A failed
query fails the fixture, never silently drops a watched process. Compare the full
image path against SystemRoot/System32/conhost.exe, case-insensitively. A foreign
same-name image is not a system console host. Resolve the expected system directory
using GetSystemDirectoryW, not an inherited environment override.

Keep the original exit whitelist for all processes. Additionally accept
ERROR_BROKEN_PIPE only for the verified system console host. Shells, missing
identity and unknown images do not receive that exception. Unknown host exit
codes still fail. Role and code are independent pure decision inputs, covered by
negative tests. This does not assert a bundled host's behavior or broaden process
termination authorization.

The data is fixture-local: Watched retains the queried role and Rows copy it for
validation/diagnostics. No persistent state, public API, schema or UI changes.

Proof: decision tests, original serial orphan regressions, full local-shell suite,
then bundled gate. Preserve every observed failure; no time-bound changes or
test skips are allowed.
