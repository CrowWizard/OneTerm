# Detail Design: Native drag priority

## Contract

`MouseDragPriority` belongs to settings and serializes as `tui_mouse` or
`selection`. An absent field defaults to TUI-first without schema migration.
The live setting and config map both directions through the existing snapshot
and background persistence path. Older builds ignore the added field and drop
it on their next save; all prior preferences remain compatible. Invalid values
follow the existing typed configuration recovery policy, not silent rewriting.

## UI and ownership

Use the existing Mouse settings group dropdown, labelled Drag Priority, with
Selection First and TUI Mouse First options. Describe that it applies to native
terminal sessions and that automatic clipboard copying still requires Copy on
Select. Do not alter the user's current stored settings during implementation.

The native selection admission gate combines physical Shift with the
selection-first preference. No synthetic modifiers reach the application.
Once admitted, the existing drag task owns move/release through Finish; changing
the preference mid-drag affects only the next gesture. With TUI-first, native
mouse press/release pairing remains unchanged. No reporting request means
ordinary local selection in either mode. Right/middle buttons, wheel, cursor
motion without a drag, IME, clipboard policies and legacy paths are unchanged.

## Verification

Cover absent-field compatibility, both JSON values and settings round trips.
Extend native mouse-reporting selection coverage for Shift/TUI-first and plain
selection-first, including setting changes during a drag. Verify ordinary
TUI-first reporting admission and independent copy-on-select behavior. Local
build/tests are skipped at owner request; static checks and unexecuted focused
regressions do not claim Windows acceptance.
