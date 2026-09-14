# Challenge walkthrough

The base program had **one** rate-limit bucket for everyone. The challenges turn that into **one bucket per user, per mint**, then show why you cannot trigger the hook by transferring from inside the same program.

Run this after every code change:

```bash
anchor build
cargo test
```

Tests load `target/deploy/solana_fall_transfer_hook.so`. If you skip `anchor build`, they run yesterday’s program.

---

## 0. Sync the program id first

The mint’s Transfer Hook extension stores `crate::ID` at compile time (`initialize_mint.rs`). If that constant does not match the keypair you deploy, Token-2022 CPIs to the wrong address.

```bash
solana-keygen pubkey target/deploy/solana_fall_transfer_hook-keypair.json
# must equal declare_id! in lib.rs and Anchor.toml

anchor keys sync
anchor build
```

We synced to `HxffLoMCWNSaodVfUJwh1pKcy2EHcucRDmaGteBwXmo3`.

---

## Challenge 1 — validate the mint

**Problem.** `initialize` created a rate-limit account without checking it was pointed at a real Token-2022 mint. Anyone could pass garbage.

**What we did.**

1. Added `mint` to the `Initialize` context (`InterfaceAccount<Mint>`).
2. In the **handler** (not only as an Anchor constraint), checked the account owner:

```rust
require_keys_eq!(
    *ctx.accounts.mint.to_account_info().owner,
    token_2022::ID,
    ErrorCode::InvalidMint
);
```

`InterfaceAccount<Mint>` accepts both classic Token and Token-2022. The owner check is what rejects a Tokenkeg mint.

**How to know it works.** `test_initialize_rejects_legacy_token_mint` creates a real SPL Token mint and expects `Invalid mint account`.

---

## Challenge 2 — remember the mint on the account

**Problem.** The PDA seeds will bind the account to a mint, but the account data itself did not record which mint it belongs to. Indexers / later instructions would have to re-derive it.

**What we did.** Added `mint: Pubkey` to `RateLimit` and set it in `set_inner`. `#[derive(InitSpace)]` grows the account size; the `space = 8 + RateLimit::INIT_SPACE` constraint picks that up automatically.

**How to know it works.** `test_initialize_persists_mint_and_authority` deserializes the account and checks `rate_limit.mint == mint.pubkey()`.

---

## Challenge 3 — one bucket per mint and per owner

**Problem.** The PDA was `["rate_limit"]`. One account. Alice transferring 1_000_000 locked Bob out.

**What we did.** New seeds everywhere:

```
["rate_limit", mint, owner]
```

Those four (five) sites must be identical. If one is stale, you get `ConstraintSeeds` or “account not found”.

| Site | How the seeds are written |
|---|---|
| `initialize.rs` | `seeds = [b"rate_limit", mint.key().as_ref(), owner.key().as_ref()]` |
| `transfer_hook.rs` | same |
| `tests/helpers/mod.rs` | `find_program_address(&[b"rate_limit", mint, owner], program_id)` |
| `test_initialize.rs` | same (README forgot this file) |
| `extra_account_metas()` | see below — **indices**, not names |

### Why indices, not names

At transfer time Token-2022 does not call your `Initialize` accounts struct. It builds the hook’s **Execute** instruction and asks ExtraAccountMetaList: “derive the extra PDA from *these* accounts.”

Execute account order (fixed by the SPL interface):

```
0  source token
1  mint          ← Seed::AccountKey { index: 1 }
2  destination
3  owner         ← Seed::AccountKey { index: 3 }
4  extra-account-metas
5+ extras (rate_limit is resolved here)
```

That is the same order as the `TransferHook` struct. So:

```rust
ExtraAccountMeta::new_with_seeds(
    &[
        Seed::Literal { bytes: b"rate_limit".to_vec() },
        Seed::AccountKey { index: 1 }, // mint
        Seed::AccountKey { index: 3 }, // owner
    ],
    false,
    true,
)?;
```

