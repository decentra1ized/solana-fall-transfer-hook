#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    solana_pubkey::Pubkey,
};

use anchor_lang::{
    InstructionData, ToAccountMetas,
    solana_program::instruction::{AccountMeta, Instruction}
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, 
    build_transfer_with_hook_ix, initialize_rate_limit, send_ix		
};

#[test]
fn test_transfer_hook_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    // Mint more than the rate limit so we have enough tokens
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // First transfer: exactly at the limit - should succeed
    let ix1 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer at limit should succeed: {:?}", res.err());

    // Second transfer: 1 token more - should fail with RateLimitExceeded
    let ix2 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");
}


#[test]
fn test_multiple_users_separate_limits() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // 1. Setup Mint and Wallet 1 (payer)
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);
    let source_ata_1 = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata_1, 2_000_000);

    // 2. Setup Wallet 2
    let wallet_2 = Keypair::new();
    svm.airdrop(&wallet_2.pubkey(), 1_000_000_000).unwrap();
    
    // Wallet 2 must initialize its own independent rate limit folder
    initialize_rate_limit(&mut svm, &wallet_2, &mint, &program_id);
    
    let source_ata_2 = create_ata(&mut svm, &payer, &wallet_2.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata_2, 2_000_000);

    // 3. Setup a shared destination
    let dest_owner = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &dest_owner.pubkey(), &mint.pubkey());

    // 4. Both send 1,000,000 (hitting their individual max limits)
    let ix1 = build_transfer_with_hook_ix(
        &source_ata_1, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9
    );
    let ix2 = build_transfer_with_hook_ix(
        &source_ata_2, &dest_ata, &mint.pubkey(), &wallet_2.pubkey(), &program_id, 1_000_000, 9
    );

    // Both should succeed without panicking. Before Challenge 3, the second would fail.
    send_ix(&mut svm, ix1, &payer, &[&payer]);
    send_ix(&mut svm, ix2, &wallet_2, &[&wallet_2]);
}


// Helper function to build the CPI instruction specifically for the Mover program
fn build_cpi_transfer(
    source: &Pubkey, dest: &Pubkey, mint: &Pubkey, owner: &Pubkey,
    mover_id: &Pubkey, hook_id: &Pubkey, amount: u64
) -> Instruction {
    let mut ix = Instruction::new_with_bytes(
        *mover_id,
        &token_mover::instruction::TransferWithHook { amount }.data(),
        token_mover::accounts::TransferWithHook {
            owner: *owner,
            source_token: *source,
            mint: *mint,
            destination_token: *dest,
            token_program: anchor_spl::token_2022::ID,
        }.to_account_metas(None),
    );

    // FIX: Using the correct hyphenated standard for the seed
    let extra_metas = Pubkey::find_program_address(&[b"extra-account-metas", mint.as_ref()], hook_id).0;
    let rate_limit = Pubkey::find_program_address(&[b"rate_limit", mint.as_ref(), owner.as_ref()], hook_id).0;

    // Push the remaining accounts in the exact order the hook expects
    ix.accounts.push(AccountMeta::new_readonly(*hook_id, false));
    ix.accounts.push(AccountMeta::new_readonly(extra_metas, false));
    ix.accounts.push(AccountMeta::new(rate_limit, false));

    ix
}

#[test]
fn test_cpi_transfer_success() {
    let (mut svm, payer, hook_id) = setup();
    let mover_id = token_mover::id();
    let mint = Keypair::new();
    
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_id);

    let recipient = Keypair::new();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source, 1_000_000);

    let ix = build_cpi_transfer(&source, &dest, &mint.pubkey(), &payer.pubkey(), &mover_id, &hook_id, 100);
    
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    
    assert!(res.is_ok(), "CPI transfer failed: {:?}", res.err());
}

#[test]
fn test_cpi_transfer_rate_limit_exceeded() {
    let (mut svm, payer, hook_id) = setup();
    let mover_id = token_mover::id();
    let mint = Keypair::new();
    
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_id);

    let recipient = Keypair::new();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source, 2_000_000);

    // First transfer maxes out the 1,000,000 limit
    let ix1 = build_cpi_transfer(&source, &dest, &mint.pubkey(), &payer.pubkey(), &mover_id, &hook_id, 1_000_000);
    let blockhash1 = svm.latest_blockhash();
    let msg1 = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash1);
    let tx1 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg1), &[&payer]).unwrap();
    svm.send_transaction(tx1).unwrap();

    // Second transfer pushes it over the edge and should be blocked
    let ix2 = build_cpi_transfer(&source, &dest, &mint.pubkey(), &payer.pubkey(), &mover_id, &hook_id, 1);
    let blockhash2 = svm.latest_blockhash();
    let msg2 = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash2);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[&payer]).unwrap();
    let res = svm.send_transaction(tx2);
    
    assert!(res.is_err(), "CPI transfer should have failed due to rate limit");
}
