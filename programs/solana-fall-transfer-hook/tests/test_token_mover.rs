#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{InstructionData, ToAccountMetas},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    spl_transfer_hook_interface::get_extra_account_metas_address,
};

use helpers::{
    build_transfer_with_hook_ix, create_ata, mint_tokens, setup, setup_mint_and_extra_metas,
};

fn send_through_program(
    svm: &mut litesvm::LiteSVM,
    payer: &Keypair,
    hook_id: &solana_keypair::Address,
    amount: u64,
    source: &Pubkey,
    dest: &Pubkey,
    mint: &Pubkey,
) -> Result<(), litesvm::types::FailedTransactionMetadata> {
    let mover_id = token_mover::id();
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.as_ref(), payer.pubkey().as_ref()],
        hook_id,
    ).0;
    let extra = get_extra_account_metas_address(mint, hook_id);
    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: mover_id,
        accounts: token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: *source,
            mint: *mint,
            destination_token: *dest,
            token_program: anchor_spl::token_2022::ID,
        }
        .to_account_metas(None)
        .into_iter()
        .chain([
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*hook_id, false),
            anchor_lang::solana_program::instruction::AccountMeta::new(extra, false),
            anchor_lang::solana_program::instruction::AccountMeta::new(rate_limit, false),
        ])
        .collect(),
        data: token_mover::instruction::TransferWithHook { amount }.data(),
    };
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).map(|_| ())
}

#[test]
fn program_transfer_respects_the_hook() {
    let (mut svm, payer, hook_id) = setup();
    let bytes = include_bytes!("../../../target/deploy/token_mover.so");
    svm.add_program(token_mover::id(), bytes).unwrap();

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_id);
    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source, 2_000_000);

    send_through_program(&mut svm, &payer, &hook_id, 1_000_000, &source, &dest, &mint.pubkey())
        .expect("program transfer at the limit should succeed");
    let err = send_through_program(&mut svm, &payer, &hook_id, 1, &source, &dest, &mint.pubkey());
    assert!(err.is_err(), "hook should block the next token from the program");
}
