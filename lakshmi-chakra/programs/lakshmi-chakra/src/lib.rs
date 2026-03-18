use anchor_lang::prelude::*;
use solana_program::pubkey;
use ephemeral_rollups_sdk::prelude::*;
use ephemeral_rollups_sdk::{delegate, ephemeral};

declare_id!("BYe1eVU9XeUeezxyrUN7L9zfWBhcjGAYugmbzhf6L1ze");

const SCALE: u128 = 1_000_000_000_000; // 1e12 scaling factor

#[ephemeral]
#[program]
pub mod lakshmi_chakra {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        p0: f64,
        k: f64,
        duration_seconds: i64,
    ) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        
        // Convert f64 to u128 fixed-point
        lottery.p0 = (p0 * SCALE as f64) as u128;
        lottery.k = (k * SCALE as f64) as u128;
        
        lottery.total_sol = 0;
        lottery.total_tickets = 0;

        let current_time = Clock::get()?.unix_timestamp;
        lottery.start_time = current_time;
        lottery.end_time = current_time + duration_seconds;

        lottery.authority = ctx.accounts.authority.key();
        lottery.winner = None;
        lottery.bump = ctx.bumps.lottery;

        Ok(())
    }

    pub fn delegate_lottery(ctx: Context<DelegateLottery>) -> Result<()> {
        ephemeral_rollups_sdk::cpi::delegate(
            ctx.accounts.magic_block_program.to_account_info(),
            ctx.accounts.lottery.to_account_info(),
            ctx.accounts.authority.to_account_info(),
        )?;
        Ok(())
    }

    #[ephemeral]
    pub fn buy_ticket(ctx: Context<BuyTicket>, sol_amount_lamports: u64) -> Result<()> {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.lottery.to_account_info(),
                },
            ),
            sol_amount_lamports,
        )?;

        let lottery = &mut ctx.accounts.lottery;
        let current_time = Clock::get()?.unix_timestamp;

        require!(current_time >= lottery.start_time, ErrorCode::LotteryNotStarted);
        require!(current_time <= lottery.end_time, ErrorCode::LotteryEnded);
        require!(sol_amount_lamports > 0, ErrorCode::InvalidAmount);

        let delta_tickets = lottery.calculate_delta_tickets(sol_amount_lamports)?;

        let user_stats = &mut ctx.accounts.user_stats;
        let ticket_batch = &mut ctx.accounts.ticket_batch;

        // Set batch data
        ticket_batch.owner = ctx.accounts.user.key();
        ticket_batch.start_index = lottery.total_tickets;
        ticket_batch.tickets = delta_tickets;
        ticket_batch.bump = ctx.bumps.ticket_batch;

        // Update globals
        lottery.total_tickets = lottery.total_tickets.checked_add(delta_tickets).ok_or(ErrorCode::MathOverflow)?;
        lottery.total_sol = lottery.total_sol.checked_add(sol_amount_lamports).ok_or(ErrorCode::MathOverflow)?;

        // Update user stats
        user_stats.owner = ctx.accounts.user.key();
        user_stats.total_tickets = user_stats.total_tickets.checked_add(delta_tickets).ok_or(ErrorCode::MathOverflow)?;
        // Use batch_count as the index for the next batch, then increment
        user_stats.batch_count = user_stats.batch_count.checked_add(1).ok_or(ErrorCode::MathOverflow)?;
        user_stats.bump = ctx.bumps.user_stats;

        msg!("User {} bought batch #{} with {} tickets", 
            ctx.accounts.user.key(), 
            user_stats.batch_count,
            delta_tickets as f64 / SCALE as f64
        );
        
        Ok(())
    }

    pub fn request_winner(ctx: Context<RequestWinner>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let current_time = Clock::get()?;

        require!(current_time.unix_timestamp >= lottery.end_time, ErrorCode::LotteryNotEnded);

        ephemeral_rollups_sdk::vrf::request_randomness(
            ctx.accounts.magic_block_program.to_account_info(),
            ctx.accounts.vrf_account.to_account_info(),
            ctx.accounts.lottery.to_account_info(),
        )?;

        lottery.randomness_account = ctx.accounts.vrf_account.key();
        msg!("Magic block VRF requested!..");
        Ok(())
    }

    pub fn resolve_winner(ctx: Context<ResolveWinner>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;

        require!(
            ctx.accounts.vrf_account.key() == lottery.randomness_account,
            ErrorCode::InvalidRandomnessAccount
        );

        let randomness = ephemeral_rollups_sdk::vrf::get_randomness(&ctx.accounts.vrf_account)?;

        // winner_index = (randomness % 1e12) / 1e12 * total_tickets
        let rand_scalar = (randomness % SCALE as u64) as u128;
        lottery.winning_index = Some(
            rand_scalar
                .checked_mul(lottery.total_tickets)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(SCALE)
                .ok_or(ErrorCode::MathOverflow)?
        );

        msg!("Winning Index set to {:?}", lottery.winning_index);
        Ok(())
    }

    pub fn claim_prize(ctx: Context<ClaimPrize>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let ticket_batch = &ctx.accounts.ticket_batch;

        let win_idx = lottery.winning_index.ok_or(ErrorCode::WinnerNotDrawn)?;

        // Check if the winning index falls within THIS batch
        let is_winner = win_idx >= ticket_batch.start_index && 
                        win_idx < ticket_batch.start_index.checked_add(ticket_batch.tickets).ok_or(ErrorCode::MathOverflow)?;

        require!(is_winner, ErrorCode::NotWinner);

        let prize_amount = lottery.total_sol;
        
        **ctx.accounts.lottery.to_account_info().try_borrow_mut_lamports()? -= prize_amount;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += prize_amount;

        lottery.total_sol = 0;
        lottery.winner = Some(ctx.accounts.user.key());

        msg!("Winner found! paid out {} SOL to {}", prize_amount as f64 / 1e9, ctx.accounts.user.key());
        Ok(())
    }
}

