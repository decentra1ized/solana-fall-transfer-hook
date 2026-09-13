# Rate Limit Transfer Hook

This example demonstrates how to implement a transfer hook using the SPL Token 2022 Transfer Hook interface to enforce rate limiting on token transfers.

In this example, every token transfer is validated against a rate limit. If the cumulative amount transferred within a time window exceeds the configured maximum, the transfer is rejected - providing automatic, on-chain throttling of token movements.

The base implementation is intentionally minimal: it uses a single, program-wide rate limit account (derived from just the `"rate_limit"` seed) that every transfer shares. Turning this into a proper per-mint, per-user rate limit is left to you as a set of guided exercises - look for the `CHALLENGE` comments in the code and the [Challenges](#challenges) section at the end.

> **Note:** the walkthrough below describes the *base* design as originally written, so
> the snippets showing a single `["rate_limit"]` seed and a `RateLimit` without a `mint`
> field no longer match the code. All four challenges are implemented on this branch —
> see [Challenges](#challenges) for what actually changed and why.

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
    pub mint: Pubkey,          // added in challenge 2
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

- mint: The mint this rate limit applies to. The base version omitted this to keep the account small; it was added in [challenge 2](#2-remember-the-mint-on-the-account--done).

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
        seeds = [b"rate_limit"],
        bump,
        space = ANCHOR_DISCRIMINATOR_SIZE + RateLimit::INIT_SPACE,
    )]
    pub rate_limit: Account<'info, RateLimit>,
    pub system_program: Program<'info, System>,
}
```

Let's have a closer look at the accounts that we are passing in this context:

- payer: Will be the person creating the rate limit account. He will be a signer of the transaction, and we mark his account as mutable as we will be deducting lamports from this account.

- rate_limit: The state account that we will initialize. In the base version we derive the RateLimit PDA from the single seed `["rate_limit"]`, so there is exactly one rate limit account for the whole program. Making it unique per mint and per user is a [Challenge](#challenges).

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

In here, we set the initial data of our RateLimit account with the authority, a maximum transfer amount of 1,000,000, the current timestamp, and zero amount transferred. Notice the base handler does not take a `mint` account and does not validate it - passing the mint in and checking it is owned by the Token-2022 program is one of the [Challenges](#challenges).

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
        seeds = [b"rate_limit"],
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

- rate_limit: The rate limit account being enforced. In the base version this is the single program-wide account; its seeds must match those declared in `extra_account_metas()` so the runtime passes in the same account. We mark it as mutable because we will update the transferred amount on each successful transfer.

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

## Challenges

> **Status: all four are implemented on this branch.** The notes below record what
> changed and, where the answer was not the obvious one, why.

The base program took a deliberate shortcut: a single, global rate limit shared by
every user and every mint. That is now a per-mint, per-owner bucket.

### 1. Validate the mint — done

`Initialize` now takes a `mint` account, and the handler checks it is owned by the
Token-2022 program:

```rust
require_keys_eq!(
    *ctx.accounts.mint.to_account_info().owner,
    token_2022::ID,
    ErrorCode::InvalidMint
);
```

`InterfaceAccount<Mint>` alone is not enough — it accepts a legacy SPL Token mint
too. Checking the owning program is the meaningful test, because the transfer hook
extension only exists under Token-2022: a rate limit opened against a legacy mint
would look valid while never being enforced. `test_initialize_rejects_non_token_2022_mint`
writes a real legacy mint into the SVM to prove the check bites.

### 2. Remember the mint on the account — done

`RateLimit` gained a `mint: Pubkey` field, set in the handler. `InitSpace` recomputes
the account size automatically.

### 3. Per-mint and per-owner — done

The PDA is now `["rate_limit", mint, owner]`, updated in all four places that must
agree: `extra_account_metas()`, the `Initialize` context, the `TransferHook` context,
and the test helpers.

The subtle part is `extra_account_metas()`, where seeds reference accounts by **index**
into the Execute instruction rather than by name. The transfer hook interface fixes
that order as:

| Index | Account |
| --- | --- |
| 0 | source token account |
| 1 | **mint** |
| 2 | destination token account |
| 3 | **owner / source authority** |

so the extra seeds are `AccountKey { index: 1 }` and `AccountKey { index: 3 }`. Getting
these wrong does not fail to compile — it silently resolves a different PDA and the
transfer fails with a seeds constraint violation.

### 4. Transfer inside the program, and re-entrancy — done, with a caveat worth reading

`transfer.rs` implements a `transfer_checked` CPI that resolves and appends the hook's
extra accounts via `add_extra_accounts_for_execute_cpi`.

The re-entrancy question has a sharper answer than "add a guard". If this program CPIs
into Token-2022 for a mint whose hook is *this same program*, the chain is:

```
this program (depth 1) -> Token-2022 (depth 2) -> this program's hook (depth 3)
```

Solana's runtime rejects that outright with `ReentrancyNotAllowed`: a program already on
the invocation stack cannot be re-entered from a deeper frame. This is enforced by the
runtime, not by Token-2022, so it cannot be worked around from program code.

So the honest result is:

- The instruction works for the normal composition case — moving tokens for a mint whose
  hook belongs to a *different* program.
- Pointed at its own hooked mint, it **fails loudly** with `ReentrancyNotAllowed` rather
  than silently transferring without running the hook. That ordering matters: a transfer
  that skipped its own rate limit would be the dangerous outcome; refusing to execute is
  the safe one. `test_program_transfer_rejects_self_reentrancy` pins that exact error and
  asserts nothing moved.
- The hook itself is unaffected on the ordinary top-level path, where Token-2022 is at
  depth 1 and the hook runs at depth 2 — `test_top_level_transfer_still_runs_the_hook`
  covers that.

The second layer of protection is the one already in the base program:
`check_is_transferring` requires Token-2022's `transferring` flag, which is set only for
the duration of a real transfer. That is what stops anyone calling `transfer_hook`
directly to burn through someone else's window.

## Tests

```bash
anchor build && cargo test   # 10 passing
```

Run `anchor build` before `cargo test` — the harness embeds the compiled program with
`include_bytes!`, so the tests otherwise run against a stale binary.

| Test | Covers |
| --- | --- |
| `test_initialize` | base initialization still works |
| `test_initialize_extra_account_meta_list` | metas account is created |
| `test_transfer_hook` | a transfer within the limit succeeds |
| `test_transfer_hook_rate_limit_exceeded` | exceeding the window is rejected |
| `test_initialize_rejects_non_token_2022_mint` | challenge 1 |
| `test_initialize_accepts_token_2022_mint` | challenge 1, the positive case |
| `test_rate_limit_is_per_owner` | challenge 3 — two owners do not share a bucket |
| `test_rate_limit_is_per_mint` | challenge 3 — one owner, two mints |
| `test_program_transfer_rejects_self_reentrancy` | challenge 4 |
| `test_top_level_transfer_still_runs_the_hook` | challenge 4, the working path |

The per-owner and per-mint tests were checked by mutation: reverting the owner seed makes
`test_rate_limit_is_per_owner` fail while `test_rate_limit_is_per_mint` still passes, and
removing the Token-2022 owner check fails only the mint test. They discriminate rather
than passing vacuously.
