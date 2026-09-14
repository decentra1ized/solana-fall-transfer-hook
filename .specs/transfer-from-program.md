# Transfer from another program

## Problem and outcome

Applications must be able to move Token-2022 tokens through a CPI while the existing transfer hook continues enforcing its per-user, per-mint rate limit.

## Scope

- Add a separate on-chain mover program to avoid re-entering the hook program.
- Forward the hook accounts supplied by the caller as remaining accounts.
- Cover successful and rate-limited CPI transfers with LiteSVM tests.

## Non-goals

- Change the hook's limit calculation or account layout.
- Hard-code this hook's PDA layout inside the mover program.

## Acceptance criteria

- A transfer of 100 base units through the mover succeeds.
- A transfer of 1,000,000 base units through the mover succeeds.
- A subsequent transfer of 1 base unit through the mover fails with `RateLimitExceeded` (`0x1771`).
- Existing hook tests continue to pass.

## Assumptions and decisions

- The caller supplies the hook program, extra-account-meta list, and resolved hook accounts as remaining accounts.
- The mover uses `add_extra_accounts_for_execute_cpi` so it remains generic across transfer hooks.

## Technical plan

Create a second Anchor program that builds a Token-2022 `transfer_checked` instruction, resolves its hook accounts from the provided remaining accounts, and invokes Token-2022. Load both programs in LiteSVM and exercise the mover through generated Anchor instruction/account types.

## Tasks

- Add and register the mover program.
- Implement the CPI transfer instruction.
- Add test transaction construction and two end-to-end tests.
- Build both SBF programs and run the complete test suite.

## Verification

- `anchor build`: both SBF programs build successfully.
- `cargo test --workspace`: all seven integration tests pass, including both mover scenarios.
- `cargo fmt --check`: passes.
- `git diff --check`: passes.
