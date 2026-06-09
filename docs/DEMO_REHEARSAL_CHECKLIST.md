# Dress-Rehearsal Checklist — Live MPC Demo

> 中文 / Chinese: [DEMO_REHEARSAL_CHECKLIST.zh.md](./DEMO_REHEARSAL_CHECKLIST.zh.md)

Run this **end to end at least twice** on the **actual demo devices and network** before
investors. The goal isn't to learn the commands — it's to surface the boring failures
(a typo'd room, a duplicate device id, a dead Wi‑Fi, a cold binary) **before** they happen
on stage. Pair this with the guide: [`INVESTOR_GUIDE.md`](INVESTOR_GUIDE.md) (section refs below
point into it).

> Rule of thumb (per the project's demo principle): **never run a command on stage you
> haven't run, verbatim, in rehearsal on the same machine.**

---

## T‑1 day — preparation

- [ ] **Binaries built on every device.** `cargo build --release -p mpc-wallet-cli` on
      each laptop. Confirm `--curve ed25519` exists: `mpc-wallet-cli serve --help | grep -i curve`.
- [ ] **Node.js present** on the machine that will run the independent verification (and,
      if showing on‑chain, `@solana/web3.js` installed: `bun install` at repo root).
- [ ] **Decide the curve story:** ed25519 (Solana) — it's the independently verifiable one.
- [ ] **Pre‑fund the Solana address** (only if doing the on‑chain `finalize` beat). The
      public devnet faucet is rate‑limited, so do NOT rely on a live airdrop. Pre‑fund via
      <https://faucet.solana.com> or a transfer from a funded devnet wallet, and record the
      address. (The address is per‑DKG‑run, so either pre‑create a persistent wallet and
      reuse its keystores, or plan to do the on‑chain beat as the *fund‑independent*
      `verify` proof — see guide §3.4.)
- [ ] **Write down a fixed plan:** device ids (`alice`/`bob`/`carol`), the room id, the
      threshold (2‑of‑3), the demo password. Unique device id per machine — **a duplicate
      silently breaks the mesh.**
- [ ] **Charge everything / power adapters.** Demos run long.

## T‑10 min — on the actual network, on every device

- [ ] **Pre‑flight passes on each laptop:**
      `SIGNAL=wss://panda.qzz.io scripts/demo/preflight.sh` → **8 passed, 0 failed**,
      including step 4 (a real ceremony through the live server). If step 4 is red, the
      live path is down → switch to the LAN fallback now (guide §6 rung 1).
- [ ] **Clock/agreement:** everyone has the *same* room id and signal URL pasted, ready.
- [ ] **Projector/screen mirroring works**; terminals are large‑font and readable.
- [ ] **Fallback ready:** know which laptop runs the LAN signal server if Wi‑Fi dies, and
      that the NUCLEAR `scripts/demo/ceremony.sh … --sign` one‑liner is one paste away
      (guide §6 rung 3).

## The run — do it twice, timed

1. [ ] Three terminals start `serve --curve ed25519` (guide §3.1). All three show
       `connection: true`.
2. [ ] alice `create_wallet` → reads the `session_announced` id aloud.
3. [ ] bob + carol `join_session`. **All three print the SAME `group_public_key`.** ✅
       *(Glance: do the three keys match? That's the live "it's real" beat.)*
4. [ ] Show the three separate keystore files exist (`ls ~/.frost_*`).
5. [ ] Investor names a message → alice `sign` → bob `approve_signing` → `signature_complete`.
6. [ ] **Independent verify** on a clean machine (guide §3.3) → `VERIFIED: true`.
7. [ ] (If using it) the on‑chain beat (guide §3.4): `verify` → `verifySignatures: true`,
       and/or `finalize` → explorer link opens and shows the confirmed tx.
8. [ ] **Threshold drama** (guide §3.5): alice tries to sign alone → it times out → repeat
       with bob → it completes.
9. [ ] **Recovery beat** (guide §5, optional): on the wallet-holding node run
       `mpc-wallet-cli reshare --wallet-id <W> --room "$ROOM"` and have the retained signers
       `session join` it → same `group_public_key`, the wallet keeps signing, the dropped
       device's share is dead. The "lose/rotate a device, same address" story. *(No live
       setup? The resharing engine is verified in `cargo test -p mpc-wallet-cli`.)*

## Failure drills (rehearse the recovery, not just the happy path)

- [ ] **Kill Wi‑Fi mid‑run** → switch the whole group to the LAN server (rung 1). Time it.
- [ ] **Type a wrong room on one node** → see it never joins; fix it live calmly.
- [ ] **"Nuclear" cold open** → from nothing, run the `scripts/demo/ceremony.sh … --sign`
      one‑liner and verify the output. This is your "no matter what, here is the crypto
      working — real separate processes" card.
- [ ] **Faucet dead** (if on‑chain) → fall back to the fund‑independent `verify` proof.

## Sign‑off (don't go on stage until all true)

- [ ] Ran the full happy path **twice**, start to finish, on the real devices + network.
- [ ] The independent verification returned `true` both times.
- [ ] At least one failure drill rehearsed and recovered cleanly.
- [ ] Every operator knows their exact lines and the fallback ladder.
- [ ] A printed copy of the guide's **§10 quick‑reference card** is at each station.

---

### Timing target
A clean run (start nodes → DKG → sign → independent verify) is **~60–90 seconds** of
actual work. Budget for narration; the cryptography itself is fast (DKG + signing complete
in single‑digit seconds). If it's taking minutes, something's wrong with the network —
fall back rather than stall in front of the room.
