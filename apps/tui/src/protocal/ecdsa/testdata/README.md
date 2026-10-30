# Public insecure signing-control fixture

`insecure-control.json` is a deliberately public test key. Never use its key,
primes, or deterministic randomness for funds or production ceremonies.

It was generated in an isolated test session using the existing network helpers
in `packages/@starlab/core/src/ecdsa/tests.rs`, not from a user keystore:

- `keygen("INSECURE-public-native-control-fixture", 2, 7301)`; three participants
  `device-0`, `device-1`, `device-2`, using committed `ecdsa/testdata/primes.json`.
- `sign(&shares, &[0, 1], 0, [42; 32], 7302)`; Ethereum account 0.
- Store `shares[2]` and the signature's `to_bytes()` as hex. Its recovery byte is
  0/1; the native tests convert it to Ethereum 27/28.

A temporary ignored test invoked these helpers once (1 passed in 56.56 seconds)
and was removed after writing the fixture. Control-frame regressions deserialize
this fixture and never run key generation, auxiliary-info proofs, or Paillier
signing. They still independently recover and compare the account address.
