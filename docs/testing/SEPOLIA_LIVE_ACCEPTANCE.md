# Real three-client Sepolia acceptance

On 2026-10-04 a production extension, desktop and TUI restored their shares
of the same real-prime 2-of-3 wallet. Extension Send initiated the transaction;
the desktop reviewed its digest and approved. The TUI did not approve.

The [public evidence](evidence/sepolia-live-2026-10-04.json) records the final
source revisions, release hashes, signed transaction and recovery checks.
No passwords, encrypted shares or private key material are included.

- Network: Sepolia, chain ID 11155111; EIP-1559, nonce 1.
- MPC sender/recipient: `0x2fb6ae33558e46ec6baa39c9c98cd00f4b0f7548`.
- Value: 0.00001 ETH self-transfer.
- [Transaction](https://eth-sepolia.blockscout.com/tx/0xe83cfdfba02c82fb067e7bdc195b8f729abe32ffaa247be9a2950805d1e4ff54):
  successful receipt (`0x1`), block 11841485, gas 21000.
- Publicnode receipt and independent Blockscout transaction agree.
- Independent viem recovery returns the MPC sender; hashing the reconstructed
  signed transaction equals the chain hash. The unsigned transaction digest
  equals the desktop approval digest.
- Actual TUI prompt present before completion, absent after session withdrawal;
  no manual TUI dismissal. Extension activity shows Confirmed.
- Unknown fiat price renders a dash; result badge accurately identifies
  Ethereum threshold ECDSA rather than labeling a transaction EIP-191.

Local screenshots: `/tmp/starlab-stage5/shots/35-final-source-real-review.png`,
`/tmp/starlab-stage5/shots/36-final-source-real-submitted.png`; desktop live shots
and TUI before/after captures under `/tmp/starlab-stage5` and
`/tmp/starlab-desktop-validation`. Screenshots are supporting local evidence;
the committed JSON and public explorer preserve independently reviewable
transaction evidence.
