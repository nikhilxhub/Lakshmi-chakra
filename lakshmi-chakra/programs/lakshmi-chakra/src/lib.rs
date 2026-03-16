use anchor_lang::prelude::*;
use solana_program::pubkey;

use ephemeral_rollups_sdk::prelude::*;

use ephemeral_rollups_sdk::{delegate, ephemeral};

declare_id!("BYe1eVU9XeUeezxyrUN7L9zfWBhcjGAYugmbzhf6L1ze");

#[ephemeral]
#[program]
pub mod lakshmi_chakra {
    use super::*;



    pub fn initialize(
        ctx :Context<Initialize>,
        p0: f64,
        k: f64,
        duration_seconds: i64,
    ) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        lottery.p0 = p0;
        lottery.k = k;
        lottery.total_sol = 0;
        lottery.total_tickets = 0.0;

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

        let lottery = &mut ctx.accounts.lottery;

        let current_time = Clock::get()?.unix_timestamp;

        require!(
            current_time >= lottery.start_time,
            ErrorCode::LotteryNotStarted
        );

        require!(
            current_time <= lottery.end_time,
            ErrorCode::LotteryEnded
        );

        require!(
            sol_amount_lamports > 0,
            ErrorCode::InvalidAmount
        );

        let delta_tickets = lottery.calculate_delta_tickets(sol_amount_lamports);

        let user_ticket = &mut ctx.accounts.user_ticket;
 
        user_ticket.start_index = lottery.total_tickets;

        lottery.total_tickets += delta_tickets;
        lottery.total_sol += sol_amount_lamports;

        

       user_ticket.owner = ctx.accounts.user.key();
       user_ticket.tickets += delta_tickets;
       user_ticket.bump = ctx.bumps.user_ticket;

       

       msg!("User {} bought {} tickets", ctx.accounts.user.key(), delta_tickets);
       Ok(())

    }

    pub fn request_winner(ctx: Context<RequestWinner>) -> Result<()> {

        let lottery = &mut ctx.accounts.lottery;
        let current_time = Clock::get()?;

        require!(
            current_time.unix_timestamp >= lottery.end_time,
            ErrorCode::LotteryNotEnded
        );

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
            ctx.accounts.vrf_account.key() ==
            lottery.randomness_account,

            ErrorCode::InvalidRandomnessAccount
        );

        let randomness = magic_block_core::vrf::get_randomness(&ctx.accounts.vrf_account)?;

        lottery.winning_index = Some((randomness % 1_000_000) as f64 / 1_000_000.0 * lottery.total_tickets);

        msg!("Winning Index set to {:?}", lottery.winning_index);

        Ok(())


    }

    pub fn claim_prize(ctx: Context<ClaimPrize>) -> Result<()> {

        let lottery = &mut ctx.accounts.lottery;
        let user_ticket = &ctx.accounts.user_ticket;

        let win_idx = lottery.winning_index.ok_or(ErrorCode::WinnerNotDrawn)?;

        let is_winner = win_idx >= user_ticket.start_index && win_idx < (user_ticket.start_index + user_ticket.tickets);

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
pub struct Lottery {
    pub p0: f64,
    pub k: f64,
    pub total_sol: u64,

    pub total_tickets: f64,
    pub start_time: i64,
    pub end_time: i64,
    pub authority: Pubkey,
    pub winner: Option<Pubkey>,
    pub bump: u8,
    pub winning_index: Option<f64>,
    pub randomness_account: Pubkey,

}

#[account]
#[delegate]
pub struct UserTicket {
    pub owner: Pubkey,
    pub tickets: f64,
    pub bump:u8,
    pub start_index: f64,
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
    LotteryNotEnded
}


impl Lottery {
    pub fn calculate_delta_tickets(&self, sol_amount_lamports: u64) -> f64  {
        let sol_amount = sol_amount_lamports as f64 / 1_000_000_000.0;
        let current_sol = self.total_sol as f64 / 1_000_000_000.0;

        let new_total_sol = current_sol + sol_amount;

        if self.k > 0.0 {
            // t{s} = (-P0 + sqrt(P0^ 2 + 2*k*s)) / k;


            let current_total_tickets = (-(self.p0) + ((self.p0 * self.p0) + 2.0 * self.k * current_sol).sqrt()) / self.k;
            let new_total_tickets = (-(self.p0) + ((self.p0 * self.p0) + 2.0 * self.k * new_total_sol).sqrt()) / self.k;
            
            new_total_tickets - current_total_tickets
        } else {
            // Flat price case (k=0)
            sol_amount / self.p0
        }
    }
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
pub struct Initialize<'info> {

    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<Lottery>(),
        seeds = [b"lottery"],
        bump
    )]
    pub lottery:Account<'info, Lottery>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}



#[derive(Accounts)]
pub struct BuyTicket<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + std::mem::size_of::<UserTicket>(),
        seeds = [b"ticket", user.key().as_ref()],
        bump
    )]
    pub user_ticket: Account<'info, UserTicket>,

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
    pub lottery:Account<'info, Lottery>,

    #[account(
        seeds = [b"ticket", user.key().as_ref()],
        bump = user_ticket.bump,
    )]   
    pub user_ticket: Account<'info, UserTicket>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Clone)]
pub struct MagicBlock; 

impl anchor_lang::Id for MagicBlock {
    fn id() -> Pubkey {
        pubkey!("MagicBlock11111111111111111111111111111111")
    }
}
