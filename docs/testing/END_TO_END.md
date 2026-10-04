# End-to-end coverage and execution

The engine, browser extension, and desktop have working cross-client harnesses.
Use their real tests below; screenshots with mocked state establish presentation
coverage and do not establish cryptographic or network interoperability.

| Layer | Authoritative source | What it verifies |
| --- | --- | --- |
| Engine unit and component tests | `cargo test --workspace --locked` | Crypto primitives, keystore persistence, lifecycle transitions, TUI rendering |
| Native WebRTC ceremonies | `apps/cli/tests/e2e_dkg.rs` | DKG, threshold signing, missing-peer timeout/retry, FROST refresh, ECDSA signing |
| Conformance matrix | `apps/cli/tests/conformance_matrix.rs` | Multi-node protocol behavior, malformed inputs, account derivation and signatures |
| Separate processes and wire traces | `apps/cli/tests/l3_serve_process.rs`, `wire_trace.rs` | Restart/cold-start behavior and protocol contract |
| Browser extension interoperability | `stars-labs/starlab-wallet` tests and CI `interop` jobs | Chromium extension paired with native CLI peers, both FROST curves, disconnected/reconnected WebRTC |
| Desktop interoperability | `stars-labs/starlab-desktop` ignored integration tests | Shared engine with desktop frontend, account signing and encrypted export/import |
| Production-prime Ethereum transaction | [Sepolia evidence](evidence/sepolia-live-2026-10-04.json) | Real three-client key generation, recoverable ECDSA transaction and confirmed receipt |

Run native network suites explicitly: their expensive tests are ignored in the
default unit run. Serialize them to avoid fixture and machine resource interference.

```sh
cargo test -p starlab-cli --test e2e_dkg --locked -- --ignored --test-threads=1
cargo test -p starlab-cli --test conformance_matrix --locked -- --ignored --test-threads=1
cargo test -p starlab-cli --test l3_serve_process --locked -- --ignored --test-threads=1
cargo test -p starlab-cli --test wire_trace --locked -- --ignored --test-threads=1
```

The engine CI workflow `.github/workflows/ci.yml` splits these tests across jobs.
The extension and desktop harnesses live in their own repositories; follow their
current test documentation and pinned engine revision. Namespace outage coverage
requires network-namespace privileges and is reported separately when unavailable.
Use unique artifact directories when running concurrent harnesses.

Record source commits, executed test counts, exit status, logs, and fixture mode.
A zero-test or ignored-only run is not a pass. Fast ECDSA fixtures use deliberately
insecure test primes and cannot substitute for production-prime evidence. The
committed Sepolia evidence records its precise revisions and transaction; later UI
screenshots do not implicitly repeat that live transaction.

FROST refresh covers Bitcoin, Solana, and Sui shares. It does not rotate Ethereum
ECDSA shares or revoke an old Ethereum device; see [recovery boundaries](../RECOVERY_AND_RESHARING.md).
