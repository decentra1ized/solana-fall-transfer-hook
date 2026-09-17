#[allow(dead_code)]
mod helpers;

use anchor_lang::{
    InstructionData,
    ToAccountMetas,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        system_instruction,
    },
};
use anchor_spl::token_2022::Token2022;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

use helpers::{
    setup,
    setup_mint_and_extra_metas,
    create_ata,
    mint_tokens,
    send_ix,
};

#[test]
fn test_two_wallets_can_transfer_1000000_in_same_hour() {
    let (mut svm, payer, program_id) = setup();

    // Create the mint and initialize the rate limit for the first wallet.
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // Create a second wallet and give it SOL.
    let second_wallet = Keypair::new();

    let airdrop_ix = system_instruction::transfer(
        &payer.pubkey(),
        &second_wallet.pubkey(),
        1_000_000_000,
    );

    send_ix(
        &mut svm,
        airdrop_ix,
        &payer,
        &[&payer],
    );

    // Create token accounts for both wallets.
    let first_ata = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &mint.pubkey(),
    );

    let second_ata = create_ata(
        &mut svm,
        &payer,
        &second_wallet.pubkey(),
        &mint.pubkey(),
    );

    // Create a destination ATA for each transfer.
    let first_destination_wallet = Keypair::new();
    let second_destination_wallet = Keypair::new();

    let first_destination_ata = create_ata(
        &mut svm,
        &payer,
        &first_destination_wallet.pubkey(),
        &mint.pubkey(),
    );

    let second_destination_ata = create_ata(
        &mut svm,
        &payer,
        &second_destination_wallet.pubkey(),
        &mint.pubkey(),
    );

    // Mint 1,000,000 tokens to each wallet.
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &first_ata,
        1_000_000,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &second_ata,
        1_000_000,
    );

    // First wallet transfers 1,000,000.
    let hook_program = program_id;
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    ).0;
    
    let first_rate_limit = Pubkey::find_program_address(
        &[
            b"rate_limit",
            mint.pubkey().as_ref(),
            payer.pubkey().as_ref(),
            ],
            &program_id,
    ).0;

    let first_ix = Instruction {
        program_id: token_mover::id(),
        accounts: token_mover::accounts::TransferWithHook {
            owner: payer.pubkey(),
            source_token: first_ata,
            mint: mint.pubkey(),
            destination_token: first_destination_ata,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
        data: token_mover::instruction::TransferWithHook {
            amount: 1_000_000,
            decimals: 0,
        }
        .data(),
    };

// Hook accounts must be pushed as remaining accounts.
// Order: hook program, extra-account-meta-list, rate-limit.
    let mut first_ix = first_ix;

    first_ix
    .accounts
    .push(AccountMeta::new_readonly(hook_program, false));

    first_ix
    .accounts
    .push(AccountMeta::new_readonly(
        extra_account_meta_list,
        false,
    ));

    first_ix
    .accounts
    .push(AccountMeta::new(
        first_rate_limit,
        false,
    ));

    send_ix(
        &mut svm,
        first_ix,
        &payer,
        &[&payer],
    );

    // Initialize the rate limit for the SECOND wallet.
    //
    // This is the important part of the challenge:
    // wallet #2 must have its own rate-limit PDA.
    helpers::initialize_rate_limit(
        &mut svm,
        &second_wallet,
        &mint,
        &program_id,
    );

    // Second wallet transfers 1,000,000 during the SAME hour.
    let second_rate_limit = Pubkey::find_program_address(
        &[
            b"rate_limit",
            mint.pubkey().as_ref(),
            second_wallet.pubkey().as_ref(),
            ],
            &program_id,
    ).0;

    let second_ix = Instruction {
        program_id: token_mover::id(),
        accounts: token_mover::accounts::TransferWithHook {
            owner: second_wallet.pubkey(),
            source_token: second_ata,
            mint: mint.pubkey(),
            destination_token: second_destination_ata,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
        data: token_mover::instruction::TransferWithHook {
            amount: 1_000_000,
            decimals: 0,
        }
        .data(),
    };

    let mut second_ix = second_ix;

    second_ix
    .accounts
    .push(AccountMeta::new_readonly(hook_program, false));

    second_ix
    .accounts
    .push(AccountMeta::new_readonly(
        extra_account_meta_list,
        false,
    ));

    second_ix
    .accounts
    .push(AccountMeta::new(
        second_rate_limit,
        false,
    ));

    send_ix(
        &mut svm,
        second_ix,
        &second_wallet,
        &[&second_wallet],
    );