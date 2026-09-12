// pub mod constants;
// pub mod error;
// pub mod instructions;
// pub mod state;

// use anchor_lang::prelude::*;

// pub use constants::*;
// pub use instructions::*;
// pub use state::*;

// declare_id!("EFdxPcg5vuErFteJ7H9ibvrwX32qSkgCboeX4VGn2TXV");

// #[program]
// pub mod token_mover {
//     use super::*;

//     pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
//         crate::instructions::initialize::handle_initialize(ctx)
//     }

//     pub fn increment(ctx: Context<Increment>) -> Result<()> {
//         crate::instructions::increment::handle_increment(ctx)
//     }
// }

use anchor_lang::prelude::*;

pub mod instructions;
pub use instructions::*;

declare_id!("EFdxPcg5vuErFteJ7H9ibvrwX32qSkgCboeX4VGn2TXV");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
    ctx: Context<'info, TransferWithHook<'info>>,
    amount: u64,
) -> Result<()> {
    instructions::transfer_with_hook::handler(ctx, amount)
}
}
