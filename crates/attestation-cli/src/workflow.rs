use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::Path,
    process,
    str::FromStr as _,
    time::Instant,
};

use attestation_prover::{read_proof, InputError};
use attestation_types::{
    digest_from_hex, digest_to_hex, program_owner_to_hex, serde_digest_hex, AttestationEnvelope,
    Digest32, ProofTransport, VerificationChallenge,
};
use attestation_verifier::{
    create_challenge, present, verify, InMemoryReplayCache, VerificationError, VerifiedAttestation,
    VerifyRequest,
};
use balance_gate_core::{
    decode_access_badge, decode_gate_state, on_chain_presentation_message, AccessBadge,
    ClaimAccess, GateState, OnChainAttestationJournal, OnChainChallenge, ACCESS_BADGE_VERSION,
};
use balance_gate_methods::BALANCE_GATE_ID;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use ed25519_dalek::{Signer as _, SigningKey};
use fs2::FileExt as _;
use jsonrpsee::{core::client::ClientT as _, http_client::HttpClientBuilder, rpc_params};
use lee::{AccountId, PrivateKey};
use lee_core::{
    account::{Account, AccountWithMetadata},
    encryption::ViewingPublicKey,
    program::DEFAULT_PROGRAM_ID,
    InputAccountIdentity,
};
use lez_gate_sdk::{
    compose_demo_gate_execution, compose_private_execution, gate_deployment_transaction,
    gate_initialization_transaction, private_badge_identity, private_gate_claim_transaction,
    prove_demo_gate_claim, prove_gate_claim, signer_account_id, simulate_demo_claim,
    PrivateBadgeIdentity, SimulatedGateClaim,
};
use proofgate_messaging::bind_admission_challenge;
use rand::{rngs::OsRng, RngCore as _};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokenstudio_config::read_gate_config;

const PRESENTER_KEY_VERSION: u16 = 1;
const LEZ_ACCOUNT_KEY_VERSION: u16 = 1;
const LEZ_PRIVATE_ACCOUNT_KEY_VERSION: u16 = 1;
const REPLAY_CACHE_VERSION: u16 = 1;
const LEZ_TX_PUBLIC_TAG: u8 = 0;
const LEZ_TX_PRIVATE_TAG: u8 = 1;
const LEZ_TX_DEPLOYMENT_TAG: u8 = 2;

#[derive(Debug, Serialize, Deserialize)]
struct PresenterKeyFile {
    version: u16,
    #[serde(with = "serde_digest_hex")]
    secret_key: Digest32,
    #[serde(with = "serde_digest_hex")]
    public_key: Digest32,
}

#[derive(Debug, Serialize, Deserialize)]
struct LezAccountKeyFile {
    version: u16,
    private_key_hex: String,
    account_id: String,
}

impl LezAccountKeyFile {
    fn generate() -> Self {
        let private_key = PrivateKey::new_os_random();
        let account_id = signer_account_id(&private_key);
        Self {
            version: LEZ_ACCOUNT_KEY_VERSION,
            private_key_hex: private_key.to_string(),
            account_id: account_id.to_string(),
        }
    }

