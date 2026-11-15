# PowerShell 7 executable setting

US-0250 reuses LocalShellConfig.program and the existing SettingField::input
get/set/default/persist path. The row reads Startup Executable for Pwsh and
Custom Program otherwise, with specific Pwsh blank/administrator help. No
file picker, arguments field, schema change or separate per-kind profile is added.

Settings changes notify and persist through TerminalSettings. Changing kind
clears program as before; selecting the same kind preserves it. Explicit new-tab
preset selection preserves program only when its kind matches configured kind.
Other settings remain intact. Existing sessions are not restarted.

Ordinary resolve_shell already prefers program before PATH and retains
powershell_init/-NoLogo/-NoExit/-Command. A bad explicit path fails normal spawn;
it is never silently replaced. Both native and production transports share this
resolver. Restricted resolution still discards program/args/env/cwd and uses
trusted_shell_config. M3/M4 and isolated test configuration remain unchanged.
