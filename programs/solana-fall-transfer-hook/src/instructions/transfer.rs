// Challenge 4 - deliberately left without an implementation.
//
// The obvious move is to put a `transfer_checked` CPI right here, in the hook
// program, and hand Token-2022 the hook's extra accounts. That builds and
// deploys fine, and then every transfer fails at runtime with
// `ReentrancyNotAllowed`:
//
//     hook program  ->  Token-2022  ->  hook program
//
// Solana permits a program to call itself directly (A -> A), but not to be
// re-entered *through* another program while it is still on the instruction
// stack (A -> B -> A). Since this program is the mint's transfer hook, any CPI
// it makes into Token-2022 comes straight back into it, and the runtime rejects
// the whole transaction before our handler ever runs - so there is no guard we
// could add here to recover.
//
// The fix is structural rather than defensive: the instruction that initiates
// the transfer has to live in a *different* program, so no program appears
// twice on the stack:
//
//     token-mover  ->  Token-2022  ->  hook program
//
// That program is `programs/token-mover` (`transfer_with_hook`). The hook is
// unchanged and still runs inside the CPI, which the
// `test_token_mover_respects_rate_limit` test proves.