    fn private_key(&self) -> Result<PrivateKey, String> {
        if self.version != LEZ_ACCOUNT_KEY_VERSION {
            return Err(format!(
                "unsupported LEZ account key version {}; expected {LEZ_ACCOUNT_KEY_VERSION}",
                self.version
            ));
        }
        let private_key = PrivateKey::from_str(&self.private_key_hex)
            .map_err(|error| format!("invalid LEZ private key: {error}"))?;
        let account_id = signer_account_id(&private_key);
        if account_id.to_string() != self.account_id {
            return Err("LEZ account key file has a mismatched account ID".to_owned());
        }
        Ok(private_key)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LezPrivateAccountKeyFile {
    version: u16,
    #[serde(with = "serde_digest_hex")]
    nullifier_secret_key: Digest32,
    #[serde(with = "serde_digest_hex")]
    viewing_secret_d: Digest32,
    #[serde(with = "serde_digest_hex")]
    viewing_secret_z: Digest32,
    identifier: u128,
    account_id: String,
}

impl LezPrivateAccountKeyFile {
    fn generate() -> Self {
        let mut nullifier_secret_key = [0_u8; 32];
        let mut viewing_secret_d = [0_u8; 32];
        let mut viewing_secret_z = [0_u8; 32];
        OsRng.fill_bytes(&mut nullifier_secret_key);
        OsRng.fill_bytes(&mut viewing_secret_d);
        OsRng.fill_bytes(&mut viewing_secret_z);
        let identifier = 0;
        let viewing_public_key = ViewingPublicKey::from_seed(&viewing_secret_d, &viewing_secret_z);
        let account_id =
            private_badge_identity(nullifier_secret_key, &viewing_public_key, identifier)
                .account_id;
        Self {
            version: LEZ_PRIVATE_ACCOUNT_KEY_VERSION,
            nullifier_secret_key,
            viewing_secret_d,
            viewing_secret_z,
            identifier,
            account_id: account_id.to_string(),
        }
    }

    fn identity(&self) -> Result<PrivateBadgeIdentity, String> {
        if self.version != LEZ_PRIVATE_ACCOUNT_KEY_VERSION {
            return Err(format!(
                "unsupported LEZ private account key version {}; expected {LEZ_PRIVATE_ACCOUNT_KEY_VERSION}",
                self.version
            ));
        }
        let viewing_public_key =
            ViewingPublicKey::from_seed(&self.viewing_secret_d, &self.viewing_secret_z);
        let identity = private_badge_identity(
            self.nullifier_secret_key,
            &viewing_public_key,
            self.identifier,
        );
        if identity.account_id.to_string() != self.account_id {
            return Err("LEZ private account key file has a mismatched account ID".to_owned());
        }
        Ok(identity)
    }
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

pub struct ComposedOnChainOutcome {
    pub proof_bytes: usize,
    pub public_post_states: usize,
    pub encrypted_private_post_states: usize,
    pub private_commitments: usize,
    pub nullifiers: usize,
    pub gate_proving_ms: u128,
    pub outer_proving_ms: u128,
    pub total_proving_ms: u128,
    pub gate_total_cycles: u64,
    pub gate_user_cycles: u64,
    pub gate_paging_cycles: u64,
    pub gate_segments: usize,
    pub outer_total_cycles: u64,
    pub outer_user_cycles: u64,
    pub outer_paging_cycles: u64,
    pub outer_segments: usize,
}

pub struct SubmittedOnChainClaim {
    pub transaction_hash: String,
    pub composition: ComposedOnChainOutcome,
}

pub struct ClaimOutputPaths<'path> {
    pub badge: &'path Path,
    pub proof: &'path Path,
}

#[must_use]
pub const fn balance_gate_program_id() -> [u32; 8] {
    BALANCE_GATE_ID
}

pub fn generate_lez_account_key(output: &Path) -> Result<AccountId, String> {
    let key = LezAccountKeyFile::generate();
    let private_key = key.private_key()?;
    let account_id = signer_account_id(&private_key);
    write_private_json_new(output, &key)?;
    Ok(account_id)
}

pub fn lez_account_id(path: &Path) -> Result<AccountId, String> {
    let key = read_lez_account_key(path)?;
    Ok(signer_account_id(&key.private_key()?))
}

pub fn generate_lez_private_account_key(output: &Path) -> Result<AccountId, String> {
    let key = LezPrivateAccountKeyFile::generate();
    let account_id = key.identity()?.account_id;
    write_private_json_new(output, &key)?;
    Ok(account_id)
}

pub fn lez_private_account_id(path: &Path) -> Result<AccountId, String> {
    Ok(read_lez_private_account_key(path)?.identity()?.account_id)
}

pub async fn deploy_balance_gate(sequencer_url: &str) -> Result<String, String> {
    send_lez_transaction(
        sequencer_url,
        LEZ_TX_DEPLOYMENT_TAG,
        &gate_deployment_transaction(),
    )
    .await
}

pub async fn fetch_on_chain_state(
    sequencer_url: &str,
    gate_account_id: Digest32,
    output: &Path,
) -> Result<GateState, String> {
    let account_id = AccountId::new(gate_account_id);
    let account = fetch_public_account(sequencer_url, account_id).await?;
    let state = validated_gate_state(&account, account_id)?;
    write_json(output, &state)?;
    Ok(state)
}

pub async fn fetch_on_chain_badge(
    sequencer_url: &str,
    badge_account_id: Digest32,
    output: &Path,
) -> Result<AccessBadge, String> {
    let account_id = AccountId::new(badge_account_id);
    let account = fetch_public_account(sequencer_url, account_id).await?;
    let badge = validated_access_badge(&account, account_id)?;
    write_json(output, &badge)?;
    Ok(badge)
}

pub async fn submit_on_chain_initialization(
    sequencer_url: &str,
    state_path: &Path,
    gate_key_path: &Path,
) -> Result<(String, AccountId), String> {
    let state: GateState = read_json(state_path)?;
    let key_file = read_lez_account_key(gate_key_path)?;
    let private_key = key_file.private_key()?;
    let gate_account_id = signer_account_id(&private_key);
    let account = fetch_public_account(sequencer_url, gate_account_id).await?;
    if account.program_owner != DEFAULT_PROGRAM_ID || !account.data.as_ref().is_empty() {
        return Err(format!(
            "gate account {gate_account_id} is already initialized"
        ));
    }
    let transaction =
        gate_initialization_transaction(&state, gate_account_id, account.nonce, &private_key)
            .map_err(|error| error.to_string())?;
    let transaction_hash =
        send_lez_transaction(sequencer_url, LEZ_TX_PUBLIC_TAG, &transaction).await?;
    Ok((transaction_hash, gate_account_id))
}

pub async fn submit_on_chain_claim(
    sequencer_url: &str,
    proof_path: &Path,
    state_path: &Path,
    claim_path: &Path,
    gate_account_id: Digest32,
    badge_key_path: &Path,
    output: ClaimOutputPaths<'_>,
) -> Result<SubmittedOnChainClaim, String> {
    let proof = read_proof(proof_path).map_err(input_message)?;
    let expected_state: GateState = read_json(state_path)?;
    let claim: ClaimAccess = read_json(claim_path)?;
    let gate_account_id = AccountId::new(gate_account_id);
    let gate_account = fetch_public_account(sequencer_url, gate_account_id).await?;
    let current_state = validated_gate_state(&gate_account, gate_account_id)?;
    if current_state != expected_state {
        return Err(
            "on-chain gate state changed after the challenge was issued; fetch state and create a new claim"
                .to_owned(),
        );
    }

    let badge_identity = read_lez_private_account_key(badge_key_path)?.identity()?;
    let badge_account_id = badge_identity.account_id;
    let pre_states = vec![
        AccountWithMetadata::new(gate_account, false, gate_account_id),
        AccountWithMetadata::new(Account::default(), true, badge_account_id),
    ];
    let total_started = Instant::now();
    eprintln!("Composition phase 1/2: proving the balance-gate program receipt");
    let phase_started = Instant::now();
    let gate_execution =
        prove_gate_claim(pre_states, claim, &proof).map_err(|error| error.to_string())?;
    let badge = decode_access_badge(
        gate_execution
            .output
            .post_states
            .get(1)
            .ok_or_else(|| "gate execution did not return an access badge".to_owned())?
            .account()
            .data
            .as_ref(),
    )
    .map_err(|error| format!("gate execution returned an invalid access badge: {error}"))?;
    let gate_proving_ms = phase_started.elapsed().as_millis();
    eprintln!("Composition phase 1/2 complete in {gate_proving_ms} ms");

    eprintln!("Composition phase 2/2: proving the official LEZ PPE receipt");
    let phase_started = Instant::now();
    let composed = compose_private_execution(
        gate_execution,
        vec![
            InputAccountIdentity::Public,
            badge_identity.circuit_identity,
        ],
    )
    .map_err(|error| error.to_string())?;
    let outer_proving_ms = phase_started.elapsed().as_millis();
    let total_proving_ms = total_started.elapsed().as_millis();
    eprintln!("Composition phase 2/2 complete in {outer_proving_ms} ms");

    let public_post_states = composed.circuit_output.public_post_states.len();
    let private_commitments = composed.circuit_output.new_commitments.len();
    let nullifiers = composed.circuit_output.new_nullifiers.len();
    let encrypted_private_post_states = composed.circuit_output.encrypted_private_post_states.len();
    if public_post_states != 1
        || private_commitments != 1
        || nullifiers != 1
        || encrypted_private_post_states != 1
    {
        return Err(format!(
            "invalid private badge transaction shape: {public_post_states} public post states, \
             {private_commitments} commitments, {nullifiers} nullifiers, and \
             {encrypted_private_post_states} encrypted private post states",
        ));
    }
    write_private_json(output.badge, &badge)?;
    let proof_bytes = composed.proof.clone().into_inner();
    let gate_stats = composed.gate_stats.clone();
    let outer_stats = composed.outer_stats.clone();
    create_parent(output.proof)?;
    fs::write(output.proof, &proof_bytes)
        .map_err(|error| format!("failed to write {}: {error}", output.proof.display()))?;
    let transaction = private_gate_claim_transaction(composed, gate_account_id)
        .map_err(|error| error.to_string())?;
    let transaction_hash =
        send_lez_transaction(sequencer_url, LEZ_TX_PRIVATE_TAG, &transaction).await?;

    Ok(SubmittedOnChainClaim {
        transaction_hash,
        composition: ComposedOnChainOutcome {
            proof_bytes: proof_bytes.len(),
            public_post_states,
            encrypted_private_post_states,
            private_commitments,
            nullifiers,
            gate_proving_ms,
            outer_proving_ms,
            total_proving_ms,
            gate_total_cycles: gate_stats.total_cycles,
            gate_user_cycles: gate_stats.user_cycles,
            gate_paging_cycles: gate_stats.paging_cycles,
            gate_segments: gate_stats.segments,
            outer_total_cycles: outer_stats.total_cycles,
            outer_user_cycles: outer_stats.user_cycles,
            outer_paging_cycles: outer_stats.paging_cycles,
            outer_segments: outer_stats.segments,
        },
    })
}

pub fn initialize_on_chain_state(
    gate_path: &Path,
    commitment_root: Digest32,
    challenge_nonce: Digest32,
    max_proof_age_ms: u64,
    output: &Path,
) -> Result<GateState, String> {
    let gate = read_gate_config(gate_path).map_err(|error| error.to_string())?;
    let state = GateState::new_with_max_proof_age(
        &gate.context,
        commitment_root,
        challenge_nonce,
        max_proof_age_ms,
    );
    state.validate().map_err(|error| error.to_string())?;
    write_json(output, &state)?;
    Ok(state)
}

pub fn current_on_chain_challenge(
    state_path: &Path,
    claim_account_id: Digest32,
) -> Result<OnChainChallenge, String> {
    let state: GateState = read_json(state_path)?;
    state.validate().map_err(|error| error.to_string())?;
    Ok(state.challenge(BALANCE_GATE_ID, claim_account_id))
}

pub fn create_on_chain_claim(
    proof_path: &Path,
    state_path: &Path,
    claim_account_id: Digest32,
    presenter_key_path: &Path,
    output: &Path,
) -> Result<ClaimAccess, String> {
    let proof = read_proof(proof_path).map_err(input_message)?;
    let state: GateState = read_json(state_path)?;
    state.validate().map_err(|error| error.to_string())?;
    if proof.journal.commitment_root != state.commitment_root {
        return Err("proof commitment root is not authorized by the on-chain gate".to_owned());
    }
    let key = read_presenter_key(presenter_key_path)?;
    let signing_key = key.signing_key()?;
    if signing_key.verifying_key().to_bytes() != proof.journal.presenter_public_key {
        return Err(
            "presenter key does not match the public key committed in the proof".to_owned(),
        );
    }
    let journal = OnChainAttestationJournal::from(&proof.journal);
    let challenge = state.challenge(BALANCE_GATE_ID, claim_account_id);
    let signature = signing_key.sign(&on_chain_presentation_message(&challenge, &journal));
    let claim = ClaimAccess {
        journal,
        presenter_signature: signature.to_bytes().to_vec(),
    };
    write_json(output, &claim)?;
    Ok(claim)
}

pub fn simulate_on_chain_claim(
    state_path: &Path,
    claim_path: &Path,
    gate_account_id: Digest32,
    badge_account_id: Digest32,
) -> Result<SimulatedGateClaim, String> {
    let state: GateState = read_json(state_path)?;
    let claim: ClaimAccess = read_json(claim_path)?;
    simulate_demo_claim(&state, gate_account_id, badge_account_id, claim)
        .map_err(|error| error.to_string())
}

pub fn compose_on_chain_claim(
    proof_path: &Path,
    state_path: &Path,
    claim_path: &Path,
    gate_account_id: Digest32,
    badge_key_path: &Path,
    output_badge: &Path,
    output: &Path,
) -> Result<ComposedOnChainOutcome, String> {
    let proof = read_proof(proof_path).map_err(input_message)?;
    let state: GateState = read_json(state_path)?;
    let claim: ClaimAccess = read_json(claim_path)?;
    let badge_identity = read_lez_private_account_key(badge_key_path)?.identity()?;
    let badge_account_id = badge_identity.account_id.into_value();
    let total_started = Instant::now();
    eprintln!("Composition phase 1/2: proving the balance-gate program receipt");
    let phase_started = Instant::now();
    let gate_execution =
        prove_demo_gate_claim(&state, gate_account_id, badge_account_id, claim, &proof)
            .map_err(|error| error.to_string())?;
    let badge = decode_access_badge(
        gate_execution
            .output
            .post_states
            .get(1)
            .ok_or_else(|| "gate execution did not return an access badge".to_owned())?
            .account()
            .data
            .as_ref(),
    )
    .map_err(|error| format!("gate execution returned an invalid access badge: {error}"))?;
    let gate_proving_ms = phase_started.elapsed().as_millis();
    eprintln!("Composition phase 1/2 complete in {gate_proving_ms} ms");
    eprintln!("Composition phase 2/2: proving the official LEZ PPE receipt");
    let phase_started = Instant::now();
    let composed = compose_demo_gate_execution(gate_execution, badge_identity.circuit_identity)
        .map_err(|error| error.to_string())?;
    let outer_proving_ms = phase_started.elapsed().as_millis();
    let total_proving_ms = total_started.elapsed().as_millis();
    eprintln!("Composition phase 2/2 complete in {outer_proving_ms} ms");
    let public_post_states = composed.circuit_output.public_post_states.len();
    let private_commitments = composed.circuit_output.new_commitments.len();
    let nullifiers = composed.circuit_output.new_nullifiers.len();
    let encrypted_private_post_states = composed.circuit_output.encrypted_private_post_states.len();
    if public_post_states != 1
        || private_commitments != 1
        || nullifiers != 1
        || encrypted_private_post_states != 1
    {
        return Err(format!(
            "invalid private badge transaction shape: {public_post_states} public post states, \
             {private_commitments} commitments, {nullifiers} nullifiers, and \
             {encrypted_private_post_states} encrypted private post states",
        ));
    }
    write_private_json(output_badge, &badge)?;
    let gate_stats = composed.gate_stats;
    let outer_stats = composed.outer_stats;
    let proof_bytes = composed.proof.into_inner();
    create_parent(output)?;
    fs::write(output, &proof_bytes)
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(ComposedOnChainOutcome {
        proof_bytes: proof_bytes.len(),
        public_post_states,
        encrypted_private_post_states,
        private_commitments,
        nullifiers,
        gate_proving_ms,
        outer_proving_ms,
        total_proving_ms,
        gate_total_cycles: gate_stats.total_cycles,
        gate_user_cycles: gate_stats.user_cycles,
        gate_paging_cycles: gate_stats.paging_cycles,
        gate_segments: gate_stats.segments,
        outer_total_cycles: outer_stats.total_cycles,
        outer_user_cycles: outer_stats.user_cycles,
        outer_paging_cycles: outer_stats.paging_cycles,
        outer_segments: outer_stats.segments,
    })
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
    expected_commitment_root: Digest32,
    verifier_id: Option<&str>,
    now_unix_ms: u64,
    ttl_ms: u64,
    output: &Path,
) -> Result<VerificationChallenge, String> {
    let gate = read_gate_config(gate_path).map_err(|error| error.to_string())?;
    let verifier_id = verifier_id.unwrap_or(&gate.context.verifier_id);
    let challenge = create_challenge(
        &gate,
        expected_commitment_root,
        verifier_id,
        now_unix_ms,
        ttl_ms,
    )
    .map_err(verification_message)?;
    write_json(output, &challenge)?;
    Ok(challenge)
}

pub fn issue_admission_challenge(
    gate_path: &Path,
    expected_commitment_root: Digest32,
    group_id: &str,
    member_address: &str,
    now_unix_ms: u64,
    ttl_ms: u64,
    output: &Path,
) -> Result<VerificationChallenge, String> {
    let gate = read_gate_config(gate_path).map_err(|error| error.to_string())?;
    let challenge = create_challenge(
        &gate,
        expected_commitment_root,
        &gate.context.verifier_id,
        now_unix_ms,
        ttl_ms,
    )
    .map_err(verification_message)?;
    let challenge = bind_admission_challenge(challenge, group_id, member_address);
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

pub fn read_verification_challenge(path: &Path) -> Result<VerificationChallenge, String> {
    read_json(path)
}

pub fn write_verification_challenge(
    path: &Path,
    challenge: &VerificationChallenge,
) -> Result<(), String> {
    write_json(path, challenge)
}

pub fn read_presentation(path: &Path) -> Result<AttestationEnvelope, String> {
    read_json(path)
}

pub fn write_presentation(path: &Path, envelope: &AttestationEnvelope) -> Result<(), String> {
    write_json(path, envelope)
}

pub fn verify_presentation(
    gate_path: &Path,
    envelope_path: &Path,
    expected_challenge_path: &Path,
    replay_cache_path: &Path,
    expected_verifier_id: Option<&str>,
    now_unix_ms: u64,
) -> Result<VerifyOutcome, String> {
    let envelope: AttestationEnvelope = read_json(envelope_path)?;
    let expected_challenge: VerificationChallenge = read_json(expected_challenge_path)?;
    verify_received_presentation(
        gate_path,
        &envelope,
        &expected_challenge,
        replay_cache_path,
        expected_verifier_id,
        now_unix_ms,
    )
}

pub fn verify_received_presentation(
    gate_path: &Path,
    envelope: &AttestationEnvelope,
    expected_challenge: &VerificationChallenge,
    replay_cache_path: &Path,
    expected_verifier_id: Option<&str>,
    now_unix_ms: u64,
) -> Result<VerifyOutcome, String> {
    let gate = read_gate_config(gate_path).map_err(|error| error.to_string())?;
    let verifier_id = expected_verifier_id.unwrap_or(&gate.context.verifier_id);
    let _replay_lock = lock_replay_cache(replay_cache_path)?;
    let mut replay_cache = load_replay_cache(replay_cache_path)?;
    let verified = verify(
        VerifyRequest {
            envelope,
            expected_challenge,
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

fn lock_replay_cache(path: &Path) -> Result<File, String> {
    create_parent(path)?;
    let lock_path = path.with_extension("lock");
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "failed to open replay lock {}: {error}",
                lock_path.display()
            )
        })?;
    lock.lock_exclusive()
        .map_err(|error| format!("failed to lock replay cache {}: {error}", path.display()))?;
    Ok(lock)
}

fn read_presenter_key(path: &Path) -> Result<PresenterKeyFile, String> {
    ensure_private_file(path, "presenter key")?;
    read_json(path)
}

fn read_lez_account_key(path: &Path) -> Result<LezAccountKeyFile, String> {
    ensure_private_file(path, "LEZ account key")?;
    read_json(path)
}

fn read_lez_private_account_key(path: &Path) -> Result<LezPrivateAccountKeyFile, String> {
    ensure_private_file(path, "LEZ private account key")?;
    read_json(path)
}

fn ensure_private_file(path: &Path, label: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let permissions = fs::metadata(path)
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?
            .permissions()
            .mode();
        if permissions & 0o077 != 0 {
            return Err(format!(
                "{label} {} is accessible by group or other users; run chmod 600",
                path.display()
            ));
        }
    }
    Ok(())
}

async fn fetch_public_account(
    sequencer_url: &str,
    account_id: AccountId,
) -> Result<Account, String> {
    let client = HttpClientBuilder::default()
        .build(sequencer_url)
        .map_err(|error| format!("failed to create sequencer RPC client: {error}"))?;
    client
        .request("getAccount", rpc_params![account_id.to_string()])
        .await
        .map_err(|error| format!("sequencer getAccount request failed: {error}"))
}

fn validated_gate_state(account: &Account, account_id: AccountId) -> Result<GateState, String> {
    if account.program_owner != BALANCE_GATE_ID {
        return Err(format!(
            "account {account_id} is not owned by balance_gate {}",
            program_owner_to_hex(&balance_gate_program_id())
        ));
    }
    let state = decode_gate_state(account.data.as_ref())
        .map_err(|error| format!("account {account_id} has invalid gate state: {error}"))?;
    state.validate().map_err(|error| error.to_string())?;
    Ok(state)
}

fn validated_access_badge(account: &Account, account_id: AccountId) -> Result<AccessBadge, String> {
    if account.program_owner != BALANCE_GATE_ID {
        return Err(format!(
            "account {account_id} is not owned by balance_gate {}",
            program_owner_to_hex(&balance_gate_program_id())
        ));
    }
    let badge = decode_access_badge(account.data.as_ref())
        .map_err(|error| format!("account {account_id} has invalid access badge: {error}"))?;
    if badge.version != ACCESS_BADGE_VERSION {
        return Err(format!(
            "account {account_id} has unsupported access badge version {}",
            badge.version
        ));
    }
    Ok(badge)
}

async fn send_lez_transaction<T: borsh::BorshSerialize>(
    sequencer_url: &str,
    variant_tag: u8,
    transaction: &T,
) -> Result<String, String> {
    let bytes = encode_lez_transaction(variant_tag, transaction)?;
    let payload = BASE64_STANDARD.encode(bytes);
    let client = HttpClientBuilder::default()
        .build(sequencer_url)
        .map_err(|error| format!("failed to create sequencer RPC client: {error}"))?;
    client
        .request("sendTransaction", rpc_params![payload])
        .await
        .map_err(|error| format!("sequencer sendTransaction request failed: {error}"))
}

fn encode_lez_transaction<T: borsh::BorshSerialize>(
    variant_tag: u8,
    transaction: &T,
) -> Result<Vec<u8>, String> {
    let mut bytes = vec![variant_tag];
    borsh::BorshSerialize::serialize(transaction, &mut bytes)
        .map_err(|error| format!("failed to encode LEZ transaction: {error}"))?;
    Ok(bytes)
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

fn write_private_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    create_parent(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                format!(
                    "failed to restrict permissions on {}: {error}",
                    path.display()
                )
            })?;
    }
    serde_json::to_writer_pretty(&mut file, value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    file.write_all(b"\n")
        .map_err(|error| format!("failed to finish {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("failed to sync {}: {error}", path.display()))
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
            "failed to create new private key {}: {error}",
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
    use balance_gate_core::encode_access_badge;

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
    fn generated_lez_account_key_round_trips_with_restricted_permissions() {
        let path = temporary_path("lez-account-key");
        let account_id = generate_lez_account_key(&path).unwrap();

        assert_eq!(lez_account_id(&path).unwrap(), account_id);
        assert!(generate_lez_account_key(&path).is_err());
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
    fn generated_private_account_key_round_trips_with_restricted_permissions() {
        let path = temporary_path("lez-private-account-key");
        let account_id = generate_lez_private_account_key(&path).unwrap();

        assert_eq!(lez_private_account_id(&path).unwrap(), account_id);
        assert!(generate_lez_private_account_key(&path).is_err());
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
    fn private_json_output_is_restricted_even_when_replacing_a_public_file() {
        let path = temporary_path("private-json-output");
        fs::write(&path, b"old").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }

        write_private_json(&path, &serde_json::json!({"claim_number": 1})).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap(),
            serde_json::json!({"claim_number": 1})
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn transaction_wire_tags_match_lez_v02_enum_order() {
        let deployment =
            encode_lez_transaction(LEZ_TX_DEPLOYMENT_TAG, &gate_deployment_transaction()).unwrap();
        assert_eq!(deployment[0], 2);
        assert_eq!(LEZ_TX_PUBLIC_TAG, 0);
        assert_eq!(LEZ_TX_PRIVATE_TAG, 1);
    }

    #[test]
    fn access_badge_requires_the_gate_owner_and_current_version() {
        let account_id = AccountId::new([7; 32]);
        let badge = AccessBadge {
            version: ACCESS_BADGE_VERSION,
            context_hash: [1; 32],
            presenter_public_key: [2; 32],
            claim_number: 3,
            proof_issued_at_unix_ms: 4,
        };
        let account = Account {
            program_owner: BALANCE_GATE_ID,
            data: encode_access_badge(&badge).unwrap().try_into().unwrap(),
            ..Account::default()
        };
        assert_eq!(validated_access_badge(&account, account_id).unwrap(), badge);

        let wrong_owner = Account {
            program_owner: DEFAULT_PROGRAM_ID,
            ..account.clone()
        };
        assert!(validated_access_badge(&wrong_owner, account_id).is_err());

        let unsupported = AccessBadge {
            version: ACCESS_BADGE_VERSION + 1,
            ..badge
        };
        let wrong_version = Account {
            data: encode_access_badge(&unsupported)
                .unwrap()
                .try_into()
                .unwrap(),
            ..account
        };
        assert!(validated_access_badge(&wrong_version, account_id).is_err());
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

    #[test]
    fn replay_cache_lock_serializes_verifiers() {
        use std::{sync::mpsc, thread, time::Duration};

        let path = temporary_path("replay-lock");
        let first = lock_replay_cache(&path).unwrap();
        let second_path = path.clone();
        let (sender, receiver) = mpsc::channel();
        let waiter = thread::spawn(move || {
            let _second = lock_replay_cache(&second_path).unwrap();
            sender.send(()).unwrap();
        });

        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        waiter.join().unwrap();
        fs::remove_file(path.with_extension("lock")).unwrap();
    }
}
