# What I changed and why

This repo started as a Token-2022 transfer hook that rate limits transfers:
no more than 1,000,000 base units may move in any one-hour window. The
starting code worked, but took three deliberate shortcuts, and left a fourth
problem unsolved. This document explains what each of the four changes was,
in plain words, and which files it touched.

All 11 tests pass. To run them:

```sh
anchor build --ignore-keys
cargo test
```

`anchor build` must come first: the tests load the compiled `.so` file, so
without a fresh build they run against stale code.

---

## Background: what a transfer hook is

Token-2022 is the newer of Solana's two token programs. Unlike the original,
it supports *extensions* - optional features attached to a mint. One of them
is the **transfer hook**: a program that Token-2022 calls automatically on
every single transfer of that token. If the hook returns an error, the
transfer does not happen.

That makes it an enforcement point. This one enforces a spending cap.

The cap lives in a `RateLimit` account, which tracks how much has moved in
the current window and when that window opened.

---

## Challenge 1 - check that the mint is really a Token-2022 mint

**The problem.** The `initialize` instruction created a `RateLimit` account
without taking a mint at all. Nothing connected the rate limit to a token.
You could point it at a mint from the *old* token program - and old-style
mints cannot have transfer hooks, so the hook would never fire. The result
is an account that looks like working protection but enforces nothing,
silently.

**The fix.** `initialize` now takes the mint as an account and checks who
owns it. On Solana every account has an owner - the program allowed to change
it - and for a mint that owner is whichever token program created it. The
check requires that owner to be Token-2022.

The non-obvious part: the account type used here, `InterfaceAccount<Mint>`,
accepts a mint from *either* token program by design. So adding the account
alone proves nothing. The explicit owner check is what does the work.

**Files touched**

| File | What changed |
| --- | --- |
| `src/instructions/initialize.rs` | added the `mint` account and the owner check |
| `tests/helpers/mod.rs` | pass the mint when calling initialize |
| `tests/test_initialize.rs` | same, plus a new test |

**Test added.** `test_initialize_rejects_legacy_spl_token_mint` builds a real
old-style mint and confirms initialize refuses it. It checks for the specific
`InvalidMint` error rather than just "the transaction failed", so it cannot
quietly pass for some unrelated reason later.

---

## Challenge 2 - record which mint the rate limit belongs to

**The problem.** The `RateLimit` account knew its owner, its cap and its
window, but not which token it governed. Nothing on-chain tied it to a mint.

**The fix.** A `mint` field on the struct, set when the account is created.

Worth knowing: the account's size did not need updating. It is computed from
`RateLimit::INIT_SPACE`, which is generated automatically from the struct, so
adding the field grew the account by the right 32 bytes on its own. Had the
size been a hardcoded number, this one-line change would have quietly
corrupted the account.

**Files touched**

| File | What changed |
| --- | --- |
| `src/state/rate_limit.rs` | added the `mint` field |
| `src/instructions/initialize.rs` | set it when creating the account |
| `tests/test_initialize.rs` | new test |

**Test added.** `test_initialize_records_the_mint` reads the account back off
the chain and deserializes it, confirming the mint is genuinely stored rather
than just present in the Rust type.

---

## Challenge 3 - one rate limit per person, per token

**The problem.** This was the big one. The rate limit account's address was
derived from a single fixed label, `"rate_limit"`. That means there was
exactly **one** rate limit account for the entire program - shared by every
holder of every token. The first person to spend the 1,000,000 allowance
froze everybody else for the rest of the hour.

**The fix.** Derive the address from three things instead: the label, the
mint, and the owner. Now every person gets their own independent allowance
for each token they hold.

**Why this is the hard one.** The address is computed in six different places,
and every one must compute it identically. If any of them disagrees, it
produces a different (but perfectly valid-looking) address, and the failure
shows up much later as an unhelpful error that points at nothing.

The genuinely tricky place is `init_extra_account_meta.rs`. The other five
can name the mint and owner directly, because they have those accounts in
hand. That one cannot: it writes a recipe that gets stored on-chain and
followed later by the runtime, during a transfer. At that moment there are no
names - only the list of accounts in the transfer. So the recipe refers to
them **by position** in that list:

```
0 = source account   1 = mint   2 = destination   3 = owner
```

Hence "index 1" for the mint and "index 3" for the owner. Get a number wrong
and nothing fails at compile time - it just computes the wrong address.

**Files touched**

