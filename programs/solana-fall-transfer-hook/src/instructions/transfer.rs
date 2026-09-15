//! Deliberately empty, and deliberately not declared in `mod.rs`.
//!
//! Challenge 4 reads as though the transfer belongs here — "implement a
//! transfer_checked inside your program to invoke the transfer hook". It cannot
//! live in this program.
//!
//! A `transfer_checked` on a hooked mint makes Token-2022 CPI into the mint's
//! transfer-hook program. If this program started that transfer, the call stack
//! would be:
//!
//!     solana_fall_transfer_hook -> token_2022 -> solana_fall_transfer_hook
//!
//! Solana forbids re-entrant CPI. The single exception is direct self-recursion
//! — a program invoking *itself* as the immediate next frame — and this is not
//! that, because another program sits in between. The runtime rejects it with
//! `ReentrancyNotAllowed`. No account layout or ordering fixes it: the shape of
//! the call stack is the problem.
//!
//! Verified rather than assumed — a self-CPI built here and run against LiteSVM
//! returns `InstructionError(0, ReentrancyNotAllowed)`, and the program log reads:
//!
//!     Program <hook>   invoke [1]
//!     Program <token2022> invoke [2]
//!     Program <token2022> failed: Cross-program invocation reentrancy not
//!                                 allowed for this instruction
//!
//! Note where it fails: Token-2022 refuses at frame 2, before it ever reaches the
//! hook. The runtime rejects the *call into a program already on the stack*, so
//! nothing in the hook's own logic is ever consulted.
//!
//! So the transfer has to originate from a *different* program, which is what
//! `programs/token-mover` is. Tests: `tests/test_token_mover.rs`.
//!
//! Worth being precise about what this does and does not say. Re-entrancy is
//! not "impossible by design" on Solana — this particular cycle is refused, and
//! that is a narrower claim. The hook still runs on the mover's transfer and
//! still enforces the limit: reaching the token through a program is not a way
//! around the gate, only a way to reach it.
