# Work: Release cleanup and source publication

ID: US-0260
Intake: IN-0057
Created: 2026-11-16

## Classification

- Change type: maintenance
- Risk lane: local artifact deletion and authorized source publication

## Outcome

Keep the accepted final native Release and its runtime dependencies/data,
remove target build intermediates, and publish the current implementation,
tests and owning documentation to the configured GitHub origin.

## Scope

- In scope: target/debug, generated docs, target temp and Release build caches.
- Preserve final EXE, ConPTY DLL/helper, local runtime data and shortcut.
- Exclude all target/.tmp/.cargo-home output and secrets from Git.
- Do not delete source, local research/build inputs, verification evidence,
  user settings or credentials; do not change runtime behavior or cut over
  production engine defaults.

## Acceptance

- [x] Final EXE hash unchanged; settings/credentials match prior smoke, with only normal user layout saves differing.
- [x] Only final runtime delivery files remain under target/release.
- [x] No generated build or runtime data files enter the commit.
- [x] Authorized source commit is present on the configured origin.

## Documentation

### Owning Docs Reviewed

- AGENTS.md, docs/HARNESS.md and docs/templates/work.md: routing and safety.
- docs/agents/code-style.md and structure.md: commit and source ownership.
- .gitignore: target/.tmp/.cargo-home already ignored; no rule change needed.
- crates/app/build.rs: required ConPTY runtime files beside the executable.
- US-0249-native-workspace.md: accepted Release hash and verification evidence.

### Documentation Action

No runtime contract change. Record cleanup/publication evidence here.
Harness tool was unavailable in prior discovery; no database status is claimed.

## Plan

- [x] Inspect paths and hash preserved runtime files.
- [x] Remove explicit contained build-intermediate paths and verify hashes.
- [x] Audit/stage source, inspect commit inventory and publish normally.

## Verification Plan

Reuse the completed full CI, native tests (1,305 pass, 7 existing ignored),
strict native Clippy and Release smoke for the unchanged implementation.
Do not rebuild after cleanup merely to recreate removed intermediates.
Check source whitespace, ignore rules, staged paths, file sizes and potential
secret material before commit; verify remote commit identity after push.

## Evidence and Gaps

Expected final EXE SHA256:
45B15423B49BA5C0309CDCCDE81792DD0875A27784211BEFA22A620DE145178C.
Long-duration/network-interruption testing remains usage follow-up.
Known post-close window-not-found diagnostic is not fixed by this task.

Cleanup completed on 2026-11-16. The first attempt stopped at a hardlinked
build executable while the owner was running the final application (PID
26044). The owner exited normally; no process was terminated by the agent.
The retry removed the remaining Release intermediates. target now contains
only release; release contains the final EXE, conpty.dll, x64/OpenConsole.exe,
oneterm-native-data and the existing shortcut. Debug, generated documentation,
build/deps/examples/incremental, PDB and Cargo intermediate files are removed.

Evidence: .tmp/release-cleanup-result.json and
.tmp/release-cleanup-final-manifest.json (22 retained files). Final EXE SHA256
matches the accepted value above. The initial Windows separator filter
recorded only three root files, so it is not evidence of a full pre/post
nested-data comparison. All 18 runtime-data files were instead checked
against the prior Release smoke: SSH, trust, vault and settings match; only
docks.json/docks.bak differ following normal user use and exit. All final
manifest hashes were rechecked. No runtime directory was a deletion target.

The 285 changed/new source, test and documentation files were audited: no
file above 5 MB, detected private-key/token signatures, or generated/runtime
paths. target, .tmp and .cargo-home remain ignored and untracked. Existing
.tmp backups/evidence/build inputs and .cargo-home were deliberately retained.
No rebuild was run after cleanup; the implementation had already passed the
full gates recorded in US-0249.

Source publication PASS: commit
708870c990b5160e5646e1c324c92793886d47aa
(`feat(native): integrate native terminal workspace and directional split drops`)
was pushed normally to origin/main at github.com:CrowWizard/OneTerm.git.
git ls-remote confirmed the remote main identity equals that source commit.
The staged audit covered 285 files, with zero forbidden paths, detected secret
signatures, tracked build files, unstaged changes or untracked publishable
files. Both working and staged whitespace checks passed. No force push,
tag, GitHub Release or binary upload was performed. This documentation-only
publication reconciliation is committed separately after the source push.
