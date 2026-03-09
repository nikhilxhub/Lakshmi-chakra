use anchor_lang::prelude::*;

declare_id!("BYe1eVU9XeUeezxyrUN7L9zfWBhcjGAYugmbzhf6L1ze");

#[program]
pub mod lakshmi_chakra {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
