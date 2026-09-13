#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::solana_program::instruction::Instruction,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    build_program_transfer_ix, create_ata, mint_tokens, setup, setup_mint_and_extra_metas,
};

fn send(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair) -> Result<(), String> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e.err))
}

#[test]
fn test_transfer_from_program() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    let ix = build_program_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        100,
    );
    let res = send(&mut svm, ix, &payer);
    assert!(
        res.is_ok(),
        "transfer through program failed: {:?}",
        res.err()
    );
}

#[test]
fn test_transfer_from_program_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // exactly the limit: allowed
    let ix1 = build_program_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
    );
    let res1 = send(&mut svm, ix1, &payer);
    assert!(
        res1.is_ok(),
        "transfer at limit should succeed: {:?}",
        res1.err()
    );

    // one more: the hook must reject it, even inside our CPI
    let ix2 = build_program_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1,
    );
    let err = send(&mut svm, ix2, &payer).expect_err("second transfer must fail");
    // 0x1771 == 6001 == RateLimitExceeded
    assert!(
        err.contains("Custom(6001)"),
        "expected RateLimitExceeded, got: {err}"
    );
}
