#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup,
    setup_mint_and_extra_metas,
    create_ata,
    mint_tokens,
    build_token_mover_transfer_ix,
};

#[test]
fn test_token_mover_transfer_with_hook() {
    let (mut svm, payer, hook_program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(
        &mut svm,
        &payer,
        &mint,
        &hook_program_id,
    );

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000)
        .unwrap();

    let source_ata = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &mint.pubkey(),
    );

    let destination_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata,
        1_000_000,
    );

    let transfer_ix = build_token_mover_transfer_ix(
        &source_ata,
        &mint.pubkey(),
        &destination_ata,
        &payer.pubkey(),
        &hook_program_id,
        100,
        9,
    );

    let blockhash = svm.latest_blockhash();

    let msg = Message::new_with_blockhash(
        &[transfer_ix],
        Some(&payer.pubkey()),
        &blockhash,
    );

    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let result = svm.send_transaction(tx);

    assert!(
        result.is_ok(),
        "Token-mover transfer with hook failed: {:?}",
        result.err()
    );
}

