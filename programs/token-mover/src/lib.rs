pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("Fz9KWivnsnb5V2U3DcGYWpgyzFBN3od31EqzMRd6UrqM");

/// A deliberately small program whose only job is to move Token-2022 tokens by
/// CPI, so that a transfer hook fires with a program — not a wallet — as the
/// authority.
///
/// This exists as a separate program on purpose. Calling Token-2022's
/// `transfer_checked` from inside the hook program itself would re-enter that
/// same program through the hook and Token-2022 rejects it with
/// `ReentrancyNotAllowed`. Moving the call into another program breaks the
/// hook -> hook chain into mover -> hook.
#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        transfer::handler(ctx, amount)
    }
}
