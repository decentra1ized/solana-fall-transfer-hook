#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::solana_program::instruction::Instruction,
    litesvm::LiteSVM,
    litesvm::types::TransactionResult,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_mover_transfer_ix};

fn send(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair) -> TransactionResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
}

#[test]
fn test_transfer_through_mover() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100,
    );
    let res = send(&mut svm, ix, &payer);
    assert!(res.is_ok(), "Transfer through token-mover failed: {:?}", res.err());
}

#[test]
fn test_mover_transfer_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // Exactly at the limit - should succeed.
    let ix1 = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000,
    );
    let res = send(&mut svm, ix1, &payer);
    assert!(res.is_ok(), "Transfer at limit should succeed: {:?}", res.err());

    // One more token - the hook, two CPIs deep, must reject it.
    let ix2 = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1,
    );
    let err = send(&mut svm, ix2, &payer).expect_err("Transfer exceeding rate limit should fail");
    // 0x1771 = 6001 = RateLimitExceeded
    assert_eq!(
        format!("{:?}", err.err),
        format!("InstructionError(0, Custom({}))", 0x1771),
        "logs: {:#?}",
        err.meta.logs,
    );
}
