pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;


use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("Fta1Qjqe1ot1LLWnMJrSXm6KfWVozspC3BC1PkBwzQTd");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'a>(
        ctx : Context<'a,TransferWithHook<'a>>, 
        amount:u64,
        decimals:u8
    ) -> Result<()> {
        instructions::transfer::handler(ctx, amount,decimals)
    }
}
