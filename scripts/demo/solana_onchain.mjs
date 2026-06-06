// solana_onchain.mjs — put a REAL Solana transaction on-chain, signed by the
// MPC wallet (#A). Division of labour that makes it credible:
//   - the STANDARD @solana/web3.js library builds + submits the transaction
//   - our raw `mpc-wallet-cli` produces the signature (the only thing we made)
// So a skeptic trusts the Solana lib for everything except the signature, and
// the signature is a plain Ed25519 signature their own tools verify.
//
// Flow (steps are separate so the MPC signing in the middle is visible/raw):
//   node solana_onchain.mjs address  <groupKeyHex>
//   node solana_onchain.mjs airdrop  <groupKeyHex> [sol]
//   node solana_onchain.mjs prepare  <groupKeyHex> <toBase58|self> <lamports>   # prints MSG hex to sign
//       -> (sign MSG with the MPC wallet:  mpc-wallet-cli sign --encoding hex --message <MSG> ...)
//   node solana_onchain.mjs finalize <signatureHex>                              # submits, prints explorer URL
//
// Cluster defaults to devnet; override with SOLANA_RPC.
import {
  Connection, PublicKey, SystemProgram, Transaction, LAMPORTS_PER_SOL, clusterApiUrl,
} from "@solana/web3.js";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const RPC = process.env.SOLANA_RPC || clusterApiUrl("devnet");
const CTX = path.join(os.tmpdir(), "mpc-solana-ctx.json");
const conn = new Connection(RPC, "confirmed");

const pkFromHex = (hex) => new PublicKey(Buffer.from(hex.replace(/^0x/, ""), "hex"));
const explorer = (sig) =>
  `https://explorer.solana.com/tx/${sig}?cluster=${RPC.includes("devnet") ? "devnet" : "custom"}`;

const [cmd, a, b, c] = process.argv.slice(2);

if (cmd === "address") {
  console.log(pkFromHex(a).toBase58());
} else if (cmd === "airdrop") {
  const pk = pkFromHex(a);
  const sol = Number(b || "1");
  console.log(`requesting ${sol} SOL airdrop to ${pk.toBase58()} on ${RPC} ...`);
  const sig = await conn.requestAirdrop(pk, sol * LAMPORTS_PER_SOL);
  const bh = await conn.getLatestBlockhash();
  await conn.confirmTransaction({ signature: sig, ...bh }, "confirmed");
  console.log("airdrop confirmed:", explorer(sig));
  console.log("balance:", (await conn.getBalance(pk)) / LAMPORTS_PER_SOL, "SOL");
} else if (cmd === "prepare") {
  const from = pkFromHex(a);
  const to = !b || b === "self" ? from : new PublicKey(b);
  const lamports = Number(c || "1000");
  const { blockhash, lastValidBlockHeight } = await conn.getLatestBlockhash();
  const tx = new Transaction({ feePayer: from, blockhash, lastValidBlockHeight });
  tx.add(SystemProgram.transfer({ fromPubkey: from, toPubkey: to, lamports }));
  const msg = tx.serializeMessage(); // exact bytes Solana verifies the signature over
  fs.writeFileSync(
    CTX,
    JSON.stringify({
      fromHex: a.replace(/^0x/, ""), to: to.toBase58(), lamports, blockhash, lastValidBlockHeight,
    }),
  );
  console.log("from   :", from.toBase58());
  console.log("to     :", to.toBase58());
  console.log("lamports:", lamports);
  console.log("");
  console.log("MESSAGE TO SIGN (hex) — feed to the MPC wallet with --encoding hex:");
  console.log(msg.toString("hex"));
  console.log("");
  console.log(`(context saved to ${CTX}; run \`finalize <signatureHex>\` within ~60s)`);
} else if (cmd === "verify") {
  // OFFLINE proof: rebuild the exact tx, attach the MPC signature, and ask the
  // standard Solana library whether the signature is valid for it. No funds /
  // no network needed — this confirms the threshold signature would be accepted
  // by Solana for a real transaction.
  const ctx = JSON.parse(fs.readFileSync(CTX, "utf8"));
  const from = pkFromHex(ctx.fromHex);
  const to = new PublicKey(ctx.to);
  const tx = new Transaction({
    feePayer: from, blockhash: ctx.blockhash, lastValidBlockHeight: ctx.lastValidBlockHeight,
  });
  tx.add(SystemProgram.transfer({ fromPubkey: from, toPubkey: to, lamports: ctx.lamports }));
  tx.addSignature(from, Buffer.from(a.replace(/^0x/, ""), "hex"));
  console.log("from (Solana addr):", from.toBase58());
  console.log("to                :", to.toBase58(), `(${ctx.lamports} lamports)`);
  console.log("web3.js tx.verifySignatures():", tx.verifySignatures());
} else if (cmd === "finalize") {
  const ctx = JSON.parse(fs.readFileSync(CTX, "utf8"));
  const from = pkFromHex(ctx.fromHex);
  const to = new PublicKey(ctx.to);
  // Rebuild the IDENTICAL message so the signed bytes match.
  const tx = new Transaction({
    feePayer: from, blockhash: ctx.blockhash, lastValidBlockHeight: ctx.lastValidBlockHeight,
  });
  tx.add(SystemProgram.transfer({ fromPubkey: from, toPubkey: to, lamports: ctx.lamports }));
  tx.addSignature(from, Buffer.from(a.replace(/^0x/, ""), "hex")); // verifies the sig fits
  const raw = tx.serialize(); // throws if the signature is invalid for these bytes
  const sig = await conn.sendRawTransaction(raw, { skipPreflight: false });
  console.log("submitted:", sig);
  await conn.confirmTransaction(
    { signature: sig, blockhash: ctx.blockhash, lastValidBlockHeight: ctx.lastValidBlockHeight },
    "confirmed",
  );
  console.log("CONFIRMED ON-CHAIN:", explorer(sig));
} else {
  console.error("usage: address|airdrop|prepare|finalize  (see file header)");
  process.exit(2);
}
