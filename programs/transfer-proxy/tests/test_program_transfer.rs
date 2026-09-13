//! Challenge 4 — on-chain transfer_checked via a proxy program
//!
//! The re-entrancy problem:
//!   If the hook program called `transfer_checked` itself, Token-2022 would
//!   re-enter the same program (same program ID already on the call stack →
//!   runtime error). The fix is a *second* program (this proxy) with a
//!   different program ID. The hook is called by Token-2022 from inside the
//!   proxy, so no program appears twice in the call stack.
//!
//! Call chain:
//!   test → proxy::proxy_transfer
//!              └─→ Token-2022 transfer_checked
//!                       └─→ hook::transfer_hook   ✓ no re-entrancy

use {
    anchor_lang::{Id, InstructionData, ToAccountMetas, solana_program::instruction::{AccountMeta, Instruction}},
    anchor_spl::token_2022::Token2022,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    transfer_proxy::accounts::ProxyTransfer,
    transfer_proxy::instruction::ProxyTransfer as ProxyTransferIx,
};

// ── shared helpers from the hook crate ───────────────────────────────────────

fn airdrop_keypair(svm: &mut LiteSVM) -> Keypair {
    let kp = Keypair::new();
    svm.airdrop(&kp.pubkey(), 1_000_000_000).unwrap();
    kp
}

fn setup_hook_program(svm: &mut LiteSVM) -> Pubkey {
    let program_id = solana_fall_transfer_hook::id();
    let bytes = include_bytes!("../../../target/deploy/solana_fall_transfer_hook.so");
    svm.add_program(program_id, bytes).unwrap();
    program_id
}

fn setup_proxy_program(svm: &mut LiteSVM) -> Pubkey {
    let proxy_id = transfer_proxy::id();
    let bytes = include_bytes!("../../../target/deploy/transfer_proxy.so");
    svm.add_program(proxy_id, bytes).unwrap();
    proxy_id
}

// ── thin inline helper stubs (avoid importing the hook's test-only helpers) ──

use anchor_lang::system_program::ID as SYSTEM_PROGRAM_ID;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::associated_token::spl_associated_token_account;

fn send(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair, signers: &[&Keypair]) {
    let bh = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &bh);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx).unwrap();
}

fn init_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, hook_id: &Pubkey) {
    let ix = Instruction::new_with_bytes(
        *hook_id,
        &solana_fall_transfer_hook::instruction::InitializeMint {}.data(),
        solana_fall_transfer_hook::accounts::InitializeMint {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            system_program: SYSTEM_PROGRAM_ID,
            token_program: Token2022::id(),
        }.to_account_metas(None),
    );
    send(svm, ix, payer, &[payer, mint]);
}

fn init_rate_limit(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, hook_id: &Pubkey) {
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        hook_id,
    ).0;
    let ix = Instruction::new_with_bytes(
        *hook_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );
    send(svm, ix, payer, &[payer]);
}

fn init_extra_metas(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, hook_id: &Pubkey) {
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        hook_id,
    ).0;
    let ix = Instruction::new_with_bytes(
        *hook_id,
        &solana_fall_transfer_hook::instruction::InitializeExtraAccountMetaList {}.data(),
        solana_fall_transfer_hook::accounts::InitializeExtraAccountMetaList {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            extra_account_meta_list,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );
    send(svm, ix, payer, &[payer]);
}

fn create_ata(svm: &mut LiteSVM, payer: &Keypair, wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    let ata = get_associated_token_address_with_program_id(wallet, mint, &Token2022::id());
    let ix = spl_associated_token_account::instruction::create_associated_token_account(
        &payer.pubkey(), wallet, mint, &Token2022::id(),
    );
    send(svm, ix, payer, &[payer]);
    ata
}

fn mint_tokens(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, dest: &Pubkey, amount: u64) {
    let ix = spl_token_2022::instruction::mint_to(
        &Token2022::id(), mint, dest, &payer.pubkey(), &[], amount,
    ).unwrap();
    send(svm, ix, payer, &[payer]);
}

// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_program_transfer_via_proxy() {
    let mut svm = LiteSVM::new();
    let payer     = airdrop_keypair(&mut svm);
    let hook_id   = setup_hook_program(&mut svm);
    let proxy_id  = setup_proxy_program(&mut svm);
    let mint      = Keypair::new();

    init_mint(&mut svm, &payer, &mint, &hook_id);
    init_rate_limit(&mut svm, &payer, &mint, &hook_id);
    init_extra_metas(&mut svm, &payer, &mint, &hook_id);

    let recipient = airdrop_keypair(&mut svm);
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(),      &mint.pubkey());
    let dest_ata   = create_ata(&mut svm, &payer, &recipient.pubkey(),  &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &hook_id,
    ).0;
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &hook_id,
    ).0;

    // Build the proxy_transfer instruction. The hook's extra-accounts are
    // appended as remaining_accounts so Token-2022 forwards them to the hook.
    let mut ix = Instruction::new_with_bytes(
        proxy_id,
        &ProxyTransferIx { amount: 100, decimals: 9 }.data(),
        ProxyTransfer {
            source: source_ata,
            mint: mint.pubkey(),
            destination: dest_ata,
            authority: payer.pubkey(),
            token_program: Token2022::id(),
        }.to_account_metas(None),
    );

    // Remaining accounts: hook program id → extra_account_meta_list → rate_limit
    ix.accounts.push(AccountMeta::new_readonly(hook_id, false));
    ix.accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix.accounts.push(AccountMeta::new(rate_limit, false));

    let bh  = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &bh);
    let tx  = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Proxy transfer failed: {:?}", res.err());
}
