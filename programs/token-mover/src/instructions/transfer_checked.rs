use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022;
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

// Get these from ctx.accounts with .to_account_info()
let (source, mint, destination, owner) = /* ... */;
let hook_program_id = ctx.remaining_accounts[0].key();

// 1. Build
let mut ix = spl_token_2022::instruction::transfer_checked(
    /* token program, source, mint, destination, owner, &[], amount, decimals */
)?;

// 2. List (same order as the instruction's accounts)
let mut infos = vec![source.clone(), mint.clone(), destination.clone(), owner.clone()];

// 3. Append the hook's accounts
add_extra_accounts_for_execute_cpi(
    &mut ix, &mut infos, &hook_program_id,
    source, mint, destination, owner,
    amount, ctx.remaining_accounts,
)?;

// 4. Invoke
invoke(&ix, &infos)?;