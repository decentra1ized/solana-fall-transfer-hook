#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{solana_program::instruction::Instruction, Id, InstructionData, ToAccountMetas},
    anchor_spl::token_2022::Token2022,
    litesvm::{
        types::{FailedTransactionMetadata, TransactionMetadata},
        LiteSVM,
    },
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    build_transfer_with_hook_ix, create_ata, mint_tokens, setup, setup_mint_and_extra_metas,
};

/// Loads token-mover at runtime. `anchor build` compiles the hook (and its
/// tests, for the IDL) before token-mover, so `include_bytes!` would fail on a
/// clean checkout.
fn add_token_mover(svm: &mut LiteSVM) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy/token_mover.so");
    let bytes = std::fs::read(path).expect("token_mover.so missing: run `anchor build` first");
    svm.add_program(token_mover::id(), &bytes).unwrap();
}

/// A token-mover instruction that moves `amount` from `source` to `destination`.
fn build_mover_ix(
    source: &Pubkey,
    destination: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    hook_program: &Pubkey,
    amount: u64,
) -> Instruction {
    let mut ix = Instruction::new_with_bytes(
        token_mover::id(),
        &token_mover::instruction::TransferWithHook { amount }.data(),
        token_mover::accounts::TransferWithHook {
            owner: *owner,
            source_token: *source,
            mint: *mint,
            destination_token: *destination,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    );

    // Remaining accounts: the hook program, its extra-account-metas list and the
    // owner's rate limit. The direct-transfer helper appends exactly these three
    // after the four transfer_checked accounts, so reuse them.
    let hook_ix = build_transfer_with_hook_ix(source, destination, mint, owner, hook_program, amount, 9);
    ix.accounts.extend_from_slice(&hook_ix.accounts[4..]);
    ix
}

fn send(
    svm: &mut LiteSVM,
    owner: &Keypair,
    ix: Instruction,
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&owner.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[owner]).unwrap();
    svm.send_transaction(tx)
}

#[test]
fn test_transfer_through_program() {
    let (mut svm, payer, program_id) = setup();
    add_token_mover(&mut svm);
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_mover_ix(&source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100);
    let res = send(&mut svm, &payer, ix);
    assert!(res.is_ok(), "transfer through token-mover failed: {:?}", res.err());
}

#[test]
fn test_rate_limit_holds_through_program() {
    let (mut svm, payer, program_id) = setup();
    add_token_mover(&mut svm);
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // Using the whole limit through the program is fine...
    let ix = build_mover_ix(&source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000);
    let res = send(&mut svm, &payer, ix);
    assert!(res.is_ok(), "transfer at the limit should succeed: {:?}", res.err());

    // ...but one more token in the same window must still be stopped by the hook.
    let ix = build_mover_ix(&source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1);
    let err = match send(&mut svm, &payer, ix) {
        Ok(_) => panic!("the hook should reject a transfer over the limit, even through a program"),
        Err(err) => err,
    };
    let logs = err.meta.logs.join("\n");
    assert!(logs.contains("0x1771"), "expected RateLimitExceeded (0x1771), got: {logs}");
}
