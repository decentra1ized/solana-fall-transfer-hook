//! Challenge 4 — transferring from a program.
//!
//! The hook cannot CPI into Token-2022 itself: Token-2022 would call straight
//! back into it and the runtime rejects that with `ReentrancyNotAllowed`. The
//! `token-mover` program sits outside that chain, so the call becomes
//! mover -> Token-2022 -> hook.

#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{InstructionData, ToAccountMetas},
    anchor_lang::solana_program::instruction::{AccountMeta, Instruction},
    anchor_spl::token_2022::Token2022,
    anchor_lang::Id,
    litesvm::LiteSVM,
    solana_keypair::{Address, Keypair},
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, rate_limit_pda,
};

/// Load the mover alongside the hook.
fn add_token_mover(svm: &mut LiteSVM) -> Address {
    let program_id = token_mover::id();
    let bytes = include_bytes!("../../../target/deploy/token_mover.so");
    svm.add_program(program_id, bytes).unwrap();
    program_id
}

/// Build a `transfer_with_hook` instruction. The hook program goes in the
/// remaining accounts *first* — `add_extra_accounts_for_execute_cpi` looks it
/// up there — followed by the two accounts the hook needs resolved.
fn build_mover_ix(
    mover_id: &Address,
    hook_id: &Address,
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> Instruction {
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        hook_id,
    ).0;
    let rate_limit = rate_limit_pda(mint, owner, hook_id);

    let mut accounts = token_mover::accounts::TransferWithHook {
        owner: *owner,
        source_token: *source_ata,
        mint: *mint,
        destination_token: *dest_ata,
        token_program: Token2022::id(),
    }.to_account_metas(None);

    accounts.push(AccountMeta::new_readonly(*hook_id, false));
    accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    accounts.push(AccountMeta::new(rate_limit, false));

    Instruction::new_with_bytes(
        *mover_id,
        &token_mover::instruction::TransferWithHook { amount }.data(),
        accounts,
    )
}

/// Everything a mover test needs: a mint with the hook wired up, funded source
/// and destination token accounts, and both program ids.
fn setup_mover() -> (LiteSVM, Keypair, Address, Address, Pubkey, Pubkey, Pubkey) {
    let (mut svm, payer, hook_id) = setup();
    let mover_id = add_token_mover(&mut svm);

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    (svm, payer, hook_id, mover_id, mint.pubkey(), source_ata, dest_ata)
}

fn send(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair) -> Result<(), String> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{:?}", e))
}

#[test]
fn transfer_through_the_mover_succeeds() {
    let (mut svm, payer, hook_id, mover_id, mint, source_ata, dest_ata) = setup_mover();

    let ix = build_mover_ix(
        &mover_id, &hook_id, &source_ata, &dest_ata, &mint, &payer.pubkey(), 100,
    );

    let res = send(&mut svm, ix, &payer);
    assert!(res.is_ok(), "transfer through the mover should succeed: {:?}", res.err());
}

#[test]
fn the_hook_still_enforces_the_rate_limit_through_the_mover() {
    let (mut svm, payer, hook_id, mover_id, mint, source_ata, dest_ata) = setup_mover();

    // Spend the whole window through the mover.
    let ix = build_mover_ix(
        &mover_id, &hook_id, &source_ata, &dest_ata, &mint, &payer.pubkey(), 1_000_000,
    );
    let res = send(&mut svm, ix, &payer);
    assert!(res.is_ok(), "a transfer at the limit should succeed: {:?}", res.err());

    // One more base unit in the same window must be rejected by the hook, not
    // silently allowed just because a program made the call.
    let ix = build_mover_ix(
        &mover_id, &hook_id, &source_ata, &dest_ata, &mint, &payer.pubkey(), 1,
    );
    let err = send(&mut svm, ix, &payer).expect_err("over the limit must fail");
    assert!(
        err.contains("0x1771") || err.contains("RateLimitExceeded"),
        "expected RateLimitExceeded, got: {err}"
    );
}
