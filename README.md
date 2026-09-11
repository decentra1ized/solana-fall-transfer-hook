# Rate Limit Transfer Hook

This example demonstrates how to implement a transfer hook using the SPL Token 2022 Transfer Hook interface to enforce rate limiting on token transfers.

In this example, every token transfer is validated against a rate limit. If the cumulative amount transferred within a time window exceeds the configured maximum, the transfer is rejected - providing automatic, on-chain throttling of token movements.

The rate limit is scoped to each mint and source owner. A separate `token-mover`
program performs program-initiated transfers through Token-2022 so the hook can
run without re-entering itself.

---

## Let's walk through the architecture:

For this program, we will have 1 main state account:

- A RateLimit account

A RateLimit account consists of:

```rust
#[account]
#[derive(InitSpace)]
pub struct RateLimit {
    pub authority: Pubkey,
    pub max_amount: u64,
    pub window_start: i64,
    pub amount_transferred: u64,
}
```

### In this state account, we will store:

- authority: The public key of the account that initialized (and controls) this rate limit.
- max_amount: The maximum cumulative amount that can be transferred within a single window.
- window_start: The Unix timestamp at which the current window opened, used to determine when the window expires.
- amount_transferred: The cumulative amount transferred so far within the current window.

> The `mint` field binds each rate limit account to the Token-2022 mint it controls.

The rate limit uses a **fixed window**: `window_start` is set when the window opens and is never moved by transfers. Once more than 3600 seconds (1 hour) have elapsed since `window_start`, the next transfer opens a fresh window with a zeroed total. This is a deliberate design choice - if every transfer refreshed the timestamp instead (a sliding window keyed on "last activity"), a holder transferring at least once per hour would keep the window alive forever and their running total would never reset, permanently capping an active account.

---

### The admin will first create a Token-2022 mint with transfer hook extensions. For that, we create the following context:

```rust
#[derive(Accounts)]
pub struct InitializeMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        mint::decimals = 9,
        mint::authority = payer,
        extensions::permanent_delegate::delegate = payer,
        extensions::transfer_hook::authority = payer,
        extensions::transfer_hook::program_id = crate::ID,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}
```

Let's have a closer look at the accounts that we are passing in this context:

- payer: The account paying for the mint creation. He will be a signer of the transaction, and we mark his account as mutable as we will be deducting lamports from this account.

- mint: The Token-2022 mint account to be created. Anchor initializes it with 9 decimals, sets the payer as the mint authority and permanent delegate, and configures the transfer hook extension to point to our program.

- system_program: Program responsible for the initialization of any new account.

- token_program: The Token-2022 program that will manage this mint.

---

### The admin will then create the RateLimit account. For that, we create the following context:

```rust
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        seeds = [b"rate_limit", mint.key().as_ref(), payer.key().as_ref()],
        bump,
        space = ANCHOR_DISCRIMINATOR_SIZE + RateLimit::INIT_SPACE,
    )]
    pub rate_limit: Account<'info, RateLimit>,
    pub system_program: Program<'info, System>,
}
```

Let's have a closer look at the accounts that we are passing in this context:

- payer: Will be the person creating the rate limit account. He will be a signer of the transaction, and we mark his account as mutable as we will be deducting lamports from this account.

- rate_limit: The state account that we will initialize. Its PDA is derived from the rate-limit seed, mint, and payer, so each mint/owner pair has an independent account.

- system_program: Program responsible for the initialization of any new account.

### We then implement the handler for Initialize:

```rust
pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    ctx.accounts.rate_limit.set_inner(RateLimit {
        authority: ctx.accounts.payer.key(),
        max_amount: RateLimit::MAX_AMOUNT,
        window_start: Clock::get()?.unix_timestamp,
        amount_transferred: 0,
    });

    Ok(())
}
```

In here, we set the initial data of our RateLimit account with the authority, mint, a maximum transfer amount of 1,000,000, the current timestamp, and zero amount transferred. The handler rejects accounts that are not owned by Token-2022.

---

### The system will need to initialize extra account metadata for the transfer hook:

```rust
#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    payer: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    /// CHECK: ExtraAccountMetaList Account, will be initialized in this instruction
    #[account(
        init,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
        space = ExtraAccountMetaList::size_of(extra_account_metas()?.len()).unwrap(),
        payer = payer
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
```

In this context, we are passing all the accounts needed to set up the transfer hook metadata:

- payer: The address paying for the initialization. He will be a signer of the transaction, and we mark his account as mutable as we will be deducting lamports from this account.

- mint: The token mint that will have the transfer hook enabled.

- extra_account_meta_list: The account that will store the extra metadata required for the transfer hook. This account is derived from the byte representation of "extra-account-metas" and the mint's public key.

- system_program: Program responsible for the initialization of any new account.

### We then define the extra account metas for the transfer hook:

```rust
pub fn extra_account_metas() -> Result<Vec<ExtraAccountMeta>> {
    Ok(vec![
        ExtraAccountMeta::new_with_seeds(
            &[
                Seed::Literal { bytes: b"rate_limit".to_vec() },
            ],
            false,  // is signer
            true,   // is writable
        )?,
    ])
}
```