Swap 1 and 3 and Token-2022 derives `[rate_limit, owner, mint]`. Your program looks up `[rate_limit, mint, owner]`. Mismatch.

We also added an explicit `owner` account on `Initialize`. If you seed with `payer` instead, a relayer who pays rent binds the bucket to themselves, not to Alice.

**How to know it works.**

- Two owners, same mint: Alice can still transfer after Bob fills his cap.
- Two mints, same owner: filling mint A does not cap mint B.
- Missing init for that (mint, owner): transfer fails.

---

## Challenge 4 — transfer inside the program, and re-entrancy

This is two different bugs with the same word.

### Bug A — Anchor write-back (the one you *can* fix)

If the outer instruction types `source_token`, `destination_token`, or `rate_limit` as `Account` / `InterfaceAccount` + `mut`, Anchor:

1. Deserializes them at the start.
2. Token-2022 / the hook mutate the real account data.
3. On exit, Anchor writes the **stale copies** back.

That undoes the transfer and fights `amount_transferred`.

**Fix.** Those accounts are `UncheckedAccount`. Anchor does not deserialize/reserialize them. `mint` stays `InterfaceAccount` — Token-2022 does not mutate mint data on a normal transfer.

### Bug B — SVM re-entrancy (the one you *cannot* fix in this program)

A transfer hook mint always does this:

```
someone
  └── Token-2022.transfer_checked
        └── your program.transfer_hook     // Token-2022 CPIs here
```

If “someone” is **the same program** that owns the hook:

```
your program.transfer_checked
  └── Token-2022.transfer_checked
        └── your program.transfer_hook     // re-enter YOURSELF
```

The runtime returns `ReentrancyNotAllowed`. Token-2022 is not allowed to CPI back into a program that is already on the stack. This is not LiteSVM-only. It fails on a real validator too.

**What “deal with re-entrancy” means here.**

You do **not** make that CPI succeed. You stop trying.

| Who starts the transfer | Hook runs? | Why |
|---|---|---|
| Wallet / client calls Token-2022 `transfer_checked` | yes | Your program is not on the stack yet. Token-2022 is the caller. |
| Another program (not the hook) CPIs Token-2022 | yes | Same reason — hook program is entered once. |
| This program CPIs Token-2022 | **no** | Re-enter the hook program. Runtime aborts. |

So the working product path is the one in `build_transfer_with_hook_ix`: the **client** sends Token-2022 `transfer_checked` and appends `hook program + extra-metas + rate_limit`.

`program_transfer.rs` is the experiment. It builds the CPI correctly (`UncheckedAccount` + `add_extra_accounts_for_execute_cpi`) and then dies on re-entrancy. `test_program_transfer_reentrancy_rejected` locks that in so nobody “fixes” it by deleting the test.

If you later need an on-chain transfer of these tokens, put that instruction on a **second program**. That second program CPIs Token-2022. Token-2022 CPIs the hook. Two different program ids — no re-entrancy.

### Naming note

Anchor 1.2 cannot have `mod transfer` + `fn transfer` + `struct Transfer` in the same crate. The `#[program]` macro dies with `unresolved import crate`. The wrapper is therefore `transfer_checked` / `ProgramTransfer`.

---

## End-to-end flow (the one that works)

1. `initialize_mint` — Token-2022 mint with Transfer Hook pointing at `crate::ID`.
2. `initialize` — create `RateLimit` PDA `["rate_limit", mint, owner]`. Check mint owner. Store mint.
3. `initialize_extra_account_meta_list` — tell Token-2022 how to derive that PDA at transfer time (indices 1 and 3).
4. Create ATAs, mint tokens.
5. Client sends Token-2022 `transfer_checked` + extra accounts.
6. Token-2022 sets `transferring = true`, CPIs `transfer_hook`, then moves the tokens.
7. Hook: reject if not transferring; reset window if expired; reject if over cap; else add `amount`.