#[account]
#[delegate]
#[derive(InitSpace)]
pub struct Lottery {
    pub p0: u128,             // Fixed point p0
    pub k: u128,              // Fixed point k
    pub total_sol: u64,       // Lamports
    pub total_tickets: u128,   // Fixed point tickets
    pub start_time: i64,
    pub end_time: i64,
    pub authority: Pubkey,
    pub winner: Option<Pubkey>,
    pub bump: u8,
    pub winning_index: Option<u128>,
    pub randomness_account: Pubkey,
}

#[account]
#[delegate]
#[derive(InitSpace)]
pub struct UserStats {
    pub owner: Pubkey,
    pub total_tickets: u128,
    pub batch_count: u64,
    pub bump: u8,
}

#[account]
#[delegate]
#[derive(InitSpace)]
pub struct TicketBatch {
    pub owner: Pubkey,
    pub start_index: u128,
    pub tickets: u128,
    pub bump: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Lottery has already ended")]
    LotteryEnded,
    #[msg("Lottery has not started yet")]
    LotteryNotStarted,
    #[msg("Not enough tickets")]
    NotEnoughTickets,
    #[msg("Invalid Sol amount")]
    InvalidAmount,
    #[msg("Invalid lottery parameters")]
    InvalidLotteryParameters,
    #[msg("Invalid vrf account")]
    InvalidRandomnessAccount,
    #[msg("Winner has not been drawn yet")]
    WinnerNotDrawn,
    #[msg("You are not the winner")]
    NotWinner,
    #[msg("Lottery has not ended yet")]
    LotteryNotEnded,
    #[msg("Math overflow")]
    MathOverflow,
}

