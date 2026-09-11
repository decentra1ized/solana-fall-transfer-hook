#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

use helpers::{initialize_mint, rate_limit_pda, setup};

#[test]
fn test_initialize() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // First create the mint via the dedicated instruction
    initialize_mint(&mut svm, &payer, &mint, &program_id);

    // Then initialize the rate limit account
    let rate_limit = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);

    let instruction = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Initialization failed: {:?}", res.err());

    let account = svm.get_account(&rate_limit).unwrap();
    let mut data = account.data.as_slice();
    let rate_limit = solana_fall_transfer_hook::RateLimit::try_deserialize(&mut data).unwrap();
    assert_eq!(rate_limit.authority, payer.pubkey());
    assert_eq!(rate_limit.mint, mint.pubkey());
}
