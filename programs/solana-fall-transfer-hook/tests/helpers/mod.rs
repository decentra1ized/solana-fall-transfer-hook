use {
    anchor_lang::{
        AccountDeserialize, Id, InstructionData, ToAccountMetas,
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
    solana_account::Account,
    solana_keypair::{Address, Keypair},
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    solana_fall_transfer_hook::RateLimit,
};

pub fn setup() -> (LiteSVM, Keypair, Address) {
    let program_id = solana_fall_transfer_hook::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../../target/deploy/solana_fall_transfer_hook.so");
    svm.add_program(program_id, bytes).unwrap();

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

/// Derives the rate limit PDA for a given mint and owner.
///
/// These seeds are declared in four places that must agree: `extra_account_metas()`,
/// the `Initialize` context, the `TransferHook` context, and here. Keeping the test
/// side in one function means there is only one of those four to fix.
pub fn rate_limit_pda(mint: &Pubkey, owner: &Pubkey, program_id: &Address) -> Pubkey {
    Pubkey::find_program_address(
        &[b"rate_limit", mint.as_ref(), owner.as_ref()],
        program_id,
    ).0
}

/// Opens a rate limit bucket for `payer` on `mint`.
///
/// The payer is the owner the bucket is keyed to, so this creates the bucket that
/// `payer`'s own transfers of `mint` will be charged against.
pub fn initialize_rate_limit(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, program_id: &Address) {
    let rate_limit = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), program_id);

    let ix = Instruction::new_with_bytes(
        *program_id,
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

pub fn initialize_extra_account_metas(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, program_id: &Address) {
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        program_id,
    ).0;

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

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        program_id,
    ).0;

    // Same (mint, owner) pair the runtime will resolve from the extra account metas.
    let rate_limit = rate_limit_pda(mint, owner, program_id);

    ix.accounts.push(AccountMeta::new_readonly(*program_id, false));
    ix.accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    ix.accounts.push(AccountMeta::new(rate_limit, false));

    ix
}

/// Builds a call to this program's own `transfer` instruction.
///
/// The hook's accounts are passed as `remaining_accounts` — the program itself, the
/// ExtraAccountMetaList and the rate limit PDA — which is what
/// `add_extra_accounts_for_execute_cpi` looks through to resolve the hook CPI.
pub fn build_program_transfer_ix(
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    program_id: &Address,
    amount: u64,
) -> Instruction {
    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        program_id,
    ).0;
    let rate_limit = rate_limit_pda(mint, owner, program_id);

    let mut accounts = solana_fall_transfer_hook::accounts::Transfer {
        owner: *owner,
        source_token: *source_ata,
        mint: *mint,
        destination_token: *dest_ata,
        token_program: Token2022::id(),
    }.to_account_metas(None);

    accounts.push(AccountMeta::new_readonly(*program_id, false));
    accounts.push(AccountMeta::new_readonly(extra_account_meta_list, false));
    accounts.push(AccountMeta::new(rate_limit, false));

    Instruction::new_with_bytes(
        *program_id,
        &solana_fall_transfer_hook::instruction::Transfer { amount }.data(),
        accounts,
    )
}

/// Reads a rate limit account's current running total.
pub fn rate_limit_amount_transferred(svm: &LiteSVM, rate_limit: &Pubkey) -> u64 {
    let account = svm.get_account(rate_limit).expect("rate limit account should exist");
    let state = RateLimit::try_deserialize(&mut account.data.as_slice())
        .expect("should deserialize as a RateLimit");
    state.amount_transferred
}

/// Writes a valid *legacy* SPL Token mint straight into the SVM.
///
/// Deliberately owned by the legacy token program rather than Token-2022: it
/// deserializes fine as a mint, so only an owner check can tell it apart. This is
/// what makes the Token-2022 check in `initialize` meaningful rather than redundant
/// with Anchor's own `InterfaceAccount<Mint>` validation.
///
/// The 82-byte legacy mint layout is written by hand to avoid pulling
/// `solana-program-pack` and friends in as dev-dependencies just for this:
///   0..4   mint_authority COption tag (1 = Some)
///   4..36  mint_authority
///   36..44 supply (u64 LE)
///   44     decimals
///   45     is_initialized
///   46..50 freeze_authority COption tag (0 = None)
///   50..82 freeze_authority (zeroed)
pub fn setup_legacy_mint(svm: &mut LiteSVM, mint: &Keypair, authority: &Pubkey) {
    let mut data = vec![0u8; 82];
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..36].copy_from_slice(authority.as_ref());
    data[36..44].copy_from_slice(&0u64.to_le_bytes());
    data[44] = 9;                                       // decimals
    data[45] = 1;                                       // is_initialized
    data[46..50].copy_from_slice(&0u32.to_le_bytes());  // freeze_authority: None

    svm.set_account(
        mint.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data,
            owner: anchor_spl::token::ID,
            executable: false,
            rent_epoch: 0,
        },
    ).unwrap();
}

/// Builds an `initialize` instruction directly, for tests that need it to fail.
pub fn build_initialize_ix(
    payer: &Pubkey,
    mint: &Pubkey,
    rate_limit: &Pubkey,
    program_id: &Address,
) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: *payer,
            mint: *mint,
            rate_limit: *rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    )
}

/// Reads an SPL token account's balance.
///
/// The amount sits at bytes 64..72 of the base token account layout, which is the
/// same for Token-2022 as for legacy SPL Token (extensions are appended after it).
pub fn token_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    let account = svm.get_account(ata).expect("token account should exist");
    u64::from_le_bytes(account.data[64..72].try_into().expect("token account too short"))
}
