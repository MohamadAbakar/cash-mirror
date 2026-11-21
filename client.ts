// Cash Mirror demo — paste into Solana Playground's client/client.ts and click "Run".
// pg.wallet plays the GAME SERVER; a fresh keypair plays the PLAYER.
import { getAssociatedTokenAddressSync } from "@solana/spl-token";

const TOKEN_PROGRAM_ID = new web3.PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM_ID = new web3.PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

const program = pg.program;
const server = pg.wallet;
const pda = (...seeds: Buffer[]) =>
  web3.PublicKey.findProgramAddressSync(seeds, program.programId)[0];

const gamePda = pda(Buffer.from("game"));
const mintPda = pda(Buffer.from("cash"));
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const money = (n: number) => "$" + n.toLocaleString("en-US");

// 1. Create the game once (skipped on re-runs)
const existing = await program.account.game.fetchNullable(gamePda);
if (!existing) {
  await program.methods
    .initializeGame(server.publicKey)
    .accounts({
      admin: server.publicKey,
      game: gamePda,
      mint: mintPda,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: web3.SystemProgram.programId,
    } as any)
    .rpc();
  console.log("🎮 Game created. $CASH mint:", mintPda.toBase58());
} else {
  console.log("🎮 Game already exists. $CASH mint:", mintPda.toBase58());
}

// 2. A new player joins
const player = web3.Keypair.generate();
const profilePda = pda(Buffer.from("player"), player.publicKey.toBuffer());
const playerAta = getAssociatedTokenAddressSync(mintPda, player.publicKey);

await program.methods
  .registerPlayer()
  .accounts({
    payer: server.publicKey,
    player: player.publicKey,
    game: gamePda,
    mint: mintPda,
    playerProfile: profilePda,
    playerAta,
    tokenProgram: TOKEN_PROGRAM_ID,
    associatedTokenProgram: ATA_PROGRAM_ID,
    systemProgram: web3.SystemProgram.programId,
  } as any)
  .signers([player])
  .rpc();
console.log("🧍 Player registered:", player.publicKey.toBase58());
console.log("─".repeat(60));

// 3. The "game" pays out — each payday is mirrored on-chain
const paydays: [string, number][] = [
  ["Robbed a convenience store", 5_000],
  ["Sold a stolen sports car", 12_500],
  ["Won a street race", 8_000],
  ["Heist complete: Vice Beach Bank", 250_000],
];

const accounts = (authority: web3.PublicKey) =>
  ({
    authority,
    game: gamePda,
    mint: mintPda,
    playerProfile: profilePda,
    playerAta,
    tokenProgram: TOKEN_PROGRAM_ID,
  } as any);

for (const [reason, amount] of paydays) {
  const sig = await program.methods
    .recordEarnings(new BN(amount), reason)
    .accounts(accounts(server.publicKey))
    .rpc();
  await sleep(500);
  const profile = await program.account.playerProfile.fetch(profilePda);
  const bal = await pg.connection.getTokenAccountBalance(playerAta);
  console.log(`💰 ${reason}: +${money(amount)}`);
  console.log(
    `   on-chain total: ${money(profile.totalEarnings.toNumber())} | $CASH: ${bal.value.uiAmountString} | paydays: ${profile.events.toNumber()}`
  );
  console.log(`   tx: https://explorer.solana.com/tx/${sig}?cluster=devnet`);
}

// 4. Security check — a player tries to pay themselves
console.log("─".repeat(60));
try {
  await program.methods
    .recordEarnings(new BN(999_999_999), "I'm definitely legit")
    .accounts(accounts(player.publicKey))
    .signers([player])
    .rpc();
  console.log("❌ Uh oh — cheat went through");
} catch (e: any) {
  const msg = e?.error?.errorMessage ?? e?.message ?? String(e);
  console.log("🛡️  Cheat blocked:", msg);
}