impl Lottery {
    pub fn calculate_delta_tickets(&self, sol_amount_lamports: u64) -> Result<u128> {
        let sol_amount = (sol_amount_lamports as u128)
            .checked_mul(SCALE)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(1_000_000_000)
            .ok_or(ErrorCode::MathOverflow)?; // SOL fixed point (1e12)
            
        let current_sol = (self.total_sol as u128)
            .checked_mul(SCALE)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(1_000_000_000)
            .ok_or(ErrorCode::MathOverflow)?;

        let new_total_sol = current_sol.checked_add(sol_amount).ok_or(ErrorCode::MathOverflow)?;

        if self.k > 0 {
            // Formula: t(s) = (-P0 + sqrt(P0^2 + 2*k*s)) / k
            
            let calc_total_tickets = |s: u128| -> Result<u128> {
                // p0_sq = p0 * p0 (scaled by SCALE^2)
                let p0_sq = self.p0.checked_mul(self.p0).ok_or(ErrorCode::MathOverflow)?;
                
                // term2 = 2 * k * s (scaled by SCALE^2)
                let term2 = 2u128
                    .checked_mul(self.k).ok_or(ErrorCode::MathOverflow)?
                    .checked_mul(s).ok_or(ErrorCode::MathOverflow)?;
                
                // radicand = p0_sq + term2 (scaled by SCALE^2)
                let radicand = p0_sq.checked_add(term2).ok_or(ErrorCode::MathOverflow)?;
                
                // root = sqrt(radicand) (scaled by SCALE)
                let root = integer_sqrt(radicand);
                
                // tickets = (root - p0) * SCALE / k
                // We multiply by SCALE to maintain fixed-point precision after division
                let numerator = root.checked_sub(self.p0).ok_or(ErrorCode::MathOverflow)?;
                let tickets = numerator
                    .checked_mul(SCALE).ok_or(ErrorCode::MathOverflow)?
                    .checked_div(self.k).ok_or(ErrorCode::MathOverflow)?;
                
                Ok(tickets)
            };

            let current_total_tickets = calc_total_tickets(current_sol)?;
            let new_total_tickets = calc_total_tickets(new_total_sol)?;
            
            Ok(new_total_tickets.checked_sub(current_total_tickets).ok_or(ErrorCode::MathOverflow)?)
        } else {
            // Flat price: tickets = (sol * SCALE) / p0
            Ok(sol_amount
                .checked_mul(SCALE)
                .ok_or(ErrorCode::MathOverflow)?
                .checked_div(self.p0)
                .ok_or(ErrorCode::MathOverflow)?)
        }
    }
}


#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Lottery::INIT_SPACE,
        seeds = [b"lottery"],
        bump
    )]
    pub lottery: Account<'info, Lottery>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DelegateLottery<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,
    pub authority: Signer<'info>,
    pub magic_block_program: Program<'info, MagicBlock>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuyTicket<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + UserStats::INIT_SPACE,
        seeds = [b"user_stats", user.key().as_ref()],
        bump
    )]
    pub user_stats: Box<Account<'info, UserStats>>,

    #[account(
        init,
        payer = user,
        space = 8 + TicketBatch::INIT_SPACE,
        seeds = [b"ticket", user.key().as_ref(), user_stats.batch_count.to_le_bytes().as_ref()],
        bump
    )]
    pub ticket_batch: Box<Account<'info, TicketBatch>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RequestWinner<'info> {
    #[account(mut, has_one = authority)]
    pub lottery: Account<'info, Lottery>,
    pub authority: Signer<'info>,
    #[account(mut)]
    pub vrf_account: AccountInfo<'info>,
    pub magic_block_program: Program<'info, MagicBlock>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolveWinner<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,
    pub vrf_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ClaimPrize<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,

    #[account(
        seeds = [b"ticket", user.key().as_ref(), batch_id.to_le_bytes().as_ref()],
        bump = ticket_batch.bump,
    )]
    pub ticket_batch: Account<'info, TicketBatch>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
    
    /// This is used to derive the correct TicketBatch PDA
    pub batch_id: u64,
}

#[derive(Clone)]
pub struct MagicBlock;

impl anchor_lang::Id for MagicBlock {
    fn id() -> Pubkey {
        pubkey!("MagicBlock11111111111111111111111111111111")
    }
}

pub fn integer_sqrt(val : u128) -> u128 {

    if val == 0 {
        return 0;
    }
    let mut x = 1u128 << ((128 - val.leading_zeros() + 1) / 2);
    loop {
        let y = (x + val / x) >> 1;
        if y >= x {
            return x;
        }
        x = y;
    }
    
}