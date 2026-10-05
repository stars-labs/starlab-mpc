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

## Engine acceptance

Runtime source `18328a1b8f6d04c769818036b2a0b835c58d22bb` passes all nine
[remote CI jobs](https://github.com/stars-labs/starlab-mpc/actions/runs/37225188943).
The full locked local workspace run passes 549 tests with zero failures and
32 default-ignored CLI network tests. All 32 are exercised explicitly by the
remote cold-DKG, e2e, conformance, process and wire shards; the shard logs show
executed test counts rather than ignored-only or empty runs. The production-prime
two-node simulation passes in 129.88 seconds, and actual discovered DKG joining
passes in 3.90 seconds on that remote source.

Client tests pass 304/304. Coverage includes exact encrypted-wallet deletion,
unrelated-wallet preservation, persistence failures and retry, in-flight guards,
READY/SET/DONE authorization, duplicate/cohort conflicts, and stale epochs.
Fifteen prime-cache tests include independent process claiming, so only one
consumer can use a persisted set. Corruption, wrong passwords, occupied leases
and publication/claim failures safely use fresh primes.

Native unused primes are stored in device-bound password-encrypted storage.
Atomic leasing, removal and directory synchronization precede use. Linux runtime
is verified; persistence is disabled on non-Unix systems, which use fresh
memory-only primes. macOS cache runtime is unverified. Cache configuration needs
the DKG password: ordinary blank desktop startup does not prove warm-cache loading.

Cold secp256k1/unified CLI creation and discovered DKG joining use the native
preparation/ceremony budgets. General operations, discovery, signing, refresh and
ed25519-only DKG keep the ordinary default; explicit timeout values are preserved.
DKG failure is returned immediately with the pending CLI command correlation.
Shared failure cleanup closes setup and enables retry for native clients while
preserving unrelated signing and replacement sessions. Guarded late preparation
and persistence results cannot publish into a replacement ceremony. The public
`DKGFailed` payload still has no originating ID; this does not claim arbitrary
already-queued public failure messages can be origin-correlated.

No `core` or `core-wasm` source differs from crypto baseline
`fab8e16634ce4c03d71b00ff9f92bcb058ecc9d4`. Formatting, normal Clippy and
locked optimized CLI/TUI builds pass. Existing workspace warnings prevent a claim
that strict engine Clippy is green. [Machine evidence](../testing/evidence/native-closeout-2026-10-05.json)
records exact test counts, shard coverage and versioned release hashes:

- CLI SHA-256: `4dcba9c4eac9207aa488b63c599a2580a0fe9f64d48b2e08f520a8672acb5308`.
- TUI SHA-256: `8aa63822a1936321fcbe0c5715f82907097b84ac88e3f868762327a01aeb2de9`.

## Browser product acceptance

The extension implements token artwork, compact spacing, translucent glass,
ambient light and hover/press/focus feedback. Ordinary cards avoid extra blur
cost; header, hero and overlays carry the glass treatment. Long asset names
truncate with their full name available, and large balances retain readable
English formatting and exact accessible values. Password/shared dialogs contain
keyboard focus and return it to their opener. NFT and simulation errors offer
plain-language retry. Text sizes and 44px primary actions remain unchanged.

The reviewed gallery retains all original 230 cases and adds twelve: 242 unique
successful light/dark, popup/wide, normal/adversarial-value states, with zero
page/action errors and no horizontal overflow. Provenance deliberately records
224 captures at `cd32ef1`, fourteen at `a0aa53f`, and four at `a4b8ee3`.
This is not claimed as one uninterrupted run or one final-version bundle.
The designer inspected every contact sheet and affected full-size states.

A final wide Settings follow-up at source `1ae6d43` removes grid-row gaps by
stacking cards at their natural height in two CSS columns. Popup stays single
column; text, targets and form logic remain unchanged. Four additional affected
light/dark popup/wide states, with full-page and 600px viewport captures, pass
with no errors or horizontal overflow and 10/12px consecutive card gaps.
[Current Settings/package evidence](https://github.com/stars-labs/starlab-wallet/blob/6b44584/docs/design/evidence/settings-natural-2026-10-05.json)
records this source and its separate current production bundle; the prior 242
captures keep their original pins. Published extension head `6b44584` includes
this source and factual evidence corrections.

Ordinary root build and extension ZIP commands pass. Svelte has zero errors and
warnings. The production ZIP contains 30 unique entries, one content script and
no insecure test-prime hook. Its SHA-256 is
`0887e59fbb960a73767cc4ff8e37c1dfb1518961de79ce282c0a0af23f5d8e59`.
Actual Chromium verifies one provider initialization and initial EIP-6963
announcement, stable rediscovery and read-only accounts. Provider unit tests
also pass. [Public visual/packaging evidence](https://github.com/stars-labs/starlab-wallet/blob/feat/ecdsa/docs/design/evidence/product-closeout-2026-10-05.json)
records per-image and per-bundle hashes; later documentation and CI-only revisions
do not claim to have generated every capture.

The current extension CI oracle pins runtime engine `18328a1`. Its check and
both curve interoperability jobs must be inspected on
[extension PR83](https://github.com/stars-labs/starlab-wallet/pull/83).
Those jobs exercise extension/native DKG, message and EIP-1559 signing, backup
persistence, quorum/account selection, refresh boundaries, outages/reconnection,
and namespace recovery. Fixture ceremonies are separate from real-prime acceptance.

## Desktop product acceptance

Business acceptance source `c1ff263bf6190339f03d38ea618a994655a13007` is published.
All three desktop Cargo dependencies and the lockfile pin runtime engine
`18328a1b8f6d04c769818036b2a0b835c58d22bb`. Fifty-one locked tests pass,
with zero failures and one default-ignored integration test. The integration
was then explicitly run on this pin and passes in 32.99 seconds: actual three
runners perform 2-of-3 DKG, Ethereum account-1 signing with independent address
recovery, and encrypted FROST export/import using deterministic fixture primes.
The creator/joiner cancellation regressions use a private signal server and
cancel genuinely active ceremonies rather than mistaking connection failure
for successful cancellation. Failed setup closes progress before showing retry,
and a fresh creation is accepted.

Formatting, strict all-target Clippy and the locked optimized release pass.
Business-baseline versioned binary SHA-256:
`f816a9ed856dc6a39efec7c2b2550ace938c681bd93b558f0b565b4e9cfbb432`.
Eight actual final-release normal/narrow windows verify navigation, keyboard
focus, setup, exhausted connection failure and a new same-server retry attempt.
An immediately refused retry returns to Offline; its screenshot is not claimed
as sustained Connecting. Independently checked source-input, screenshot and
binary hashes appear in [final desktop evidence](https://github.com/stars-labs/starlab-desktop/blob/c1ff263bf6190339f03d38ea618a994655a13007/docs/design/evidence/closeout-final-18328-2026-10-05.json).

The compact 58-case normal/narrow fixture gallery remains separately pinned to
its presentation source. Final release inputs retain that visual implementation;
earlier fixture captures are not relabeled as newly generated final-head shots.
A final presentation-only change at published desktop source
`f0f3e5c8f289fff952ee912b83d4b34261827f48` sets the default window to
1120×680 instead of Iced's 1024×768 default: 96px wider and 88px shorter.
Documentation-only head `b212a58` corrects the recorded area calculation; it
changes no source or binary.
It remains resizable/maximizable and retains fonts, targets and business logic.
All adapter and dependency bytes match the tested business baseline above.
Four public-fixture states and two scroll captures verify approval, signature
request and export controls at the new logical viewport. Two actual default
software-rendered windows confirm unresized geometry, full sidebar/status and
readable focused setup. Formatting, locked check and optimized release pass.
[Current default-window evidence](https://github.com/stars-labs/starlab-desktop/blob/b212a58/docs/design/evidence/default-window-1120-2026-10-05.json)
records source inputs and separately versioned binary SHA-256:
`c2f2733b548fb7010de952a6d9cd338860125308e7124a25911c0c38148a86e9`.
The new default evidence does not relabel prior c1ff263 test executions or windows.

Native static gradients, translucent panels and edge lighting provide depth;
this is not a claim of dynamic browser-style backdrop blur. Runtime shadows
were removed after actual software-renderer repaint trails were reproduced.

[Desktop PR24](https://github.com/stars-labs/starlab-desktop/pull/24) carries the
current Linux formatting, tests, strict all-target Clippy and optimized release
gates. Its shared failure cleanup is engine-owned; the desktop maps the error to
an actionable English message and does not add a cancellation workaround.

## Hardware bridge and supported boundaries

The native host fix at `55283e6b9740f65025b15170f082679ca1d52ee4` provides an
isolated GUI PIN entry process instead of relying on an unavailable terminal.
PIN values do not enter browser state, argv, normal logs or protocol stdout.
Assuan decoding, limits, cancellation and native framed JSON have subprocess
coverage. Fifty combined tests and the release build pass; host strict Clippy,
CI and CodeQL also pass. [Host PR19](https://github.com/stars-labs/yubiwallet/pull/19)
is a reviewable draft; no physical card signing is claimed.

Supported runtime evidence is Linux desktop and Chromium extension. Firefox
builds, but its crypto runtime is unverified and the extension requires
`chrome.offscreen`. macOS/Windows runtime and signed desktop installers are not
verified deliveries. Ethereum Send is in the extension; desktop signs messages
and approves shared operations. FROST refresh covers Bitcoin, Solana and Sui,
but does not rotate Ethereum ECDSA shares or revoke an Ethereum device.

Installation and build instructions are in each product README; the extension
hardware guide points to the GUI PIN host revision. Reviewable branches and PRs
are published, without merging main or touching existing live wallet processes.

## Separately pinned real-client transaction

[Historical evidence](../testing/evidence/sepolia-live-2026-10-04.json) records
fresh production-prime TUI/desktop/production-extension 2-of-3 setup and
[the confirmed Sepolia transaction](https://eth-sepolia.blockscout.com/tx/0xe83cfdfba02c82fb067e7bdc195b8f729abe32ffaa247be9a2950805d1e4ff54):
receipt `0x1`, block `11841485`, gas `21000`, with independent transaction hash,
sender recovery and approval-digest verification. Its live clients have their
own explicit source pins. Current presentation, packaging and lifecycle tests
do not claim a new on-chain transaction or unavailable physical hardware checks.
