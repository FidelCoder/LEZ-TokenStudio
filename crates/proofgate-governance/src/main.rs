use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use attestation_types::{digest_to_hex, AttestationEnvelope, Digest32};
use clap::{Args, Parser, Subcommand, ValueEnum};
use fs2::FileExt as _;
use proofgate_governance::{GovernanceState, VoteChoice};
use serde::{de::DeserializeOwned, Serialize};
use tokenstudio_config::read_gate_config;

#[derive(Debug, Parser)]
#[command(name = "proofgate-governance")]
#[command(about = "Proposal-bound private token governance reference integration")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Challenge(ChallengeArgs),
    Cast(CastArgs),
    Status(StatusArgs),
}

#[derive(Debug, Args)]
struct ChallengeArgs {
    #[arg(long)]
    gate: PathBuf,
    #[arg(long)]
    proposal_id: String,
    #[arg(long)]
    commitment_root_hex: String,
    #[arg(long, default_value_t = 60_000)]
    ttl_ms: u64,
    #[arg(long)]
    now_unix_ms: Option<u64>,
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ChoiceArg {
    Yes,
    No,
    Abstain,
}

impl From<ChoiceArg> for VoteChoice {
    fn from(value: ChoiceArg) -> Self {
        match value {
            ChoiceArg::Yes => Self::Yes,
            ChoiceArg::No => Self::No,
            ChoiceArg::Abstain => Self::Abstain,
        }
    }
}

#[derive(Debug, Args)]
struct CastArgs {
    #[arg(long)]
    gate: PathBuf,
    #[arg(long)]
    proposal_id: String,
    #[arg(long)]
    presentation: PathBuf,
    #[arg(long, value_enum)]
    choice: ChoiceArg,
    #[arg(long)]
    now_unix_ms: Option<u64>,
    #[arg(long)]
    state: PathBuf,
}

#[derive(Debug, Args)]
struct StatusArgs {
    #[arg(long)]
    state: PathBuf,
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Challenge(args) => issue_challenge(args),
        Command::Cast(args) => cast_vote(args),
        Command::Status(args) => show_status(args),
    }
}

fn issue_challenge(args: ChallengeArgs) -> Result<(), String> {
    let gate = read_gate_config(&args.gate).map_err(|error| error.to_string())?;
    let root = parse_digest(&args.commitment_root_hex, "commitment root")?;
    let _lock = lock_state(&args.state)?;
    let mut state = if args.state.exists() {
        read_json(&args.state)?
    } else {
        GovernanceState::new(&gate, &args.proposal_id).map_err(|error| error.to_string())?
    };
    state
        .validate(&gate, &args.proposal_id)
        .map_err(|error| error.to_string())?;
    let challenge = state
        .issue_challenge(
            &gate,
            root,
            args.now_unix_ms.unwrap_or(now_unix_ms()?),
            args.ttl_ms,
        )
        .map_err(|error| error.to_string())?;
    write_private_json_atomic(&args.state, &state)?;
    write_json(&args.output, &challenge)?;
    println!("Governance challenge written to {}", args.output.display());
    println!("Challenge digest: {}", digest_to_hex(&challenge.digest()));
    println!("Proposal: {}", args.proposal_id);
    Ok(())
}

fn cast_vote(args: CastArgs) -> Result<(), String> {
    let gate = read_gate_config(&args.gate).map_err(|error| error.to_string())?;
    let envelope: AttestationEnvelope = read_json(&args.presentation)?;
    let _lock = lock_state(&args.state)?;
    let mut state: GovernanceState = read_json(&args.state)?;
    state
        .validate(&gate, &args.proposal_id)
        .map_err(|error| error.to_string())?;
    let outcome = state
        .cast_vote(
            &gate,
            &envelope,
            args.choice.into(),
            args.now_unix_ms.unwrap_or(now_unix_ms()?),
        )
        .map_err(|error| error.to_string())?;
    write_private_json_atomic(&args.state, &state)?;
    println!("Vote accepted for proposal {}", args.proposal_id);
    println!(
        "Presenter pseudonym: {}",
        digest_to_hex(&outcome.presenter_public_key)
    );
    println!("Choice: {:?}", outcome.choice);
    println!("Total votes: {}", outcome.total_votes);
    Ok(())
}

fn show_status(args: StatusArgs) -> Result<(), String> {
    let _lock = lock_state(&args.state)?;
    let state: GovernanceState = read_json(&args.state)?;
    let yes = state
        .votes
        .values()
        .filter(|record| record.choice == VoteChoice::Yes)
        .count();
    let no = state
        .votes
        .values()
        .filter(|record| record.choice == VoteChoice::No)
        .count();
    let abstain = state.votes.len() - yes - no;
    println!("Proposal: {}", state.proposal_id);
    println!("Yes: {yes}");
    println!("No: {no}");
    println!("Abstain: {abstain}");
    println!("Pending challenges: {}", state.pending_challenges.len());
    println!("Consumed challenges: {}", state.consumed_challenges.len());
    Ok(())
}

fn now_unix_ms() -> Result<u64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?;
    u64::try_from(duration.as_millis()).map_err(|_| "Unix time does not fit u64".to_owned())
}

fn parse_digest(value: &str, label: &str) -> Result<Digest32, String> {
    let decoded = hex::decode(value).map_err(|_| format!("{label} must be hexadecimal"))?;
    decoded
        .try_into()
        .map_err(|bytes: Vec<u8>| format!("{label} must be 32 bytes; got {}", bytes.len()))
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid JSON in {}: {error}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    create_parent(path)?;
    let mut file = File::create(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    file.write_all(b"\n")
        .map_err(|error| format!("failed to finish {}: {error}", path.display()))
}

fn write_private_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    create_parent(path)?;
    let temporary = path.with_extension(format!("tmp-{}", process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to restrict {}: {error}", temporary.display()))?;
    }
    serde_json::to_writer_pretty(&mut file, value)
        .map_err(|error| format!("failed to serialize {}: {error}", temporary.display()))?;
    file.write_all(b"\n")
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("failed to finish {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| {
        format!(
            "failed to replace {} with {}: {error}",
            path.display(),
            temporary.display()
        )
    })
}

fn lock_state(path: &Path) -> Result<File, String> {
    create_parent(path)?;
    let lock_path = path.with_extension("lock");
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let lock = options
        .open(&lock_path)
        .map_err(|error| format!("failed to open {}: {error}", lock_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        lock.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to restrict {}: {error}", lock_path.display()))?;
    }
    lock.lock_exclusive()
        .map_err(|error| format!("failed to lock {}: {error}", path.display()))?;
    Ok(lock)
}

fn create_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    #[test]
    fn lock_state_restricts_existing_lock_file() {
        let directory = std::env::temp_dir().join(format!(
            "proofgate-governance-lock-{}-{}",
            process::id(),
            now_unix_ms().expect("current time")
        ));
        fs::create_dir(&directory).expect("create test directory");
        let state_path = directory.join("state.json");
        let lock_path = state_path.with_extension("lock");
        File::create(&lock_path).expect("create permissive lock file");
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o666))
            .expect("make test lock file permissive");

        let lock = lock_state(&state_path).expect("lock state");
        let mode = fs::metadata(&lock_path)
            .expect("read lock metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        drop(lock);
        fs::remove_file(lock_path).expect("remove test lock file");
        fs::remove_dir(directory).expect("remove test directory");
    }
}
