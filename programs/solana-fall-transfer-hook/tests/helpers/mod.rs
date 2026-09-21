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

pub fn setup() -> (LiteSVM, Keypair, Address) {
    let program_id = solana_fall_transfer_hook::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../../target/deploy/solana_fall_transfer_hook.so");
    svm.add_program(program_id, bytes).unwrap();

    // The token-mover program is what the Challenge 4 tests call into.
    let mover_bytes = include_bytes!("../../../../target/deploy/token_mover.so");
    svm.add_program(token_mover::id(), mover_bytes).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    (svm, payer, program_id)
}

pub fn send_ix(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair, signers: &[&Keypair]) {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx).unwrap();
}

/// Sends an instruction and returns the result instead of panicking on failure.
pub fn try_send_ix(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
) -> std::result::Result<(), String> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{:?}", e))
}

/// The rate limit PDA for one (mint, owner) pair.
///
/// These seeds must match `extra_account_metas()` in
/// `init_extra_account_meta.rs`, the `Initialize` context in `initialize.rs`
/// and the `TransferHook` context in `transfer_hook.rs`.
pub fn rate_limit_pda(mint: &Pubkey, owner: &Pubkey, program_id: &Address) -> Pubkey {
    Pubkey::find_program_address(
        &[b"rate_limit", mint.as_ref(), owner.as_ref()],
        program_id,
    ).0
}

pub fn extra_account_meta_list_pda(mint: &Pubkey, program_id: &Address) -> Pubkey {
    Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        program_id,
    ).0
}

pub fn initialize_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, program_id: &Address) {
    let ix = Instruction::new_with_bytes(
        *program_id,
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

/// Creates the rate limit account for one owner and one mint.
///
/// The PDA seeds include the payer, so the payer is the owner that this rate
/// limit applies to. Every owner must call this once per mint before their
/// first transfer.
pub fn initialize_rate_limit(svm: &mut LiteSVM, owner: &Keypair, mint: &Keypair, program_id: &Address) {
    let rate_limit = rate_limit_pda(&mint.pubkey(), &owner.pubkey(), program_id);

    let ix = Instruction::new_with_bytes(
        *program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: owner.pubkey(),
            mint: mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );
    send_ix(svm, ix, owner, &[owner]);
}

pub fn initialize_extra_account_metas(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, program_id: &Address) {
    let extra_account_meta_list = extra_account_meta_list_pda(&mint.pubkey(), program_id);

    let ix = Instruction::new_with_bytes(
        *program_id,
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

pub fn setup_mint_and_extra_metas(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, program_id: &Address) {
    initialize_mint(svm, payer, mint, program_id);
    initialize_rate_limit(svm, payer, mint, program_id);
    initialize_extra_account_metas(svm, payer, mint, program_id);
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

/// The three accounts the hook needs, in the order the resolver expects them:
/// the hook program, its ExtraAccountMetaList, then the resolved extra accounts.
fn hook_account_metas(mint: &Pubkey, owner: &Pubkey, program_id: &Address) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(*program_id, false),
        AccountMeta::new_readonly(extra_account_meta_list_pda(mint, program_id), false),
        AccountMeta::new(rate_limit_pda(mint, owner, program_id), false),
    ]
}

/// A direct Token-2022 `transfer_checked` with the hook accounts appended.
pub fn build_transfer_with_hook_ix(
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    program_id: &Address,
    amount: u64,
    decimals: u8,
) -> Instruction {
    let mut ix = spl_token_2022::instruction::transfer_checked(
        &Token2022::id(),
        source_ata,
        mint,
        dest_ata,
        owner,
        &[],
        amount,
        decimals,
    ).unwrap();

    ix.accounts.extend(hook_account_metas(mint, owner, program_id));

    ix
}

/// The same transfer, but routed through the token-mover program.
///
/// The hook accounts ride along as `remaining_accounts`; the mover resolves
/// them on-chain before it invokes Token-2022.
pub fn build_mover_transfer_ix(
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    program_id: &Address,
    amount: u64,
) -> Instruction {
    let mut accounts = token_mover::accounts::TransferWithHook {
        owner: *owner,
        source_token: *source_ata,
        mint: *mint,
        destination_token: *dest_ata,
        token_program: Token2022::id(),
    }.to_account_metas(None);

    accounts.extend(hook_account_metas(mint, owner, program_id));

    Instruction::new_with_bytes(
        token_mover::id(),
        &token_mover::instruction::TransferWithHook { amount }.data(),
        accounts,
    )
}
