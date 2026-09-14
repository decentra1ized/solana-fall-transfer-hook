//! Challenge 4: transfers driven from a second program.
//!
//! token-mover CPIs into Token-2022, which in turn calls back into the hook
//! program. Doing the same thing from inside the hook program would be an
//! indirect re-entry (hook -> Token-2022 -> hook) and the runtime rejects it,
//! which is the whole reason this second program exists.

#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        Id, InstructionData, ToAccountMetas,
        solana_program::instruction::{AccountMeta, Instruction},
    },
    anchor_spl::token_2022::Token2022,
    litesvm::LiteSVM,
    solana_keypair::{Address, Keypair},
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{create_ata, mint_tokens, setup, setup_mint_and_extra_metas};

// The hook program is already loaded by `setup()`; the CPI caller has to be
// added on top of it.
fn add_token_mover(svm: &mut LiteSVM) {
    let bytes = include_bytes!("../../../target/deploy/token_mover.so");
    svm.add_program(token_mover::id(), bytes).unwrap();
}

// Same shape as `helpers::build_transfer_with_hook_ix`, except the instruction
// targets token-mover instead of Token-2022. The three trailing accounts land
// in `ctx.remaining_accounts`, which is what
// `add_extra_accounts_for_execute_cpi` looks through.
fn build_token_mover_transfer_ix(
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    hook_program_id: &Address,
    amount: u64,
) -> Instruction {
    let mut ix = Instruction::new_with_bytes(
        token_mover::id(),
        &token_mover::instruction::TransferWithHook { amount }.data(),
        token_mover::accounts::TransferWithHook {
            owner: *owner,
            source_token: *source_ata,
            mint: *mint,
            destination_token: *dest_ata,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    );

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        hook_program_id,
    ).0;

    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.as_ref(), owner.as_ref()],
        hook_program_id,
    ).0;

    // The handler reads `remaining_accounts[0]` as the hook program, so this
    // one has to stay first.
    ix.accounts.push(AccountMeta::new_readonly(*hook_program_id, false));
    ix.accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix.accounts.push(AccountMeta::new(rate_limit, false));

    ix
}

fn send(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair) -> Result<(), String> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e.err))
}

#[test]
fn test_token_mover_transfer() {
    let (mut svm, payer, program_id) = setup();
    add_token_mover(&mut svm);

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_token_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100,
    );

    let res = send(&mut svm, ix, &payer);
    assert!(res.is_ok(), "Transfer through token-mover failed: {:?}", res.err());
}

#[test]
fn test_token_mover_transfer_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    add_token_mover(&mut svm);

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // Exactly at the limit - the hook lets this through.
    let ix1 = build_token_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000,
    );
    let res = send(&mut svm, ix1, &payer);
    assert!(res.is_ok(), "Transfer at the limit should succeed: {:?}", res.err());

    // One base unit past it - the hook must abort the whole transfer.
    let ix2 = build_token_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1,
    );
    let err = send(&mut svm, ix2, &payer)
        .expect_err("Transfer exceeding the rate limit should fail");

    // 0x1771 = 6001 = ErrorCode::RateLimitExceeded
    assert!(
        err.contains("Custom(6001)"),
        "Expected RateLimitExceeded (0x1771), got: {err}"
    );
}
