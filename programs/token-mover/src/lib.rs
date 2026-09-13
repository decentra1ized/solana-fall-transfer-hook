pub mod transfer;

use anchor_lang::prelude::*;
pub use transfer::*;

declare_id!("FrrQBj9Yfu6jijoGFoL79GxyNrXtqYWH4CLd6Zi9ZL95");

#[program]
pub mod token_mover {
    use super::*;

    // 'info is named here AND on the handler: the CPI helper needs the
    // account infos and remaining_accounts to share one lifetime.
    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        transfer::handler(ctx, amount)
    }
}
