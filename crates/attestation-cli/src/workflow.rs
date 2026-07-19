use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::Path,
    process,
};

use attestation_prover::{read_proof, InputError};
use attestation_types::{
    digest_from_hex, digest_to_hex, serde_digest_hex, AttestationEnvelope, Digest32,
    ProofTransport, VerificationChallenge,
};
use attestation_verifier::{
    create_challenge, present, verify, InMemoryReplayCache, VerificationError, VerifiedAttestation,
    VerifyRequest,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokenstudio_config::read_gate_config;

const PRESENTER_KEY_VERSION: u16 = 1;
const REPLAY_CACHE_VERSION: u16 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct PresenterKeyFile {
    version: u16,
    #[serde(with = "serde_digest_hex")]
    secret_key: Digest32,
    #[serde(with = "serde_digest_hex")]
    public_key: Digest32,
}

impl PresenterKeyFile {
    fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self {
            version: PRESENTER_KEY_VERSION,
            secret_key: signing_key.to_bytes(),
            public_key: signing_key.verifying_key().to_bytes(),
        }
    }

    fn signing_key(&self) -> Result<SigningKey, String> {
        if self.version != PRESENTER_KEY_VERSION {
            return Err(format!(
                "unsupported presenter key version {}; expected {PRESENTER_KEY_VERSION}",
                self.version
            ));
        }
        let signing_key = SigningKey::from_bytes(&self.secret_key);
        if signing_key.verifying_key().to_bytes() != self.public_key {
            return Err("presenter key file has mismatched public and secret keys".to_owned());
        }
        Ok(signing_key)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ReplayCacheFile {
    version: u16,
    consumed_challenges: Vec<String>,
}

pub struct VerifyOutcome {
    pub verified: VerifiedAttestation,
    pub consumed_challenges: usize,
}

pub fn generate_presenter_key(output: &Path) -> Result<Digest32, String> {
    let key = PresenterKeyFile::generate();
    write_private_json_new(output, &key)?;
    Ok(key.public_key)
}

pub fn presenter_public_key(path: &Path) -> Result<Digest32, String> {
    let key = read_presenter_key(path)?;
    key.signing_key()?;
    Ok(key.public_key)
}

pub fn issue_challenge(
    gate_path: &Path,
    verifier_id: Option<&str>,
    now_unix_ms: u64,
    ttl_ms: u64,
    output: &Path,
) -> Result<VerificationChallenge, String> {
    let gate = read_gate_config(gate_path).map_err(|error| error.to_string())?;
    let verifier_id = verifier_id.unwrap_or(&gate.context.verifier_id);
    let challenge =
        create_challenge(&gate, verifier_id, now_unix_ms, ttl_ms).map_err(verification_message)?;
    write_json(output, &challenge)?;
    Ok(challenge)
}

pub fn create_presentation(
    proof_path: &Path,
    challenge_path: &Path,
    presenter_key_path: &Path,
    transport: ProofTransport,
    output: &Path,
) -> Result<AttestationEnvelope, String> {
    let proof = read_proof(proof_path).map_err(input_message)?;
    let challenge: VerificationChallenge = read_json(challenge_path)?;
    let key = read_presenter_key(presenter_key_path)?;
    let signing_key = key.signing_key()?;
    let envelope =
        present(&proof, challenge, &signing_key, transport).map_err(verification_message)?;
    write_json(output, &envelope)?;
    Ok(envelope)
}

pub fn verify_presentation(
    gate_path: &Path,
    envelope_path: &Path,
    replay_cache_path: &Path,
    expected_verifier_id: Option<&str>,
    now_unix_ms: u64,
) -> Result<VerifyOutcome, String> {
    let gate = read_gate_config(gate_path).map_err(|error| error.to_string())?;
    let envelope: AttestationEnvelope = read_json(envelope_path)?;
    let verifier_id = expected_verifier_id.unwrap_or(&gate.context.verifier_id);
    let mut replay_cache = load_replay_cache(replay_cache_path)?;
    let verified = verify(
        VerifyRequest {
            envelope: &envelope,
            expected_gate: &gate,
            expected_verifier_id: verifier_id,
            now_unix_ms,
        },
        &mut replay_cache,
    )
    .map_err(verification_message)?;
    save_replay_cache(replay_cache_path, &replay_cache)?;

    Ok(VerifyOutcome {
        verified,
        consumed_challenges: replay_cache.len(),
    })
}

fn read_presenter_key(path: &Path) -> Result<PresenterKeyFile, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let permissions = fs::metadata(path)
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?
            .permissions()
            .mode();
        if permissions & 0o077 != 0 {
            return Err(format!(
                "presenter key {} is accessible by group or other users; run chmod 600",
                path.display()
            ));
        }
    }
    read_json(path)
}

fn load_replay_cache(path: &Path) -> Result<InMemoryReplayCache, String> {
    if !path.exists() {
        return Ok(InMemoryReplayCache::default());
    }
    let stored: ReplayCacheFile = read_json(path)?;
    if stored.version != REPLAY_CACHE_VERSION {
        return Err(format!(
            "unsupported replay cache version {}; expected {REPLAY_CACHE_VERSION}",
            stored.version
        ));
    }
    let consumed = stored
        .consumed_challenges
        .iter()
        .map(|digest| digest_from_hex(digest).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InMemoryReplayCache::from_consumed(consumed))
}

fn save_replay_cache(path: &Path, cache: &InMemoryReplayCache) -> Result<(), String> {
    let mut consumed_challenges = cache.consumed().map(digest_to_hex).collect::<Vec<_>>();
    consumed_challenges.sort_unstable();
    let stored = ReplayCacheFile {
        version: REPLAY_CACHE_VERSION,
        consumed_challenges,
    };
    write_json_atomic(path, &stored)
}

fn verification_message(error: VerificationError) -> String {
    format!("verification denied [{}]: {error}", error.code() as u16)
}

fn input_message(error: InputError) -> String {
    error.to_string()
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

fn write_private_json_new<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    create_parent(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        format!(
            "failed to create new presenter key {}: {error}",
            path.display()
        )
    })?;
    serde_json::to_writer_pretty(&mut file, value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    file.write_all(b"\n")
        .map_err(|error| format!("failed to finish {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("failed to sync {}: {error}", path.display()))
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    create_parent(path)?;
    let temporary = path.with_extension(format!("tmp-{}", process::id()));
    write_json(&temporary, value)?;
    fs::rename(&temporary, path).map_err(|error| {
        format!(
            "failed to replace replay cache {} with {}: {error}",
            path.display(),
            temporary.display()
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "proofgate-{name}-{}-{}.json",
            process::id(),
            rand::random::<u64>()
        ))
    }

    #[test]
    fn generated_key_round_trips_without_exposing_secret() {
        let path = temporary_path("presenter-key");
        let public_key = generate_presenter_key(&path).unwrap();

        assert_eq!(presenter_public_key(&path).unwrap(), public_key);
        assert!(generate_presenter_key(&path).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn replay_cache_round_trips_sorted_digests() {
        let path = temporary_path("replay-cache");
        let cache = InMemoryReplayCache::from_consumed([[9; 32], [3; 32]]);
        save_replay_cache(&path, &cache).unwrap();
        let loaded = load_replay_cache(&path).unwrap();

        assert_eq!(loaded.len(), 2);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.find(&"03".repeat(32)).unwrap() < text.find(&"09".repeat(32)).unwrap());
        fs::remove_file(path).unwrap();
    }
}
