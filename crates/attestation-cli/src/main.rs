mod workflow;

use std::{
    path::PathBuf,
    process, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use attestation_prover::{
    capture_wallet_snapshot, dev_mode_status, fetch_sequencer_commitment_root, prove_live,
    prove_request, read_private_snapshot, read_prover_input, write_private_snapshot, write_proof,
    DevModeStatus, ProofRequest,
};
use attestation_types::{
    digest_from_hex, digest_to_hex, program_owner_from_hex, program_owner_to_hex, GateContext,
    ProofTransport,
};
use balance_gate_core::DEFAULT_ON_CHAIN_PROOF_AGE_MS;
use clap::{Args, Parser, Subcommand, ValueEnum};
use proofgate_messaging::{
    admission_challenge_matches, pack_envelope, receive_challenge, receive_envelope,
    LogosCoreClient, DEFAULT_CHUNK_BYTES,
};
use tokenstudio_config::{
    create_token_invocation, mint_token_invocation, read_gate_config, read_token_config,
    write_gate_config, write_token_config, GateConfig, TokenConfig, TOKEN_CONFIG_VERSION,
};
use workflow::{
    balance_gate_program_id, compose_on_chain_claim, create_on_chain_claim, create_presentation,
    current_on_chain_challenge, deploy_balance_gate, fetch_on_chain_badge, fetch_on_chain_state,
    generate_lez_account_key, generate_lez_private_account_key, generate_presenter_key,
    initialize_on_chain_state, issue_admission_challenge, issue_challenge, lez_account_id,
    lez_private_account_id, presenter_public_key, read_presentation, read_verification_challenge,
    simulate_on_chain_claim, submit_on_chain_claim, submit_on_chain_initialization,
    verify_presentation, verify_received_presentation, write_presentation,
    write_verification_challenge, ClaimOutputPaths,
};

#[derive(Debug, Parser)]
#[command(
    name = "proofgate",
    version,
    about = "No-code private token gates for Logos"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Version,
    ContextHash(ContextArgs),
    SequencerRoot(SequencerRootArgs),
    Prove(ProveArgs),
    Present(PresentArgs),
    Verify(VerifyArgs),
    Presenter {
        #[command(subcommand)]
        command: PresenterCommand,
    },
    Challenge {
        #[command(subcommand)]
        command: ChallengeCommand,
    },
    OnChain {
        #[command(subcommand)]
        command: OnChainCommand,
    },
    Messaging {
        #[command(subcommand)]
        command: MessagingCommand,
    },
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },
    Token {
        #[command(subcommand)]
        command: TokenCommand,
    },
    Gate {
        #[command(subcommand)]
        command: GateCommand,
    },
}

#[derive(Debug, Subcommand)]
enum MessagingCommand {
    AdmissionChallenge(MessagingAdmissionChallengeArgs),
    SendChallenge(MessagingSendChallengeArgs),
    ReceiveChallenge(MessagingReceiveChallengeArgs),
    Send(MessagingSendArgs),
    Receive(MessagingReceiveArgs),
    ReceiveVerify(MessagingReceiveVerifyArgs),
}

#[derive(Debug, Args)]
struct MessagingAdmissionChallengeArgs {
    #[arg(long)]
    gate: PathBuf,
    #[arg(long)]
    commitment_root_hex: String,
    #[arg(long)]
    group_id: String,
    #[arg(long)]
    member_address: String,
    #[arg(long, default_value_t = 60_000)]
    ttl_ms: u64,
    #[arg(long)]
    now_unix_ms: Option<u64>,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Clone, Debug, Args)]
struct MessagingConnectionArgs {
    #[arg(long, default_value = "logoscore")]
    logoscore_binary: PathBuf,
    #[arg(long)]
    config_dir: PathBuf,
    #[arg(long)]
    conversation_id: String,
    #[arg(long, default_value = "chat_module")]
    module: String,
}

#[derive(Debug, Args)]
struct MessagingSendChallengeArgs {
    #[command(flatten)]
    connection: MessagingConnectionArgs,
    #[arg(long)]
    challenge: PathBuf,
}