| File | What changed |
| --- | --- |
| `src/instructions/init_extra_account_meta.rs` | the on-chain recipe, using positions |
| `src/instructions/initialize.rs` | where the account is created |
| `src/instructions/transfer_hook.rs` | where the account is read during a transfer |
| `tests/helpers/mod.rs` | two places, plus shared helpers |
| `tests/test_initialize.rs` | three direct address calculations |

**Tests added.** The six existing tests all passed the moment the change was
made - which proved nothing, because every one of them transfers as a single
person and would have passed against the old shared bucket too.

So `test_rate_limit_is_per_owner` does the real work: one person spends their
entire allowance and gets cut off, then a second person still transfers the
full amount. Under the old design that is impossible.

`test_transfer_fails_without_an_initialized_rate_limit` pins down a side
effect worth knowing about: **every holder must now call `initialize` once
for each token they hold.** Someone who skips it has no rate limit account,
and their transfers are refused outright rather than allowed through
unlimited. That is the safe direction to fail, but it is a real change in how
the program is used.

---

## Challenge 4 - transferring from inside a program

**The problem.** Sometimes a program, not a person, needs to move tokens. The
obvious approach is to put the transfer call inside the hook program itself.

That approach cannot work, and the reason is worth understanding. When a
program calls another program, they stack up. Here the stack would be:

```
hook program  ->  Token-2022  ->  hook program
```

because Token-2022 calls the hook as part of doing the transfer. Solana
forbids a program from appearing twice in that stack - it is called
re-entrancy, and the transaction is rejected outright.

**The fix.** Move the transfer into a *separate* program. The new
`token-mover` program does the transfer, so the stack becomes:

```
token-mover  ->  Token-2022  ->  hook program
```

Nothing appears twice, so it is allowed - and the hook still runs, so the
rate limit still applies. Going through a program is not a way around it.

**Two details that matter.**

First, the obvious helper for this (`anchor_spl`'s `transfer_checked`) is a
trap. Reading its source shows it builds the call with only four accounts and
**silently throws away any extras**. For a token with a hook, that means
Token-2022 calls the hook without the rate limit account it needs. So
`token-mover` uses a purpose-built helper from the transfer hook library that
attaches the extra accounts correctly.

Second, `token-mover` reads *which* hook program to use from the mint itself,
rather than accepting it as an argument from the caller. The mint is the
authority on that. Taking a caller's word for it would let anyone point the
program at a permissive hook of their own and walk straight past the limit.

**Files touched**

| File | What changed |
| --- | --- |
| `programs/token-mover/src/lib.rs` | new program - the working solution |
| `programs/token-mover/Cargo.toml` | new |
| `Anchor.toml` | registered the new program |
| `src/instructions/transfer.rs` | the version that hits the re-entrancy wall |
| `src/instructions/mod.rs`, `src/lib.rs` | wired that instruction in |
| `programs/solana-fall-transfer-hook/Cargo.toml` | test dependency on token-mover |
| `tests/helpers/mod.rs` | helpers to deploy and call the new program |

`src/instructions/transfer.rs` shipped empty in the starting code. I filled it
in with the version that *does* live inside the hook program, and kept it,
together with a test proving it is rejected. It is easier to trust a wall you
have walked into than one you have only read about.

**Tests added.**

- `test_in_program_transfer_is_rejected_as_reentrant` - confirms the wall is
  real. The error is `ReentrancyNotAllowed`, and the logs show the stack.
- `test_move_tokens_via_cpi` - the transfer works through `token-mover`, and
  the hook genuinely ran. It checks the recorded amount afterwards, not just
  that the transaction succeeded. A success-only check would pass even if the
  hook had been skipped entirely - which is exactly what the dropped-accounts
  trap above would cause.
- `test_mover_still_enforces_the_rate_limit` - confirms a program-initiated
  transfer is capped identically to a normal one.

---

## Notes

**On the tests.** Several of these changes are the kind that look obviously
correct and pass all existing tests while doing nothing. The mint check could
have been a no-op; the stored mint could have failed to persist; the per-user
buckets change passed the whole existing suite immediately. In each case the
new test is the one that would actually notice. Where a test asserts a
failure, it asserts on the *specific* error, because "it failed" is weak
evidence - things fail for all sorts of reasons.

**On building.** The build uses `--ignore-keys`. A fresh clone generates a new
program key that does not match the one written in the source, and this flag
keeps the original rather than rewriting the source with a local key. Nothing
depends on which is used; the tests are consistent either way.
