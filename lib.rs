// Cash Mirror — every time a player earns in-game money, the game server
// mirrors it on-chain and mints $CASH (1 token per $1,000 earned).
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, MintTo, TokenAccount, TokenInterface},
};

// Solana Playground replaces this with your program ID on Build.
declare_id!("11111111111111111111111111111111");

pub const GAME_SEED: &[u8] = b"game";
pub const MINT_SEED: &[u8] = b"cash";
pub const PLAYER_SEED: &[u8] = b"player";
pub const DOLLARS_PER_CASH: u64 = 1_000;
pub const MAX_EARNING_PER_EVENT: u64 = 1_000_000_000; // caps damage if the server key leaks
pub const MAX_REASON_LEN: usize = 48;

#[program]
pub mod cash_mirror {
    use super::*;

    /// Admin creates the game, names the server wallet allowed to report earnings,
    /// and creates the $CASH mint (authority = game PDA).
    pub fn initialize_game(ctx: Context<InitializeGame>, game_server: Pubkey) -> Result<()> {
        let game = &mut ctx.accounts.game;
        game.authority = game_server;
        game.mint = ctx.accounts.mint.key();
        game.bump = ctx.bumps.game;
        game.mint_bump = ctx.bumps.mint;
        game.total_cash_minted = 0;
        Ok(())
    }

    /// Creates the player's on-chain profile and their $CASH token account.
    pub fn register_player(ctx: Context<RegisterPlayer>) -> Result<()> {
        let profile = &mut ctx.accounts.player_profile;
        profile.owner = ctx.accounts.player.key();
        profile.total_earnings = 0;
        profile.events = 0;
        profile.cash_minted = 0;
        profile.last_update = Clock::get()?.unix_timestamp;
        profile.bump = ctx.bumps.player_profile;
        emit!(PlayerRegistered {
            player: profile.owner
        });
        Ok(())
    }

    /// Game server only: record an in-game payday and mint any $CASH now owed.
    pub fn record_earnings(ctx: Context<RecordEarnings>, amount: u64, reason: String) -> Result<()> {
        require!(amount > 0, CashError::InvalidAmount);
        require!(amount <= MAX_EARNING_PER_EVENT, CashError::AmountTooLarge);
        require!(reason.len() <= MAX_REASON_LEN, CashError::ReasonTooLong);

        let game_bump = ctx.accounts.game.bump;
        let profile = &mut ctx.accounts.player_profile;

        profile.total_earnings = profile
            .total_earnings
            .checked_add(amount)
            .ok_or(CashError::Overflow)?;
        profile.events = profile.events.checked_add(1).ok_or(CashError::Overflow)?;

        // Mint against lifetime earnings so leftover dollars (< $1,000) are never lost.
        let cash_owed = profile.total_earnings / DOLLARS_PER_CASH;
        let to_mint = cash_owed
            .checked_sub(profile.cash_minted)
            .ok_or(CashError::Overflow)?;
        profile.cash_minted = cash_owed;
        profile.last_update = Clock::get()?.unix_timestamp;

        let player = profile.owner;
        let total_earnings = profile.total_earnings;

        if to_mint > 0 {
            let signer_seeds: &[&[&[u8]]] = &[&[GAME_SEED, &[game_bump]]];
            token_interface::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    MintTo {
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ctx.accounts.player_ata.to_account_info(),
                        authority: ctx.accounts.game.to_account_info(),
                    },
                    signer_seeds,
                ),
                to_mint,
            )?;
            let game = &mut ctx.accounts.game;
            game.total_cash_minted = game
                .total_cash_minted
                .checked_add(to_mint)
                .ok_or(CashError::Overflow)?;
        }

        emit!(EarningsRecorded {
            player,
            amount,
            reason,
            total_earnings,
            cash_minted: to_mint,
        });
        Ok(())
    }
}

// ---------- Accounts ----------

#[derive(Accounts)]
pub struct InitializeGame<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + Game::INIT_SPACE,
        seeds = [GAME_SEED],
        bump
    )]
    pub game: Account<'info, Game>,

    #[account(
        init,
        payer = admin,
        seeds = [MINT_SEED],
        bump,
        mint::decimals = 0,
        mint::authority = game,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterPlayer<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub player: Signer<'info>,

    #[account(seeds = [GAME_SEED], bump = game.bump)]
    pub game: Account<'info, Game>,

    #[account(
        seeds = [MINT_SEED],
        bump = game.mint_bump,
        address = game.mint @ CashError::WrongMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = 8 + PlayerProfile::INIT_SPACE,
        seeds = [PLAYER_SEED, player.key().as_ref()],
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = player,
        associated_token::token_program = token_program,
    )]
    pub player_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecordEarnings<'info> {
    /// Must be the registered game server.
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [GAME_SEED],
        bump = game.bump,
        has_one = authority @ CashError::Unauthorized,
        has_one = mint @ CashError::WrongMint,
    )]
    pub game: Account<'info, Game>,

    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [PLAYER_SEED, player_profile.owner.as_ref()],
        bump = player_profile.bump,
    )]
    pub player_profile: Account<'info, PlayerProfile>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = player_profile.owner,
        associated_token::token_program = token_program,
    )]
    pub player_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

// ---------- State ----------

#[account]
#[derive(InitSpace)]
pub struct Game {
    pub authority: Pubkey, // game server allowed to report earnings
    pub mint: Pubkey,      // $CASH mint
    pub bump: u8,
    pub mint_bump: u8,
    pub total_cash_minted: u64,
}

#[account]
#[derive(InitSpace)]
pub struct PlayerProfile {
    pub owner: Pubkey,
    pub total_earnings: u64, // lifetime in-game dollars
    pub events: u64,         // number of paydays recorded
    pub cash_minted: u64,    // $CASH minted so far
    pub last_update: i64,
    pub bump: u8,
}

// ---------- Events ----------

#[event]
pub struct PlayerRegistered {
    pub player: Pubkey,
}

#[event]
pub struct EarningsRecorded {
    pub player: Pubkey,
    pub amount: u64,
    pub reason: String,
    pub total_earnings: u64,
    pub cash_minted: u64,
}

// ---------- Errors ----------

#[error_code]
pub enum CashError {
    #[msg("Only the game server can report earnings")]
    Unauthorized,
    #[msg("Mint does not match this game's $CASH mint")]
    WrongMint,
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Amount exceeds the per-event cap")]
    AmountTooLarge,
    #[msg("Reason is too long")]
    ReasonTooLong,
    #[msg("Arithmetic overflow")]
    Overflow,
}
