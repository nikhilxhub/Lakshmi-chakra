use anchor_lang::prelude::*;

declare_id!("BYe1eVU9XeUeezxyrUN7L9zfWBhcjGAYugmbzhf6L1ze");

#[program]
pub mod lakshmi_chakra {
    use super::*;


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
}

#[account]
#[delegate]
pub struct UserTicket {
    pub owner: Pubkey,
    pub tickets: f64,
    pub bump:u8,
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
}


