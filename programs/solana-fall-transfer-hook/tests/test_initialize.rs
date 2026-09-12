#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        InstructionData, ToAccountMetas,
        solana_program::instruction::Instruction,
        system_program::ID as SYSTEM_PROGRAM_ID,
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_interface::{state::Mint as LegacyMint, ID as LEGACY_TOKEN_PROGRAM_ID},
};

use helpers::{setup, initialize_mint};

#[test]
fn test_initialize() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // First create the mint via the dedicated instruction
    initialize_mint(&mut svm, &payer, &mint, &program_id);

    // Then initialize the rate limit account. `payer` is both the payer and
    // the owner this rate limit is scoped to, so it's part of the seeds.
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    ).0;

    let instruction = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg), &[&payer],
    ).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Initialization failed: {:?}", res.err());

    let account = svm
        .get_account(&rate_limit)
        .expect("rate limit account should exist after initialize");
    let stored: solana_fall_transfer_hook::RateLimit =
        anchor_lang::AccountDeserialize::try_deserialize(&mut account.data.as_slice())
            .expect("should deserialize as RateLimit");
    assert_eq!(
        stored.mint,
        mint.pubkey(),
        "rate limit should record the mint it was created for"
    );
}

#[test]
fn initialize_rejects_non_token_2022_mint() {
    let (mut svm, payer, program_id) = setup();

    // A mint owned by the *legacy* Token program, not Token-2022. `initialize`
    // must reject this even though Anchor's `InterfaceAccount<Mint>` happily
    // deserializes it (that type accepts either token program) - only our own
    // owner check in the handler catches this.
    let mint = Keypair::new();
    let state = LegacyMint {
        mint_authority: COption::Some(payer.pubkey()),
        supply: 0,
        decimals: 9,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    let mut data = [0u8; LegacyMint::LEN];
    LegacyMint::pack(state, &mut data).unwrap();
    svm.set_account(
        mint.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: data.to_vec(),
            owner: LEGACY_TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    ).0;

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
    let msg = solana_message::Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let res = svm.send_transaction(tx);
    let failure = res.expect_err("initialize with a legacy-Token mint must be rejected");
    let logs = failure.meta.logs.join("\n");
    assert!(
        logs.contains("InvalidMint"),
        "expected the InvalidMint error in the logs, got:\n{logs}"
    );
}
