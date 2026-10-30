# Wallet recovery and share refresh

A wallet requires its configured threshold of distinct participant shares to
sign. An encrypted backup restores one participant; it does not create an
additional independent participant. A participant seed alone cannot rebuild a
share produced by dealerless DKG.

## Back up and restore a device

Keep the encrypted keystore files and their password. A unified wallet has
separate curve files: `ed25519`, `secp256k1` (Bitcoin Taproot), and
`secp256k1-ecdsa` (Ethereum). Export every curve belonging to the wallet and
restore them together. Losing the ECDSA file loses that device's Ethereum
share even when its Bitcoin file survives.

Use the clients' import/export flows so wallet metadata and participant
identifiers travel with the encrypted share. Do not edit identifiers to make
a restored share appear to be a new participant. Keep the password separately
from the backup; the clients cannot recover a forgotten password.

## Ethereum has a different recovery boundary

The current cggmp24 ECDSA implementation supports key generation and signing,
not share refresh or resharing. FROST refresh does not rotate an Ethereum
share or revoke an old Ethereum signer.

If a threshold of original Ethereum participants survives, it can still sign
and move assets to a newly generated wallet. Changing the Ethereum cohort
requires that new wallet and moving funds to its new addresses. If fewer than
a threshold survives and there are no usable backups, the Ethereum key cannot
be recovered. Creating a new key does not recover the old address.

## FROST share refresh

For FROST keys, the implemented dealerless refresh replaces participant
shares while preserving the group public key and derived account addresses.
An old share cannot combine with refreshed shares to sign. The retained
participants must already have valid shares; this refresh cannot enroll a
brand-new participant without an existing share or lower the threshold.

The core supports removing a participant when enough existing holders remain.
Client controls differ: the CLI's `reshare --wallet-id` initiates a same-cohort
refresh; the extension device flow can select retained existing holders.
Neither operation supplies ECDSA refresh. Do not interpret a FROST completion
as proof that every key in a multi-curve wallet has been rotated.

The network ceremony uses the signaling server and WebRTC mesh. It checks
that the group key is preserved before replacing the stored encrypted FROST
share. This is implemented, not a proposed future transport.

## When a device is missing

| Situation | Recovery action |
|---|---|
| At least a threshold of original shares remains | Continue signing; restore the missing participant from its backup, or use the protocol-specific recovery above. |
| A FROST device is compromised | Refresh the supported FROST cohort; removal requires enough retained existing holders. Ethereum shares are unaffected. |
| An Ethereum device must be removed | Generate a new wallet and transfer assets using the surviving original threshold. |
| Fewer than a threshold remains, usable backups exist | Restore enough distinct original participants and unlock their backups. |
| Fewer than a threshold remains, no usable backups | The old wallet cannot sign or recover its key. |

## Verified implementation

- [FROST refresh engine](../packages/@starlab/core/src/resharing.rs): group-key
  preservation, old-share invalidation, participant removal, noncontiguous
  identifiers, and rejection of a participant without a share.
- Native ignored end-to-end tests cover network refresh followed by signing
  with the unchanged group key. Extension interoperability covers refresh
  followed by co-signing.
- [Signing keys and chain compatibility](SIGNATURE_CHAIN_COMPATIBILITY.md)
  defines the key each account uses.
- [ECDSA completion evidence](changes/2026-10-04-ecdsa-completion.md) records
  the current acceptance results and remaining live transaction gate.
