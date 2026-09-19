#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        AccountDeserialize, Id, InstructionData, ToAccountMetas,
        solana_program::instruction::Instruction,
    },
    anchor_spl::{
        token_2022::Token2022,
        token_interface::TokenAccount,
    },
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{create_ata, mint_tokens, setup, setup_mint_and_extra_metas};

#[test]
fn test_transfer_through_helper() {
    let (mut svm, payer, hook_program_id) = setup();
    let helper_program_id = transfer_helper::id();
    let helper_program = std::fs::read(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy/transfer_helper.so"),
    )
    .unwrap();
    svm.add_program(helper_program_id, &helper_program).unwrap();

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let destination_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000);

    let rate_limit = solana_pubkey::Pubkey::find_program_address(
        &[
            b"rate_limit",
            mint.pubkey().as_ref(),
            payer.pubkey().as_ref(),
        ],
        &hook_program_id,
    )
    .0;
    let extra_account_meta_list = solana_pubkey::Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &hook_program_id,
    )
    .0;

    let transfer_ix = Instruction::new_with_bytes(
        helper_program_id,
        &transfer_helper::instruction::Transfer { amount: 100 }.data(),
        transfer_helper::accounts::Transfer {
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: destination_ata,
            owner: payer.pubkey(),
            rate_limit,
            extra_account_meta_list,
            hook_program: hook_program_id,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(
        &[transfer_ix],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let transaction = VersionedTransaction::try_new(
        VersionedMessage::Legacy(message),
        &[&payer],
    )
    .unwrap();

    let result = svm.send_transaction(transaction);
    assert!(result.is_ok(), "Transfer through helper failed: {:?}", result.err());

    let destination_account = svm.get_account(&destination_ata).unwrap();
    let destination_state = TokenAccount::try_deserialize(
        &mut destination_account.data.as_slice(),
    )
    .unwrap();
    assert_eq!(destination_state.amount, 100);
}

#[test]
fn test_transfer_through_helper_enforces_rate_limit() {
    let (mut svm, payer, hook_program_id) = setup();
    let helper_program_id = transfer_helper::id();
    let helper_program = std::fs::read(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy/transfer_helper.so"),
    )
    .unwrap();
    svm.add_program(helper_program_id, &helper_program).unwrap();

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);
    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let destination_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_001);

    let rate_limit = solana_pubkey::Pubkey::find_program_address(
        &[
            b"rate_limit",
            mint.pubkey().as_ref(),
            payer.pubkey().as_ref(),
        ],
        &hook_program_id,
    )
    .0;
    let extra_account_meta_list = solana_pubkey::Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &hook_program_id,
    )
    .0;

    for (amount, should_succeed) in [(1_000_000, true), (1, false)] {
        let transfer_ix = Instruction::new_with_bytes(
            helper_program_id,
            &transfer_helper::instruction::Transfer { amount }.data(),
            transfer_helper::accounts::Transfer {
                source_token: source_ata,
                mint: mint.pubkey(),
                destination_token: destination_ata,
                owner: payer.pubkey(),
                rate_limit,
                extra_account_meta_list,
                hook_program: hook_program_id,
                token_program: Token2022::id(),
            }
            .to_account_metas(None),
        );
        let blockhash = svm.latest_blockhash();
        let message = Message::new_with_blockhash(
            &[transfer_ix],
            Some(&payer.pubkey()),
            &blockhash,
        );
        let transaction = VersionedTransaction::try_new(
            VersionedMessage::Legacy(message),
            &[&payer],
        )
        .unwrap();
        let result = svm.send_transaction(transaction);
        if should_succeed {
            assert!(result.is_ok(), "Helper transfer failed: {:?}", result.err());
        } else {
            let error = result.expect_err("Transfer above the limit should fail");
            assert!(
                error
                    .meta
                    .logs
                    .iter()
                    .any(|log| log.contains("RateLimitExceeded")),
                "Expected RateLimitExceeded error, got logs: {:?}",
                error.meta.logs,
            );
        }
    }
}
