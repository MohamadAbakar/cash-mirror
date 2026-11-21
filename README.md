# Cash Mirror + Textcart

Built during a 25-minute Solana workshop, using the Claude Code setup from [brimigs/workshop](https://github.com/brimigs/workshop).

## Cash Mirror (Anchor program)

Every time a player earns money in a game, the game server mirrors it on-chain and mints `$CASH` (1 token per $1,000 earned). Only the registered game server can report earnings.

- `lib.rs`: the Anchor program (`initialize_game`, `register_player`, `record_earnings`)
- `client.ts`: demo script that plays the game server, records 4 paydays, then shows a cheater being rejected

**Run it in [Solana Playground](https://beta.solpg.io):** create an Anchor project, paste `lib.rs` into `src/lib.rs`, get devnet SOL, then Build, Deploy, paste `client.ts` into `client/client.ts` and Run.

Known limitation: `initialize_game` has no access control (the first caller becomes admin). Fine for a devnet demo, not for production.

## Textcart (concept mockup)

`textcart.html` is a simulated demo: you text a shopping agent, it builds a grocery cart and pays in USDC from an on-chain allowance. A Solana program enforces the spending limits (weekly cap, per-order cap, approved merchants), so the agent can't overspend. No real purchases or transactions. Open the file in a browser to try it.