In here, we define the extra accounts that will be required during transfer hook execution. We use `new_with_seeds` to let the runtime derive the rate_limit PDA at transfer time. In the base version there is a single literal seed, so this always resolves to the one program-wide rate limit account.

This is the key file for the seed [Challenge](#challenges): the seeds declared here are what the runtime (and the client) use to resolve the account when a transfer happens, so they must match the seeds used everywhere the account is created and loaded. Adding extra seeds here to reference the mint and owner is what makes the rate limit per-user.

---

### The transfer hook will validate every token transfer:

```rust
#[derive(Accounts)]
pub struct TransferHook<'info> {
    #[account(
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: source token account owner, can be SystemAccount or PDA owned by another program
    pub owner: UncheckedAccount<'info>,
    /// CHECK: ExtraAccountMetaList Account
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"rate_limit", mint.key().as_ref(), owner.key().as_ref()],
        bump,
    )]
    pub rate_limit: Account<'info, RateLimit>,
}
```

In this context, we are passing all the accounts needed for transfer validation:

- source_token: The token account from which tokens are being transferred. We validate that it belongs to the correct mint and is owned by the owner.

- mint: The token mint being transferred.

- destination_token: The token account to which tokens are being transferred. We validate that it belongs to the correct mint.

- owner: The owner of the source token account. This can be a system account or a PDA owned by another program.

- extra_account_meta_list: The metadata account that contains information about extra accounts required for this transfer hook.

- rate_limit: The per-mint/per-owner rate limit account being enforced. Its seeds must match those declared in `extra_account_metas()` so the runtime passes in the same account. We mark it as mutable because we update the transferred amount on each successful transfer.

### We then implement the transfer hook handler:

```rust
pub fn handler(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
    check_is_transferring(&ctx)?;

    let current_time = Clock::get()?.unix_timestamp;
    if ctx.accounts.rate_limit.is_expired(current_time, ONE_HOUR) {
        ctx.accounts.rate_limit.reset(current_time);
        msg!("Rate limit window expired - opening a new window");
    }

    match ctx.accounts.rate_limit.limit_exceeded(amount) {
        true => {
            msg!("Transfer amount exceeds the rate limit");
            return Err(error!(ErrorCode::RateLimitExceeded));
        },
        false => {
            ctx.accounts.rate_limit.update(amount);
            msg!("Transfer amount is within the rate limit, proceeding with transfer");
        }
    }

    Ok(())
}
```

In this implementation, we first verify that the hook is being called during an actual transfer operation by checking the `TransferHookAccount` extension's `transferring` flag. Then we check if the current window has expired (more than 1 hour since `window_start`) - if so, we open a fresh window with a zeroed total. Finally, we validate whether the cumulative transferred amount plus the current transfer would exceed the maximum, using saturating arithmetic so a huge `amount` cannot wrap around `u64` and sneak under the cap. If the limit would be exceeded, we reject the transfer with a `RateLimitExceeded` error; otherwise, we record the amount against the window (without touching `window_start`) and allow the transfer to proceed.

The `check_is_transferring` function reads the source token account's data to inspect the `TransferHookAccount` extension:

```rust
fn check_is_transferring(ctx: &Context<TransferHook>) -> Result<()> {
    let source_token_info = ctx.accounts.source_token.to_account_info();
    let account_data_ref: Ref<&mut [u8]> = source_token_info.try_borrow_data()?;
    let account = PodStateWithExtensions::<PodAccount>::unpack(*account_data_ref)?;
    let account_extension = account.get_extension::<TransferHookAccount>()?;

    require!(
        bool::from(account_extension.transferring),
        ErrorCode::NotTransferring
    );

    Ok(())
}
```

This ensures the transfer hook can only be executed as part of a Token-2022 transfer, preventing direct invocation. Note that we return a proper Anchor error (`NotTransferring`) rather than panicking - a panic aborts the program with an opaque SBF error, while an Anchor error surfaces a clear error code and message to the client.

---

This rate limit transfer hook provides an automatic throttling mechanism for Token 2022 mints, ensuring that no more than a configured maximum amount can be transferred within a fixed time window - all enforced on-chain without requiring additional user intervention.

---

## Completed extensions

The implementation includes the four extensions described by the original
exercise:

1. **Validate the mint.** `initialize` rejects accounts not owned by Token-2022.

2. **Remember the mint on the account.** Each `RateLimit` stores its mint.

3. **Make the rate limit per-mint and per-owner.** The PDA uses the mint and
    owner in all four required locations:
   - `extra_account_metas()` in `init_extra_account_meta.rs` - what the runtime uses to resolve the account at transfer time,
   - the `Initialize` context in `initialize.rs` - where the account is created,
   - the `TransferHook` context in `transfer_hook.rs` - where the account is loaded,
   - the test helpers in `tests/helpers/mod.rs`.

   In `extra_account_metas()` the extra seeds don't reference the mint and owner by name - they reference accounts by their **index** in the transfer hook's execute instruction. Work out which index is the mint and which is the owner.

4. **Make program transfers.** The `token-mover` workspace program performs a
    Token-2022 `transfer_checked` CPI and resolves the hook's extra accounts.
