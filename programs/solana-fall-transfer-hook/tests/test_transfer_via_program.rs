#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        AccountDeserialize, Id, InstructionData, ToAccountMetas,
        solana_program::instruction::{AccountMeta, Instruction},
    },
    anchor_spl::token_2022::Token2022,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use solana_fall_transfer_hook::RateLimit;

use helpers::{setup, setup_mint_and_extra_metas, create_ata, mint_tokens};

/// Builds a `token_mover::transfer_with_hook` instruction.
///
/// token-mover's own accounts are the five named ones. The hook's three ride
/// along as *remaining accounts*: the program reads position 0 to learn which
/// hook to resolve, then `add_extra_accounts_for_execute_cpi` reads the meta
/// list to work out the rest.
fn build_mover_ix(
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    hook_program_id: &Pubkey,
    amount: u64,
) -> Instruction {
    let mut ix = Instruction::new_with_bytes(
        token_mover::id(),
        &token_mover::instruction::TransferWithHook { amount }.data(),
        token_mover::accounts::TransferWithHook {
            owner: *owner,
            source_token: *source_ata,
            mint: *mint,
            destination_token: *dest_ata,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    );

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        hook_program_id,
    ).0;

    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.as_ref(), owner.as_ref()],
        hook_program_id,
    ).0;

    // Hook program FIRST: token-mover reads remaining_accounts[0].
    ix.accounts.push(AccountMeta::new_readonly(*hook_program_id, false));
    ix.accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix.accounts.push(AccountMeta::new(rate_limit, false)); // writable: the hook updates it

    ix
}

fn send(svm: &mut litesvm::LiteSVM, ix: Instruction, payer: &Keypair)
    -> Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata>
{
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
}

/// A program can move tokens through Token-2022 and the hook still runs.
#[test]
fn test_transfer_through_program() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, RateLimit::MAX_AMOUNT);

    let ix = build_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100,
    );
    let res = send(&mut svm, ix, &payer);
    assert!(res.is_ok(), "transfer through token-mover failed: {:?}", res.err());

    // Not just "it succeeded" - the hook actually ran and recorded the amount.
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    ).0;
    let account = svm.get_account(&rate_limit).expect("rate limit should exist");
    let state = RateLimit::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(state.amount_transferred, 100, "hook should have recorded the CPI transfer");
}

/// The hook enforces the cap from inside our CPI, not just from a wallet.
#[test]
fn test_transfer_through_program_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, RateLimit::MAX_AMOUNT + 1);

    // Spend the whole window.
    let ix = build_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id,
        RateLimit::MAX_AMOUNT,
    );
    assert!(send(&mut svm, ix, &payer).is_ok(), "first transfer should succeed");

    // One more unit in the same window must be rejected.
    let ix = build_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1,
    );
    let err = send(&mut svm, ix, &payer).unwrap_err();

    let logs = err.meta.logs.join("\n");
    assert!(
        logs.contains("RateLimitExceeded"),
        "expected the hook to reject the transfer, got: {logs}"
    );
}