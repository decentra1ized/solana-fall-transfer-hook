use {
    anchor_lang::{
        Id, InstructionData, ToAccountMetas,
        solana_program::instruction::{AccountMeta, Instruction},
        system_program::ID as SYSTEM_PROGRAM_ID,
    },
    anchor_spl::{
        associated_token::{
            spl_associated_token_account,
            get_associated_token_address_with_program_id,
        },
        token_2022::{Token2022, spl_token_2022},
    },
    litesvm::LiteSVM,
    solana_keypair::{Address, Keypair},
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

// Loads BOTH programs into the same LiteSVM instance: the mover (under test)
// and the transfer hook it CPIs into. A program-initiated transfer only
// makes sense with both present - `token_mover::transfer` invokes
// `solana_fall_transfer_hook::transfer_hook` indirectly via Token-2022.
pub fn setup() -> (LiteSVM, Keypair, Address, Address) {
    let mover_program_id = token_mover::id();
    let hook_program_id = solana_fall_transfer_hook::id();

    let mut svm = LiteSVM::new();
    let mover_bytes = include_bytes!("../../../../target/deploy/token_mover.so");
    svm.add_program(mover_program_id, mover_bytes).unwrap();
    let hook_bytes = include_bytes!("../../../../target/deploy/solana_fall_transfer_hook.so");
    svm.add_program(hook_program_id, hook_bytes).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    (svm, payer, mover_program_id, hook_program_id)
}

pub fn send_ix(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair, signers: &[&Keypair]) {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx).unwrap();
}

pub fn initialize_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, hook_program_id: &Address) {
    let ix = Instruction::new_with_bytes(
        *hook_program_id,
        &solana_fall_transfer_hook::instruction::InitializeMint {}.data(),
        solana_fall_transfer_hook::accounts::InitializeMint {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            system_program: SYSTEM_PROGRAM_ID,
            token_program: Token2022::id(),
        }.to_account_metas(None),
    );
    send_ix(svm, ix, payer, &[payer, mint]);
}

// `payer` doubles as the owner this rate limit is scoped to.
pub fn initialize_rate_limit(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, hook_program_id: &Address) {
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        hook_program_id,
    ).0;

    let ix = Instruction::new_with_bytes(
        *hook_program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );
    send_ix(svm, ix, payer, &[payer]);
}

pub fn initialize_extra_account_metas(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, hook_program_id: &Address) {
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        hook_program_id,
    ).0;

    let ix = Instruction::new_with_bytes(
        *hook_program_id,
        &solana_fall_transfer_hook::instruction::InitializeExtraAccountMetaList {}.data(),
        solana_fall_transfer_hook::accounts::InitializeExtraAccountMetaList {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            extra_account_meta_list,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );
    send_ix(svm, ix, payer, &[payer]);
}

pub fn setup_mint_and_extra_metas(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, hook_program_id: &Address) {
    initialize_mint(svm, payer, mint, hook_program_id);
    initialize_rate_limit(svm, payer, mint, hook_program_id);
    initialize_extra_account_metas(svm, payer, mint, hook_program_id);
}

pub fn create_ata(svm: &mut LiteSVM, payer: &Keypair, wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    let ata = get_associated_token_address_with_program_id(wallet, mint, &Token2022::id());
    let ix = spl_associated_token_account::instruction::create_associated_token_account(
        &payer.pubkey(),
        wallet,
        mint,
        &Token2022::id(),
    );
    send_ix(svm, ix, payer, &[payer]);
    ata
}

pub fn mint_tokens(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, dest: &Pubkey, amount: u64) {
    let ix = spl_token_2022::instruction::mint_to(
        &Token2022::id(),
        mint,
        dest,
        &payer.pubkey(),
        &[],
        amount,
    ).unwrap();
    send_ix(svm, ix, payer, &[payer]);
}

// Builds the token-mover's own `transfer` instruction, with the hook program
// and its resolved extra accounts appended as remaining accounts - exactly
// mirroring what `add_extra_accounts_for_execute_cpi` expects to find on the
// stack inside the mover program (see programs/token-mover/src/lib.rs):
// [hook_program_id, extra-account-metas PDA, rate_limit PDA].
#[allow(clippy::too_many_arguments)]
pub fn build_mover_transfer_ix(
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    authority: &Pubkey,
    mover_program_id: &Address,
    hook_program_id: &Address,
    amount: u64,
) -> Instruction {
    let mut ix = Instruction::new_with_bytes(
        *mover_program_id,
        &token_mover::instruction::Transfer { amount }.data(),
        token_mover::accounts::Transfer {
            authority: *authority,
            mint: *mint,
            source: *source_ata,
            destination: *dest_ata,
            token_program: Token2022::id(),
        }.to_account_metas(None),
    );

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        hook_program_id,
    ).0;
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.as_ref(), authority.as_ref()],
        hook_program_id,
    ).0;

    ix.accounts.push(AccountMeta::new_readonly(*hook_program_id, false));
    ix.accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix.accounts.push(AccountMeta::new(rate_limit, false));

    ix
}