#[derive(Debug, Args)]
struct MessagingReceiveChallengeArgs {
    #[command(flatten)]
    connection: MessagingConnectionArgs,
    #[arg(long)]
    expected_sender: Option<String>,
    #[arg(long, default_value_t = 120_000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 1_000)]
    poll_ms: u64,
    #[arg(long)]
    gate: Option<PathBuf>,
    #[arg(long)]
    group_id: Option<String>,
    #[arg(long)]
    member_address: Option<String>,
    #[arg(long)]
    now_unix_ms: Option<u64>,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct MessagingSendArgs {
    #[command(flatten)]
    connection: MessagingConnectionArgs,
    #[arg(long)]
    envelope: PathBuf,
    #[arg(long, default_value_t = DEFAULT_CHUNK_BYTES)]
    chunk_bytes: usize,
    #[arg(long, default_value_t = 1)]
    copies: u8,
    #[arg(long, default_value_t = 2_000)]
    copy_delay_ms: u64,
    #[arg(long, default_value_t = 0)]
    message_delay_ms: u64,
}

#[derive(Clone, Debug, Args)]
struct MessagingReceiveOptions {
    #[command(flatten)]
    connection: MessagingConnectionArgs,
    #[arg(long)]
    transfer_id: Option<String>,
    #[arg(long)]
    expected_sender: Option<String>,
    #[arg(long, default_value_t = 120_000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 1_000)]
    poll_ms: u64,
}

#[derive(Debug, Args)]
struct MessagingReceiveArgs {
    #[command(flatten)]
    receive: MessagingReceiveOptions,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct MessagingReceiveVerifyArgs {
    #[command(flatten)]
    receive: MessagingReceiveOptions,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    gate: PathBuf,
    #[arg(long)]
    challenge: PathBuf,
    #[arg(long)]
    replay_cache: PathBuf,
    #[arg(long)]
    verifier_id: Option<String>,
    #[arg(long)]
    now_unix_ms: Option<u64>,
    #[arg(long)]
    admit_group_id: Option<String>,
    #[arg(long)]
    admit_address: Option<String>,
}

#[derive(Debug, Subcommand)]
enum PresenterCommand {
    Generate {
        #[arg(long)]
        output: PathBuf,
    },
    PublicKey {
        #[arg(long)]
        key: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ChallengeCommand {
    Create(ChallengeCreateArgs),
}

#[derive(Debug, Subcommand)]
enum OnChainCommand {
    ProgramId,
    AccountGenerate(OnChainAccountGenerateArgs),
    AccountId(OnChainAccountIdArgs),
    PrivateAccountGenerate(OnChainAccountGenerateArgs),
    PrivateAccountId(OnChainAccountIdArgs),
    Deploy(OnChainDeployArgs),
    Init(OnChainInitArgs),
    FetchState(OnChainFetchStateArgs),
    FetchBadge(OnChainFetchBadgeArgs),
    InitializeSubmit(OnChainInitializeSubmitArgs),
    Challenge(OnChainChallengeArgs),
    Present(OnChainPresentArgs),
    Simulate(OnChainSimulateArgs),
    Compose(OnChainComposeArgs),
    ClaimSubmit(OnChainClaimSubmitArgs),
}

#[derive(Debug, Args)]
struct OnChainAccountGenerateArgs {
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct OnChainAccountIdArgs {
    #[arg(long)]
    key: PathBuf,
}

#[derive(Debug, Args)]
struct OnChainDeployArgs {
    #[arg(long)]
    sequencer_url: String,
}

#[derive(Debug, Args)]
struct OnChainInitArgs {
    #[arg(long)]
    gate: PathBuf,
    #[arg(long)]
    commitment_root_hex: String,
    #[arg(long)]
    challenge_nonce_hex: String,
    #[arg(long, default_value_t = DEFAULT_ON_CHAIN_PROOF_AGE_MS)]
    max_proof_age_ms: u64,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct OnChainFetchStateArgs {
    #[arg(long)]
    sequencer_url: String,
    #[arg(long)]
    gate_account_id_hex: String,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct OnChainFetchBadgeArgs {
    #[arg(long)]
    sequencer_url: String,
    #[arg(long)]
    badge_account_id_hex: String,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct OnChainInitializeSubmitArgs {
    #[arg(long)]
    sequencer_url: String,
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    gate_key: PathBuf,
}

#[derive(Debug, Args)]
struct OnChainChallengeArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    claim_account_id_hex: String,
}

#[derive(Debug, Args)]
struct OnChainPresentArgs {
    #[arg(long)]
    proof: PathBuf,
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    claim_account_id_hex: String,
    #[arg(long)]
    presenter_key: PathBuf,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct OnChainSimulateArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    claim: PathBuf,
    #[arg(long)]
    gate_account_id_hex: String,
    #[arg(long)]
    badge_account_id_hex: String,
}

#[derive(Debug, Args)]
struct OnChainComposeArgs {
    #[arg(long)]
    proof: PathBuf,
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    claim: PathBuf,
    #[arg(long)]
    gate_account_id_hex: String,
    #[arg(long)]
    badge_key: PathBuf,
    #[arg(long)]
    output_badge: PathBuf,
    #[arg(long)]
    output_lez_proof: PathBuf,
}

#[derive(Debug, Args)]
struct OnChainClaimSubmitArgs {
    #[arg(long)]
    sequencer_url: String,
    #[arg(long)]
    proof: PathBuf,
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    claim: PathBuf,
    #[arg(long)]
    gate_account_id_hex: String,
    #[arg(long)]
    badge_key: PathBuf,
    #[arg(long)]
    output_badge: PathBuf,
    #[arg(long)]
    output_lez_proof: PathBuf,
}

#[derive(Debug, Args)]
struct ChallengeCreateArgs {
    #[arg(long)]
    gate: PathBuf,
    #[arg(long)]
    commitment_root_hex: String,
    #[arg(long)]
    verifier_id: Option<String>,
    #[arg(long, default_value_t = 60_000)]
    ttl_ms: u64,
    #[arg(long)]
    now_unix_ms: Option<u64>,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum TransportArg {
    Local,
    LogosMessaging,
    OnChain,
}

impl From<TransportArg> for ProofTransport {
    fn from(value: TransportArg) -> Self {
        match value {
            TransportArg::Local => Self::Local,
            TransportArg::LogosMessaging => Self::LogosMessaging,
            TransportArg::OnChain => Self::OnChain,
        }
    }
}

#[derive(Debug, Args)]
struct PresentArgs {
    #[arg(long)]
    proof: PathBuf,
    #[arg(long)]
    challenge: PathBuf,
    #[arg(long)]
    presenter_key: PathBuf,
    #[arg(long, value_enum, default_value_t = TransportArg::Local)]
    transport: TransportArg,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    #[arg(long)]
    gate: PathBuf,
    #[arg(long)]
    envelope: PathBuf,
    #[arg(long)]
    challenge: PathBuf,
    #[arg(long)]
    replay_cache: PathBuf,
    #[arg(long)]
    verifier_id: Option<String>,
    #[arg(long)]
    now_unix_ms: Option<u64>,
}

#[derive(Debug, Subcommand)]
enum WalletCommand {
    Snapshot(WalletSnapshotArgs),
}

#[derive(Debug, Args)]
struct WalletSnapshotArgs {
    #[arg(long, default_value = "wallet")]
    wallet_binary: PathBuf,
    #[arg(long)]
    account_id: String,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct ProveArgs {
    #[arg(long)]
    gate: PathBuf,
    #[arg(long, conflicts_with = "account_snapshot")]
    input: Option<PathBuf>,
    #[arg(long, conflicts_with = "input")]
    account_snapshot: Option<PathBuf>,
    #[arg(long)]
    sequencer_url: Option<String>,
    #[arg(long)]
    presenter_public_key_hex: String,
    #[arg(long)]
    issued_at_unix_ms: Option<u64>,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Subcommand)]
enum TokenCommand {
    Select(TokenConfigArgs),
    Create(TokenCreateArgs),
    Mint(TokenMintArgs),
}

#[derive(Debug, Subcommand)]
enum GateCommand {
    Init(GateInitArgs),
    Hash {
        #[arg(long)]
        gate: PathBuf,
    },
    Show {
        #[arg(long)]
        gate: PathBuf,
    },
}

#[derive(Clone, Debug, Args)]
struct TokenConfigArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    symbol: String,
    #[arg(long, default_value_t = 0)]
    decimals: u8,
    #[arg(long)]
    definition_account_id_hex: String,
    #[arg(long)]
    token_program_owner_hex: String,
    #[arg(long)]
    definition_account: String,
    #[arg(long)]
    supply_account: String,
    #[arg(long)]
    issuer_account: String,
    #[arg(long)]
    total_supply: u128,
    #[arg(long)]
    output: PathBuf,
}

impl TokenConfigArgs {
    fn config(&self) -> Result<TokenConfig, String> {
        Ok(TokenConfig {
            schema_version: TOKEN_CONFIG_VERSION,
            name: self.name.clone(),
            symbol: self.symbol.clone(),
            decimals: self.decimals,
            definition_account_id: digest_from_hex(&self.definition_account_id_hex)
                .map_err(|error| format!("invalid --definition-account-id-hex: {error}"))?,
            token_program_owner: program_owner_from_hex(&self.token_program_owner_hex)
                .map_err(|error| format!("invalid --token-program-owner-hex: {error}"))?,
            definition_account: self.definition_account.clone(),
            supply_account: self.supply_account.clone(),
            issuer_account: self.issuer_account.clone(),
            total_supply: self.total_supply,
        })
    }
}

#[derive(Debug, Args)]
struct TokenCreateArgs {
    #[command(flatten)]
    token: TokenConfigArgs,
    #[arg(long, default_value = "wallet")]
    wallet_binary: PathBuf,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct TokenMintArgs {
    #[arg(long)]
    token: PathBuf,
    #[arg(long)]
    holder: String,
    #[arg(long)]
    amount: u128,
    #[arg(long, default_value = "wallet")]
    wallet_binary: PathBuf,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct GateInitArgs {
    #[arg(long)]
    token: PathBuf,
    #[arg(long)]
    application_id: String,
    #[arg(long)]
    gate_id: String,
    #[arg(long)]
    threshold: u128,
    #[arg(long)]
    verifier_id: String,
    #[arg(long)]
    expires_at_unix_ms: Option<u64>,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct SequencerRootArgs {
    #[arg(long)]
    sequencer_url: String,
}

#[derive(Debug, Args)]
struct ContextArgs {
    #[arg(long)]
    application_id: String,
    #[arg(long)]
    gate_id: String,
    #[arg(long)]
    token_owner_hex: String,
    #[arg(long)]
    token_definition_id_hex: String,
    #[arg(long)]
    threshold: u128,
    #[arg(long)]
    verifier_id: String,
    #[arg(long)]
    expires_at_unix_ms: Option<u64>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
        Command::ContextHash(args) => {
            println!(
                "{}",
                digest_to_hex(&context_from_args(args)?.context_hash())
            );
        }
        Command::SequencerRoot(args) => {
            let root = fetch_sequencer_commitment_root(&args.sequencer_url)
                .await
                .map_err(|error| error.to_string())?;
            println!("{}", digest_to_hex(&root));
        }
        Command::Prove(args) => run_prove(args).await?,
        Command::Present(args) => run_present(args)?,
        Command::Verify(args) => run_verify(args)?,
        Command::Presenter { command } => run_presenter_command(command)?,
        Command::Challenge { command } => run_challenge_command(command)?,
        Command::OnChain { command } => run_on_chain_command(command).await?,
        Command::Messaging { command } => run_messaging_command(command)?,
        Command::Wallet { command } => run_wallet_command(command)?,
        Command::Token { command } => run_token_command(command)?,
        Command::Gate { command } => run_gate_command(command)?,
    }
    Ok(())
}

fn run_messaging_command(command: MessagingCommand) -> Result<(), String> {
    match command {
        MessagingCommand::AdmissionChallenge(args) => {
            let commitment_root = digest_from_hex(&args.commitment_root_hex)
                .map_err(|error| format!("invalid --commitment-root-hex: {error}"))?;
            let now = args.now_unix_ms.unwrap_or(now_unix_ms()?);
            let challenge = issue_admission_challenge(
                &args.gate,
                commitment_root,
                &args.group_id,
                &args.member_address,
                now,
                args.ttl_ms,
                &args.output,
            )?;
            println!("Admission challenge written to {}", args.output.display());
            println!("Challenge digest: {}", digest_to_hex(&challenge.digest()));
            println!("Verifier ID: {}", challenge.verifier_id);
        }
        MessagingCommand::SendChallenge(args) => {
            let challenge = read_verification_challenge(&args.challenge)?;
            messaging_client(&args.connection)
                .send_challenge(&args.connection.conversation_id, &challenge)
                .map_err(|error| error.to_string())?;
            println!("Challenge sent over Logos Messaging");
            println!("Challenge digest: {}", digest_to_hex(&challenge.digest()));
        }
        MessagingCommand::ReceiveChallenge(args) => {
            if args.poll_ms == 0 {
                return Err("--poll-ms must be greater than zero".to_owned());
            }
            let challenge = receive_challenge(
                &messaging_client(&args.connection),
                &args.connection.conversation_id,
                args.expected_sender.as_deref(),
                Duration::from_millis(args.timeout_ms),
                Duration::from_millis(args.poll_ms),
            )
            .map_err(|error| error.to_string())?;
            match (&args.gate, &args.group_id, &args.member_address) {
                (None, None, None) => {}
                (Some(gate_path), Some(group_id), Some(member_address)) => {
                    let gate = read_gate_config(gate_path).map_err(|error| error.to_string())?;
                    if challenge.gate_context_hash != gate.context_hash() {
                        return Err(
                            "verification denied [1006]: challenge is for a different gate"
                                .to_owned(),
                        );
                    }
                    if challenge.verifier_id != gate.context.verifier_id {
                        return Err(
                            "verification denied [1008]: challenge is from a different verifier"
                                .to_owned(),
                        );
                    }
                    if !admission_challenge_matches(&challenge, group_id, member_address) {
                        return Err(
                            "verification denied [1006]: challenge is not bound to the expected group and member"
                                .to_owned(),
                        );
                    }
                    let now = args.now_unix_ms.unwrap_or(now_unix_ms()?);
                    if challenge.is_expired(now) {
                        return Err("verification denied [1007]: challenge has expired".to_owned());
                    }
                }
                _ => {
                    return Err(
                        "--gate, --group-id, and --member-address must be provided together"
                            .to_owned(),
                    );
                }
            }
            write_verification_challenge(&args.output, &challenge)?;
            println!("Challenge received from Logos Messaging");
            println!("Challenge digest: {}", digest_to_hex(&challenge.digest()));
            println!("Challenge written to {}", args.output.display());
        }
        MessagingCommand::Send(args) => {
            if !(1..=3).contains(&args.copies) {
                return Err("--copies must be between 1 and 3".to_owned());
            }
            let envelope = read_presentation(&args.envelope)?;
            let bundle =
                pack_envelope(&envelope, args.chunk_bytes).map_err(|error| error.to_string())?;
            let client = messaging_client(&args.connection);
            for copy in 0..args.copies {
                client
                    .send_bundle_with_delay(
                        &args.connection.conversation_id,
                        &bundle,
                        Duration::from_millis(args.message_delay_ms),
                    )
                    .map_err(|error| error.to_string())?;
                if copy + 1 < args.copies {
                    thread::sleep(Duration::from_millis(args.copy_delay_ms));
                }
            }
            println!("Proof sent over Logos Messaging");
            println!("Transfer ID: {}", bundle.transfer_id);
            println!(
                "Messages: {} unique x {} copies",
                bundle.messages.len(),
                args.copies
            );
        }
        MessagingCommand::Receive(args) => {
            let (transfer_id, envelope) = receive_presentation(&args.receive)?;
            write_presentation(&args.output, &envelope)?;
            println!("Proof received from Logos Messaging");
            println!("Transfer ID: {transfer_id}");
            println!("Envelope written to {}", args.output.display());
        }
        MessagingCommand::ReceiveVerify(args) => {
            let expected_challenge = read_verification_challenge(&args.challenge)?;
            let mut receive = args.receive.clone();
            let admission = match (&args.admit_group_id, &args.admit_address) {
                (None, None) => None,
                (Some(group_id), Some(member_address)) => {
                    if receive
                        .expected_sender
                        .as_deref()
                        .is_some_and(|sender| sender != member_address)
                    {
                        return Err(
                            "--expected-sender must equal --admit-address for chat admission"
                                .to_owned(),
                        );
                    }
                    receive.expected_sender = Some(member_address.clone());
                    Some((group_id.clone(), member_address.clone()))
                }
                _ => {
                    return Err(
                        "--admit-group-id and --admit-address must be provided together".to_owned(),
                    );
                }
            };
            let (transfer_id, envelope) = receive_presentation(&receive)?;
            if let Some((group_id, member_address)) = &admission {
                if !admission_challenge_matches(&envelope.challenge, group_id, member_address) {
                    return Err(
                        "verification denied [1006]: challenge is not bound to the requested group and member"
                            .to_owned(),
                    );
                }
            }
            let now = args.now_unix_ms.unwrap_or(now_unix_ms()?);
            let outcome = verify_received_presentation(
                &args.gate,
                &envelope,
                &expected_challenge,
                &args.replay_cache,
                args.verifier_id.as_deref(),
                now,
            )?;
            write_presentation(&args.output, &envelope)?;
            println!("ALLOW (Logos Messaging)");
            println!("Transfer ID: {transfer_id}");
            println!(
                "Context hash: {}",
                digest_to_hex(&outcome.verified.context_hash)
            );
            println!("Consumed challenges: {}", outcome.consumed_challenges);
            println!("Verified envelope written to {}", args.output.display());
            if let Some((group_id, member_address)) = admission {
                messaging_client(&args.receive.connection)
                    .add_group_member(&group_id, &member_address)
                    .map_err(|error| error.to_string())?;
                println!("Admission requested for {member_address} in group {group_id}");
            }
        }
    }
    Ok(())
}

fn receive_presentation(
    options: &MessagingReceiveOptions,
) -> Result<(String, attestation_types::AttestationEnvelope), String> {
    if options.poll_ms == 0 {
        return Err("--poll-ms must be greater than zero".to_owned());
    }
    receive_envelope(
        &messaging_client(&options.connection),
        &options.connection.conversation_id,
        options.transfer_id.as_deref(),
        options.expected_sender.as_deref(),
        Duration::from_millis(options.timeout_ms),
        Duration::from_millis(options.poll_ms),
    )
    .map_err(|error| error.to_string())
}

fn messaging_client(connection: &MessagingConnectionArgs) -> LogosCoreClient {
    LogosCoreClient::new(&connection.logoscore_binary, &connection.config_dir)
        .with_module(&connection.module)
}

async fn run_on_chain_command(command: OnChainCommand) -> Result<(), String> {
    match command {
        OnChainCommand::ProgramId => {
            println!("{}", program_owner_to_hex(&balance_gate_program_id()));
        }
        OnChainCommand::AccountGenerate(args) => {
            let account_id = generate_lez_account_key(&args.output)?;
            println!(
                "LEZ account key written to {} with restricted permissions",
                args.output.display()
            );
            println!("Account ID: {account_id}");
            println!("Account ID hex: {}", digest_to_hex(account_id.value()));
        }
        OnChainCommand::AccountId(args) => {
            let account_id = lez_account_id(&args.key)?;
            println!("{account_id}");
            println!("Account ID hex: {}", digest_to_hex(account_id.value()));
        }
        OnChainCommand::PrivateAccountGenerate(args) => {
            let account_id = generate_lez_private_account_key(&args.output)?;
            println!(
                "LEZ private account key written to {} with restricted permissions",
                args.output.display()
            );
            println!("Account ID: {account_id}");
            println!("Account ID hex: {}", digest_to_hex(account_id.value()));
        }
        OnChainCommand::PrivateAccountId(args) => {
            let account_id = lez_private_account_id(&args.key)?;
            println!("{account_id}");
            println!("Account ID hex: {}", digest_to_hex(account_id.value()));
        }
        OnChainCommand::Deploy(args) => {
            let transaction_hash = deploy_balance_gate(&args.sequencer_url).await?;
            println!("Balance gate deployment submitted");
            println!(
                "Program ID: {}",
                program_owner_to_hex(&balance_gate_program_id())
            );
            println!("Transaction hash: {transaction_hash}");
        }
        OnChainCommand::Init(args) => {
            let commitment_root = digest_from_hex(&args.commitment_root_hex)
                .map_err(|error| format!("invalid --commitment-root-hex: {error}"))?;
            let nonce = digest_from_hex(&args.challenge_nonce_hex)
                .map_err(|error| format!("invalid --challenge-nonce-hex: {error}"))?;
            let state = initialize_on_chain_state(
                &args.gate,
                commitment_root,
                nonce,
                args.max_proof_age_ms,
                &args.output,
            )?;
            println!("On-chain gate state written to {}", args.output.display());
            println!("Context hash: {}", digest_to_hex(&state.context_hash));
            println!(
                "Authorized commitment root: {}",
                digest_to_hex(&state.commitment_root)
            );
            println!("Maximum proof age: {} ms", state.max_proof_age_ms);
            println!(
                "Program ID: {}",
                program_owner_to_hex(&balance_gate_program_id())
            );
        }
        OnChainCommand::FetchState(args) => {
            let gate_account_id = digest_from_hex(&args.gate_account_id_hex)
                .map_err(|error| format!("invalid --gate-account-id-hex: {error}"))?;
            let state =
                fetch_on_chain_state(&args.sequencer_url, gate_account_id, &args.output).await?;
            println!("Current gate state written to {}", args.output.display());
            println!("Context hash: {}", digest_to_hex(&state.context_hash));
            println!("Claim counter: {}", state.claim_counter);
        }
        OnChainCommand::FetchBadge(args) => {
            let badge_account_id = digest_from_hex(&args.badge_account_id_hex)
                .map_err(|error| format!("invalid --badge-account-id-hex: {error}"))?;
            let badge =
                fetch_on_chain_badge(&args.sequencer_url, badge_account_id, &args.output).await?;
            println!("Access badge written to {}", args.output.display());
            println!("Context hash: {}", digest_to_hex(&badge.context_hash));
            println!(
                "Presenter key: {}",
                digest_to_hex(&badge.presenter_public_key)
            );
            println!("Claim number: {}", badge.claim_number);
            println!("Proof issued at: {}", badge.proof_issued_at_unix_ms);
        }
        OnChainCommand::InitializeSubmit(args) => {
            let (transaction_hash, account_id) =
                submit_on_chain_initialization(&args.sequencer_url, &args.state, &args.gate_key)
                    .await?;
            println!("Gate initialization submitted");
            println!("Gate account ID: {account_id}");
            println!("Gate account ID hex: {}", digest_to_hex(account_id.value()));
            println!("Transaction hash: {transaction_hash}");
        }
        OnChainCommand::Challenge(args) => {
            let claim_account_id = digest_from_hex(&args.claim_account_id_hex)
                .map_err(|error| format!("invalid --claim-account-id-hex: {error}"))?;
            let challenge = current_on_chain_challenge(&args.state, claim_account_id)?;
            println!("{}", digest_to_hex(&challenge.digest()));
        }
        OnChainCommand::Present(args) => {
            let claim_account_id = digest_from_hex(&args.claim_account_id_hex)
                .map_err(|error| format!("invalid --claim-account-id-hex: {error}"))?;
            let claim = create_on_chain_claim(
                &args.proof,
                &args.state,
                claim_account_id,
                &args.presenter_key,
                &args.output,
            )?;
            println!("On-chain claim written to {}", args.output.display());
            println!(
                "Presenter signature bytes: {}",
                claim.presenter_signature.len()
            );
        }
        OnChainCommand::Simulate(args) => {
            let gate_account_id = digest_from_hex(&args.gate_account_id_hex)
                .map_err(|error| format!("invalid --gate-account-id-hex: {error}"))?;
            let badge_account_id = digest_from_hex(&args.badge_account_id_hex)
                .map_err(|error| format!("invalid --badge-account-id-hex: {error}"))?;
            let result = simulate_on_chain_claim(
                &args.state,
                &args.claim,
                gate_account_id,
                badge_account_id,
            )?;
            println!("ALLOW (LEZ guest execution)");
            println!("Claim number: {}", result.badge.claim_number);
            println!("Valid from: {:?}", result.valid_from_unix_ms);
            println!("Valid until: {:?}", result.valid_until_unix_ms);
        }
        OnChainCommand::Compose(args) => {
            if dev_mode_status().map_err(|error| error.to_string())? != DevModeStatus::Disabled {
                return Err(
                    "RISC0_DEV_MODE must be 0 or unset for LEZ proof composition".to_owned(),
                );
            }
            let gate_account_id = digest_from_hex(&args.gate_account_id_hex)
                .map_err(|error| format!("invalid --gate-account-id-hex: {error}"))?;
            println!("RISC0_DEV_MODE: disabled (real LEZ composition)");
            let result = compose_on_chain_claim(
                &args.proof,
                &args.state,
                &args.claim,
                gate_account_id,
                &args.badge_key,
                &args.output_badge,
                &args.output_lez_proof,
            )?;
            println!(
                "Private access badge written to {}",
                args.output_badge.display()
            );
            println!("LEZ proof written to {}", args.output_lez_proof.display());
            println!("Proof bytes: {}", result.proof_bytes);
            println!("Public post states: {}", result.public_post_states);
            println!(
                "Encrypted private post states: {}",
                result.encrypted_private_post_states
            );
            println!("Private commitments: {}", result.private_commitments);
            println!("Nullifiers: {}", result.nullifiers);
            println!("Gate proving time: {} ms", result.gate_proving_ms);
            println!("LEZ PPE proving time: {} ms", result.outer_proving_ms);
            println!("Total composition time: {} ms", result.total_proving_ms);
            println!("Gate total cycles: {}", result.gate_total_cycles);
            println!("Gate user cycles: {}", result.gate_user_cycles);
            println!("Gate paging cycles: {}", result.gate_paging_cycles);
            println!("Gate segments: {}", result.gate_segments);
            println!("LEZ PPE total cycles: {}", result.outer_total_cycles);
            println!("LEZ PPE user cycles: {}", result.outer_user_cycles);
            println!("LEZ PPE paging cycles: {}", result.outer_paging_cycles);
            println!("LEZ PPE segments: {}", result.outer_segments);
        }
        OnChainCommand::ClaimSubmit(args) => {
            if dev_mode_status().map_err(|error| error.to_string())? != DevModeStatus::Disabled {
                return Err("RISC0_DEV_MODE must be 0 or unset for LEZ claim submission".to_owned());
            }
            let gate_account_id = digest_from_hex(&args.gate_account_id_hex)
                .map_err(|error| format!("invalid --gate-account-id-hex: {error}"))?;
            println!("RISC0_DEV_MODE: disabled (real LEZ claim submission)");
            let result = submit_on_chain_claim(
                &args.sequencer_url,
                &args.proof,
                &args.state,
                &args.claim,
                gate_account_id,
                &args.badge_key,
                ClaimOutputPaths {
                    badge: &args.output_badge,
                    proof: &args.output_lez_proof,
                },
            )
            .await?;
            println!(
                "Private access badge written to {}",
                args.output_badge.display()
            );
            println!("LEZ proof written to {}", args.output_lez_proof.display());
            println!("Proof bytes: {}", result.composition.proof_bytes);
            println!(
                "Public post states: {}",
                result.composition.public_post_states
            );
            println!(
                "Encrypted private post states: {}",
                result.composition.encrypted_private_post_states
            );
            println!(
                "Private commitments: {}",
                result.composition.private_commitments
            );
            println!("Nullifiers: {}", result.composition.nullifiers);
            println!(
                "Gate proving time: {} ms",
                result.composition.gate_proving_ms
            );
            println!(
                "LEZ PPE proving time: {} ms",
                result.composition.outer_proving_ms
            );
            println!(
                "Total composition time: {} ms",
                result.composition.total_proving_ms
            );
            println!(
                "Gate total cycles: {}",
                result.composition.gate_total_cycles
            );
            println!("Gate user cycles: {}", result.composition.gate_user_cycles);
            println!(
                "Gate paging cycles: {}",
                result.composition.gate_paging_cycles
            );
            println!("Gate segments: {}", result.composition.gate_segments);
            println!(
                "LEZ PPE total cycles: {}",
                result.composition.outer_total_cycles
            );
            println!(
                "LEZ PPE user cycles: {}",
                result.composition.outer_user_cycles
            );
            println!(
                "LEZ PPE paging cycles: {}",
                result.composition.outer_paging_cycles
            );
            println!("LEZ PPE segments: {}", result.composition.outer_segments);
            println!("Transaction hash: {}", result.transaction_hash);
        }
    }
    Ok(())
}

fn run_presenter_command(command: PresenterCommand) -> Result<(), String> {
    match command {
        PresenterCommand::Generate { output } => {
            let public_key = generate_presenter_key(&output)?;
            println!(
                "Presenter key written to {} with restricted permissions",
                output.display()
            );
            println!("Public key: {}", digest_to_hex(&public_key));
        }
        PresenterCommand::PublicKey { key } => {
            println!("{}", digest_to_hex(&presenter_public_key(&key)?));
        }
    }
    Ok(())
}

fn run_challenge_command(command: ChallengeCommand) -> Result<(), String> {
    match command {
        ChallengeCommand::Create(args) => {
            let commitment_root = digest_from_hex(&args.commitment_root_hex)
                .map_err(|error| format!("invalid --commitment-root-hex: {error}"))?;
            let now = args.now_unix_ms.unwrap_or(now_unix_ms()?);
            let challenge = issue_challenge(
                &args.gate,
                commitment_root,
                args.verifier_id.as_deref(),
                now,
                args.ttl_ms,
                &args.output,
            )?;
            println!("Challenge written to {}", args.output.display());
            println!("Challenge digest: {}", digest_to_hex(&challenge.digest()));
            println!("Expires at: {}", challenge.expires_at_unix_ms);
        }
    }
    Ok(())
}

fn run_present(args: PresentArgs) -> Result<(), String> {
    let envelope = create_presentation(
        &args.proof,
        &args.challenge,
        &args.presenter_key,
        args.transport.into(),
        &args.output,
    )?;
    println!("Presentation written to {}", args.output.display());
    println!("Context hash: {}", digest_to_hex(&envelope.context_hash()));
    println!(
        "Presenter signature bytes: {}",
        envelope.presenter_signature.len()
    );
    Ok(())
}

fn run_verify(args: VerifyArgs) -> Result<(), String> {
    let now = args.now_unix_ms.unwrap_or(now_unix_ms()?);
    let outcome = verify_presentation(
        &args.gate,
        &args.envelope,
        &args.challenge,
        &args.replay_cache,
        args.verifier_id.as_deref(),
        now,
    )?;
    println!("ALLOW");
    println!(
        "Context hash: {}",
        digest_to_hex(&outcome.verified.context_hash)
    );
    println!(
        "Commitment root: {}",
        digest_to_hex(&outcome.verified.commitment_root)
    );
    println!(
        "Presenter public key: {}",
        digest_to_hex(&outcome.verified.presenter_public_key)
    );
    println!("Consumed challenges: {}", outcome.consumed_challenges);
    Ok(())
}

fn run_wallet_command(command: WalletCommand) -> Result<(), String> {
    match command {
        WalletCommand::Snapshot(args) => {
            let snapshot = capture_wallet_snapshot(args.wallet_binary, &args.account_id)
                .map_err(|error| error.to_string())?;
            write_private_snapshot(&args.output, &snapshot).map_err(|error| error.to_string())?;
            println!(
                "Private account snapshot written to {} with restricted permissions",
                args.output.display()
            );
        }
    }
    Ok(())
}

async fn run_prove(args: ProveArgs) -> Result<(), String> {
    match dev_mode_status().map_err(|error| error.to_string())? {
        DevModeStatus::Disabled => println!("RISC0_DEV_MODE: disabled (real proving)"),
        DevModeStatus::Enabled => {
            return Err("RISC0_DEV_MODE must be 0 or unset for proof generation".to_owned());
        }
    }

    let gate = read_gate_config(&args.gate).map_err(|error| error.to_string())?;
    let presenter_public_key = digest_from_hex(&args.presenter_public_key_hex)
        .map_err(|error| format!("invalid --presenter-public-key-hex: {error}"))?;
    let issued_at_unix_ms = args.issued_at_unix_ms.unwrap_or(now_unix_ms()?);

    let proof = match (args.input, args.account_snapshot, args.sequencer_url) {
        (Some(input_path), None, None) => {
            let input = read_prover_input(input_path).map_err(|error| error.to_string())?;
            prove_request(&ProofRequest {
                gate,
                input,
                presenter_public_key,
                issued_at_unix_ms,
            })
            .map_err(|error| error.to_string())?
        }
        (None, Some(snapshot_path), Some(sequencer_url)) => {
            let snapshot =
                read_private_snapshot(snapshot_path).map_err(|error| error.to_string())?;
            prove_live(
                gate,
                snapshot,
                &sequencer_url,
                presenter_public_key,
                issued_at_unix_ms,
            )
            .await
            .map_err(|error| error.to_string())?
        }
        _ => {
            return Err(
                "provide either --input, or both --account-snapshot and --sequencer-url".to_owned(),
            );
        }
    };

    write_proof(&args.output, &proof).map_err(|error| error.to_string())?;
    println!("Proof written to {}", args.output.display());
    println!(
        "Context hash: {}",
        digest_to_hex(&proof.journal.context_hash)
    );
    println!(
        "Commitment root: {}",
        digest_to_hex(&proof.journal.commitment_root)
    );
    println!("Receipt bytes: {}", proof.receipt.len());
    Ok(())
}

fn run_token_command(command: TokenCommand) -> Result<(), String> {
    match command {
        TokenCommand::Select(args) => {
            let config = args.config()?;
            write_token_config(&args.output, &config).map_err(|error| error.to_string())?;
            println!("Token config written to {}", args.output.display());
        }
        TokenCommand::Create(args) => {
            let config = args.token.config()?;
            config.validate().map_err(|error| error.to_string())?;
            let invocation = create_token_invocation(args.wallet_binary, &config);
            println!("LEZ wallet command: {}", invocation.display());
            if !args.dry_run {
                invocation.run().map_err(|error| error.to_string())?;
            }
            write_token_config(&args.token.output, &config).map_err(|error| error.to_string())?;
            println!("Token config written to {}", args.token.output.display());
        }
        TokenCommand::Mint(args) => {
            let token = read_token_config(&args.token).map_err(|error| error.to_string())?;
            let invocation =
                mint_token_invocation(args.wallet_binary, &token, &args.holder, args.amount)
                    .map_err(|error| error.to_string())?;
            println!("LEZ wallet command: {}", invocation.display());
            if !args.dry_run {
                invocation.run().map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

fn run_gate_command(command: GateCommand) -> Result<(), String> {
    match command {
        GateCommand::Init(args) => {
            if let Some(expiry) = args.expires_at_unix_ms {
                if expiry <= now_unix_ms()? {
                    return Err("--expires-at-unix-ms must be in the future".to_owned());
                }
            }
            let token = read_token_config(&args.token).map_err(|error| error.to_string())?;
            let context = GateContext {
                application_id: args.application_id,
                gate_id: args.gate_id,
                token_program_owner: token.token_program_owner,
                token_definition_id: token.definition_account_id,
                threshold: args.threshold,
                verifier_id: args.verifier_id,
                expires_at_unix_ms: args.expires_at_unix_ms,
            };
            let config = GateConfig::new(token, context).map_err(|error| error.to_string())?;
            write_gate_config(&args.output, &config).map_err(|error| error.to_string())?;
            println!("Gate config written to {}", args.output.display());
            println!("Context hash: {}", digest_to_hex(&config.context_hash()));
        }
        GateCommand::Hash { gate } => {
            let config = read_gate_config(gate).map_err(|error| error.to_string())?;
            println!("{}", digest_to_hex(&config.context_hash()));
        }
        GateCommand::Show { gate } => {
            let config = read_gate_config(gate).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&config).map_err(|error| error.to_string())?
            );
        }
    }
    Ok(())
}

fn context_from_args(args: ContextArgs) -> Result<GateContext, String> {
    Ok(GateContext {
        application_id: args.application_id,
        gate_id: args.gate_id,
        token_program_owner: program_owner_from_hex(&args.token_owner_hex)
            .map_err(|error| format!("invalid --token-owner-hex: {error}"))?,
        token_definition_id: digest_from_hex(&args.token_definition_id_hex)
            .map_err(|error| format!("invalid --token-definition-id-hex: {error}"))?,
        threshold: args.threshold,
        verifier_id: args.verifier_id,
        expires_at_unix_ms: args.expires_at_unix_ms,
    })
}

fn now_unix_ms() -> Result<u64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before Unix epoch".to_owned())?;
    u64::try_from(duration.as_millis()).map_err(|_| "current time exceeds u64".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_gate_hash_command() {
        let cli = Cli::try_parse_from([
            "proofgate",
            "gate",
            "hash",
            "--gate",
            "examples/gates/founders.json",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Command::Gate {
                command: GateCommand::Hash { .. }
            }
        ));
    }

    #[test]
    fn context_hash_requires_specific_token_definition() {
        let error = Cli::try_parse_from([
            "proofgate",
            "context-hash",
            "--application-id",
            "tokenstudio",
            "--gate-id",
            "founders",
            "--token-owner-hex",
            &"11".repeat(32),
            "--threshold",
            "100",
            "--verifier-id",
            "logos-chat:founders",
        ])
        .unwrap_err();

        assert!(error.to_string().contains("token-definition-id-hex"));
    }

    #[test]
    fn parses_signed_on_chain_claim_submission() {
        let cli = Cli::try_parse_from([
            "proofgate",
            "on-chain",
            "claim-submit",
            "--sequencer-url",
            "http://127.0.0.1:3040",
            "--proof",
            "proof.json",
            "--state",
            "state.json",
            "--claim",
            "claim.json",
            "--gate-account-id-hex",
            &"11".repeat(32),
            "--badge-key",
            "badge.json",
            "--output-badge",
            "access-badge.json",
            "--output-lez-proof",
            "lez-proof.bin",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Command::OnChain {
                command: OnChainCommand::ClaimSubmit(_)
            }
        ));
    }

    #[test]
    fn parses_sender_bound_messaging_challenge_receive() {
        let cli = Cli::try_parse_from([
            "proofgate",
            "messaging",
            "receive-challenge",
            "--config-dir",
            "holder",
            "--conversation-id",
            "conversation",
            "--expected-sender",
            "logos:verifier",
            "--gate",
            "gate.json",
            "--group-id",
            "group",
            "--member-address",
            "logos:holder",
            "--output",
            "challenge.json",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Command::Messaging {
                command: MessagingCommand::ReceiveChallenge(_)
            }
        ));
    }

    #[test]
    fn parses_trusted_root_commands() {
        let root = "11".repeat(32);
        let challenge = Cli::try_parse_from([
            "proofgate",
            "challenge",
            "create",
            "--gate",
            "gate.json",
            "--commitment-root-hex",
            &root,
            "--output",
            "challenge.json",
        ])
        .unwrap();
        assert!(matches!(
            challenge.command,
            Command::Challenge {
                command: ChallengeCommand::Create(_)
            }
        ));

        let initialization = Cli::try_parse_from([
            "proofgate",
            "on-chain",
            "init",
            "--gate",
            "gate.json",
            "--commitment-root-hex",
            &root,
            "--challenge-nonce-hex",
            &root,
            "--output",
            "state.json",
        ])
        .unwrap();
        assert!(matches!(
            initialization.command,
            Command::OnChain {
                command: OnChainCommand::Init(_)
            }
        ));

        let sequencer = Cli::try_parse_from([
            "proofgate",
            "sequencer-root",
            "--sequencer-url",
            "http://127.0.0.1:3040",
        ])
        .unwrap();
        assert!(matches!(sequencer.command, Command::SequencerRoot(_)));
    }

    #[test]
    fn gate_config_version_is_current() {
        assert_eq!(tokenstudio_config::GATE_CONFIG_VERSION, 1);
    }
}
