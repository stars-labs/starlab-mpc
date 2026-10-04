# Product closeout and verification

User objective: improve the product, exercise its tests and finish the work.
This follows the accepted Rabby/MetaMask/Phantom visual review, token icons,
glass/light interactions and compact layouts across the browser and desktop.

## Required final state

- Main wallet flows remain usable: create/join, wallet/account/network
  selection, signing and approval/decline, lock/reopen, backup/restore,
  connection failure and retry. Ethereum Send remains available through the
  extension; the desktop remains a message-signing and approval client.
- Ordinary and adversarial display values remain readable in light/dark,
  popup/wide and normal/narrow layouts. Keyboard focus stays within an active
  password dialog and returns to its opener. Reduced motion and actual press
  states retain their existing behavior.
- Exposed wallet deletion actually deletes the exact wallet and its owned
  child shares, retains unrelated wallets and reports failures without
  falsely updating the visible state. In-flight operations remain coherent.
- Native ECDSA control authorization and stale-session branches have targeted
  regression coverage. Preserve the public native desktop and WASM contracts.
- Resolve the implementation plan's unused-prime cache requirement: native
  clients currently pre-generate in memory only, while the browser persists
  encrypted unused primes. Native persistence must protect secret primes,
  prevent cross-process reuse and degrade safely on invalid/unavailable cache.
- The documented ordinary build commands work. Current source produces a
  production extension ZIP without the insecure test-prime hook and a current
  desktop release binary. Source, artifact and evidence provenance agree.
- Current engine and extension CI pass, and desktop has a locally verified
  workflow that also passes remotely after publication. User documentation
  describes actual installation and supported functions, including hardware,
  Firefox and Ethereum share-refresh limitations honestly.
- Existing real three-client Sepolia evidence remains a separately pinned
  historical transaction. Presentation or packaging tests do not claim a new
  transaction or validation on unavailable physical hardware.

## Initial audit

The prior engine head `a41cba9` has eight passing CI jobs; extension head
`15e2db1` has three passing jobs. Desktop head `4ef4e1e` has no remote CI.
The compact extension evidence covers only 16 representative states, while
the earlier 230-case gallery predates compact layouts. Compact desktop
evidence uses debug snapshots; its release binary predates that spacing.

Reproduced issues found during closeout include the isolated Bun types
compiler resolution, overlapping long asset names/large numbers, password
dialog keyboard escape from the modal, terminal desktop connection failure
without a reachable retry, and the TUI's unimplemented deletion command.

## Verified engine delivery

Engine source `031e81f3177a839eebdd7dfa48e8710e2d999222` passes all eight
remote CI jobs in run 37219648344. Local client tests pass 296/296, including
control authorization, stale epochs, exact encrypted-wallet deletion, failures
and retries, and fifteen prime-cache tests. Independent child processes prove
that one persisted prime set is claimed at most once. Cache corruption, wrong
password, publication/claim failures and occupied leases fall back safely.

The unused-prime cache uses device-bound encrypted storage and atomic leasing,
with deletion and directory synchronization before use. Linux runtime is
verified. Persistence is disabled on non-Unix platforms; those platforms use
fresh memory-only primes. macOS cache runtime remains unverified. No core or
core-wasm source file changed from the established `fab8e166` crypto baseline.

The ordinary locked optimized CLI and TUI build completes successfully in
6m53s. Binary SHA-256 values:

- CLI: `f878e40cccffe8dbd8a46d304ef566e6980c0453aa8328a46cb081170a9e84e0`
- TUI: `2927cb3a4eedae5b0ba1f926ae67683aafc1039dc20aea75ccd76899cdd505fc`

Formatting and normal Clippy pass. Strict engine Clippy retains existing
workspace warnings and is not claimed green. Local logs are
`/tmp/starlab-native-prime-cache-client-reviewed.log`,
`/tmp/starlab-native-prime-cache-clippy-final.log` and
`/tmp/starlab-closeout-native-release.log`.

## Consumer verification in progress

The extension preserves all original 230 visual scenarios and adds twelve,
with 242 unique successful captures and no horizontal overflow. Its evidence
records three presentation versions separately rather than claiming one
uninterrupted final-version run. Normal build/ZIP and actual Chromium provider
initialization pass. Desktop final engine pin passes fifty local tests and
strict Clippy; its final release and remote workflow are still being verified.
Both clients pin the engine source above.

Browser hardware signing also requires a local GUI PIN prompt: the old native
host depended on an unavailable terminal. The isolated host fix has unit,
framed-process and release-build evidence; physical card verification remains
unavailable. Firefox build success is not Firefox crypto-runtime verification.

Current consumer remote checks and final desktop artifacts are still pending.
This acceptance scope is not yet a claim of complete product closeout.
