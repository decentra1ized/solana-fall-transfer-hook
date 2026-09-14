# Solution notes

All four challenges, plus the tests that show each one actually works.

## 1. Validate the mint

`Initialize` now takes a `mint`, and the handler checks that Token-2022 owns it
before creating the account.

`InterfaceAccount<Mint>` deserializes mints from either token program, so
without that check `initialize` would happily create a rate limit for a legacy
SPL Token mint. Legacy mints cannot carry a transfer hook, so the limit would sit
there and never be enforced — an account that looks like protection and is not.

## 2. Store the mint

`RateLimit` gained a `mint: Pubkey`, set from the account added in challenge 1.
`#[derive(InitSpace)]` picks up the extra 32 bytes on its own, so `space` is
unchanged.

## 3. One limit per (mint, owner)

The PDA moved from `["rate_limit"]` to `["rate_limit", mint, owner]`.

The shared bucket was a free denial-of-service: any single holder could send
1,000,000 base units and freeze every other holder for the rest of the hour.

The seeds appear in five places and all five have to agree:

| Where | What it does |
| --- | --- |
| `initialize.rs` | creates the account (`payer` is the owner) |
| `transfer_hook.rs` | loads it during the transfer |
| `init_extra_account_meta.rs` | tells the runtime how to resolve it |
| `tests/helpers/mod.rs` | builds the instructions |
| `tests/test_initialize.rs` | derives the address on its own |

`extra_account_metas()` cannot name accounts, only index them into the Execute
instruction's fixed account list — `0 source, 1 mint, 2 destination, 3 owner,
4 meta list` — so the mint is `Seed::AccountKey { index: 1 }` and the owner is
`index: 3`.

One consequence worth flagging: each owner must call `initialize` once per mint
before their first transfer, or the hook fails with `AccountNotInitialized`.

## 4. Transferring from inside a program

`transfer.rs` is intentionally still empty, and the comment there explains why.

Putting the `transfer_checked` CPI in the hook program produces this chain:

```
hook program → Token-2022 → hook program
```

Solana allows a program to call itself directly, but not to be re-entered
*through* another program while it is still on the instruction stack. Token-2022
calls the hook as part of settling the transfer, so the runtime aborts the whole
transaction with `ReentrancyNotAllowed` before any handler code runs. There is no
guard that recovers from this — the fix has to be structural.

So the mover is a second program, `programs/token-mover`:

```
token-mover → Token-2022 → hook program
```

No program appears twice. Its `transfer_with_hook` instruction builds a plain
`transfer_checked`, appends the hook's accounts with
`add_extra_accounts_for_execute_cpi`, and invokes Token-2022.

The hook program, its `ExtraAccountMetaList` and the rate limit PDA are passed as
remaining accounts rather than named in the context, so the mover works with any
hooked mint instead of just this one. Taking the hook program id from the first
remaining account is safe: Token-2022 compares it against the id stored in the
mint's transfer hook extension and fails the transfer if they differ.

## Tests

Four existing tests, three new ones:

| Test | What it proves |
| --- | --- |
| `test_rate_limit_is_per_owner` | one holder exhausting their window does not affect another, and is still capped themselves |
| `test_token_mover_transfers` | the CPI and the resolved extra accounts are correct |
| `test_token_mover_respects_rate_limit` | the hook still runs *inside* the CPI — a transfer that skipped it would pass the previous test too |

```
anchor build   # builds both programs; the tests load the .so files
cargo test
```

On a fresh clone the generated program keypairs will not match the ids in
`declare_id!`, including the placeholder id for `token-mover`. Run `anchor keys
sync && anchor build` before the first test run.
