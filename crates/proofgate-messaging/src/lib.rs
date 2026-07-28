use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use attestation_types::{
    digest_from_hex, digest_to_hex, sha256, AttestationEnvelope, ProofTransport,
    VerificationChallenge,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::{rngs::OsRng, RngCore as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const PROTOCOL: &str = "proofgate/attestation-transfer";
pub const CHALLENGE_PROTOCOL: &str = "proofgate/verification-challenge";
pub const WIRE_VERSION: u16 = 1;
pub const DEFAULT_CHUNK_BYTES: usize = 32 * 1024;
pub const MAX_CHUNK_BYTES: usize = 48 * 1024;
pub const MAX_TRANSFER_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_CHUNKS: usize = 128;
const MAX_ENCODED_CHUNK_BYTES: usize = MAX_CHUNK_BYTES.div_ceil(3) * 4;
const MAX_PENDING_WIRE_MESSAGES: usize = (MAX_CHUNKS + 1) * 4;
const ADMISSION_CHALLENGE_DOMAIN: &[u8] = b"LEZ-TokenStudio/LogosChatAdmission/v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireMessage {
    pub protocol: String,
    pub version: u16,
    pub transfer_id: String,
    #[serde(flatten)]
    pub body: WireBody,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireBody {
    Manifest {
        envelope_sha256: String,
        envelope_bytes: u64,
        chunk_count: u32,
        context_hash: String,
        challenge_digest: String,
    },
    Chunk {
        chunk_index: u32,
        chunk_count: u32,
        data_base64: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferBundle {
    pub transfer_id: String,
    /// Chunks are sent first and the manifest last as the transfer commit.
    pub messages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct LogosChatMessage {
    pub from_self: bool,
    pub content: String,
    pub timestamp_ms: u64,
    #[serde(default)]
    pub sender: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeWireMessage {
    pub protocol: String,
    pub version: u16,
    pub challenge: VerificationChallenge,
}

#[derive(Debug, Error)]
pub enum MessagingError {
    #[error("attestation envelope must declare LogosMessaging transport")]
    WrongTransport,
    #[error("chunk size must be between 1 and {MAX_CHUNK_BYTES} bytes")]
    InvalidChunkSize,
    #[error("attestation envelope is {actual} bytes; maximum is {maximum}")]
    TransferTooLarge { actual: usize, maximum: usize },
    #[error("transfer requires {actual} chunks; maximum is {maximum}")]
    TooManyChunks { actual: usize, maximum: usize },
    #[error("invalid transfer id")]
    InvalidTransferId,
    #[error("failed to serialize Messaging payload: {0}")]
    Encoding(String),
    #[error("no ProofGate transfer manifest was found")]
    NoTransfer,
    #[error("no ProofGate verification challenge was found")]
    NoChallenge,
    #[error("transfer {transfer_id} is incomplete: received {received} of {expected} chunks")]
    IncompleteTransfer {
        transfer_id: String,
        received: usize,
        expected: usize,
    },
    #[error("transfer {0} contains conflicting manifests")]
    ConflictingManifest(String),
    #[error("transfer {0} contains conflicting duplicate chunks")]
    ConflictingChunk(String),
    #[error("invalid ProofGate wire message: {0}")]
    InvalidWireMessage(String),
    #[error("transfer integrity check failed")]
    IntegrityFailure,
    #[error("failed to execute Logos Core CLI {binary}: {message}")]
    LogosCoreExecution { binary: String, message: String },
    #[error("Logos Core call failed: {0}")]
    LogosCoreCall(String),
    #[error("invalid Logos Core response: {0}")]
    LogosCoreResponse(String),
    #[error("timed out waiting for a complete ProofGate transfer")]
    ReceiveTimeout,
}

#[derive(Clone, Debug)]
pub struct LogosCoreClient {
    binary: PathBuf,
    config_dir: PathBuf,
    module: String,
}

impl LogosCoreClient {
    #[must_use]
    pub fn new(binary: impl Into<PathBuf>, config_dir: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            config_dir: config_dir.into(),
            module: "chat_module".to_owned(),
        }
    }

    #[must_use]
    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = module.into();
        self
    }

    pub fn send_bundle(
        &self,
        conversation_id: &str,
        bundle: &TransferBundle,
    ) -> Result<(), MessagingError> {
        self.send_bundle_with_delay(conversation_id, bundle, Duration::ZERO)
    }

    pub fn send_bundle_with_delay(
        &self,
        conversation_id: &str,
        bundle: &TransferBundle,
        inter_message_delay: Duration,
    ) -> Result<(), MessagingError> {
        for (index, message) in bundle.messages.iter().enumerate() {
            let response = self.call("send_message", &[conversation_id, message])?;
            require_result_success(&response)?;
            if index + 1 < bundle.messages.len() && !inter_message_delay.is_zero() {
                thread::sleep(inter_message_delay);
            }
        }
        Ok(())
    }

    pub fn send_challenge(
        &self,
        conversation_id: &str,
        challenge: &VerificationChallenge,
    ) -> Result<(), MessagingError> {
        let content = encode_challenge(challenge)?;
        let response = self.call("send_message", &[conversation_id, &content])?;
        require_result_success(&response)
    }

    pub fn get_messages(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<LogosChatMessage>, MessagingError> {
        let response = self.call("get_messages", &[conversation_id])?;
        let result = response
            .get("result")
            .ok_or_else(|| MessagingError::LogosCoreResponse("missing result".to_owned()))?;
        serde_json::from_value(result.clone())
            .map_err(|error| MessagingError::LogosCoreResponse(error.to_string()))
    }

    pub fn add_group_member(
        &self,
        group_id: &str,
        member_address: &str,
    ) -> Result<(), MessagingError> {
        let response = self.call("add_group_member", &[group_id, member_address])?;
        require_result_success(&response)
    }

    fn call(&self, method: &str, arguments: &[&str]) -> Result<Value, MessagingError> {
        let output = Command::new(&self.binary)
            .arg("--config-dir")
            .arg(&self.config_dir)
            .arg("call")
            .arg(&self.module)
            .arg(method)
            .args(arguments)
            .output()
            .map_err(|error| MessagingError::LogosCoreExecution {
                binary: self.binary.display().to_string(),
                message: error.to_string(),
            })?;
        if !output.status.success() {
            return Err(MessagingError::LogosCoreCall(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let response: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| MessagingError::LogosCoreResponse(error.to_string()))?;
        if response.get("status").and_then(Value::as_str) != Some("ok") {
            return Err(MessagingError::LogosCoreCall(response.to_string()));
        }
        Ok(response)
    }
}

pub fn encode_challenge(challenge: &VerificationChallenge) -> Result<String, MessagingError> {
    serde_json::to_string(&ChallengeWireMessage {
        protocol: CHALLENGE_PROTOCOL.to_owned(),
        version: WIRE_VERSION,
        challenge: challenge.clone(),
    })
    .map_err(|error| MessagingError::Encoding(error.to_string()))
}

pub fn latest_challenge<I, S>(contents: I) -> Result<VerificationChallenge, MessagingError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    contents
        .into_iter()
        .filter_map(|content| serde_json::from_str::<ChallengeWireMessage>(content.as_ref()).ok())
        .filter(|message| {
            message.protocol == CHALLENGE_PROTOCOL
                && message.version == WIRE_VERSION
                && message.challenge.has_valid_window()
        })
        .map(|message| message.challenge)
        .max_by_key(|challenge| challenge.issued_at_unix_ms)
        .ok_or(MessagingError::NoChallenge)
}

pub fn receive_challenge(
    client: &LogosCoreClient,
    conversation_id: &str,
    expected_sender: Option<&str>,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<VerificationChallenge, MessagingError> {
    let started = Instant::now();
    loop {
        let messages = client.get_messages(conversation_id)?;
        let contents = messages
            .iter()
            .filter(|message| !message.from_self)
            .filter(|message| {
                expected_sender.is_none_or(|expected| message.sender.as_deref() == Some(expected))
            })
            .map(|message| message.content.as_str());
        match latest_challenge(contents) {
            Ok(challenge) => return Ok(challenge),
            Err(MessagingError::NoChallenge) => {}
            Err(error) => return Err(error),
        }
        if started.elapsed() >= timeout {
            return Err(MessagingError::ReceiveTimeout);
        }
        thread::sleep(poll_interval);
    }
}

#[must_use]
pub fn bind_admission_challenge(
    mut challenge: VerificationChallenge,
    group_id: &str,
    member_address: &str,
) -> VerificationChallenge {
    let random_prefix: [u8; 16] = challenge.nonce[..16]
        .try_into()
        .expect("challenge nonce is 32 bytes");
    let binding = admission_binding(&challenge, group_id, member_address, random_prefix);
    challenge.nonce[..16].copy_from_slice(&random_prefix);
    challenge.nonce[16..].copy_from_slice(&binding[..16]);
    challenge
}

#[must_use]
pub fn admission_challenge_matches(
    challenge: &VerificationChallenge,
    group_id: &str,
    member_address: &str,
) -> bool {
    let random_prefix: [u8; 16] = challenge.nonce[..16]
        .try_into()
        .expect("challenge nonce is 32 bytes");
    let binding = admission_binding(challenge, group_id, member_address, random_prefix);
    challenge.nonce[16..] == binding[..16]
}

pub fn pack_envelope(
    envelope: &AttestationEnvelope,
    chunk_bytes: usize,
) -> Result<TransferBundle, MessagingError> {
    let mut transfer_id = [0_u8; 16];
    OsRng.fill_bytes(&mut transfer_id);
    pack_envelope_with_id(envelope, chunk_bytes, &hex_bytes(&transfer_id))
}

pub fn pack_envelope_with_id(
    envelope: &AttestationEnvelope,
    chunk_bytes: usize,
    transfer_id: &str,
) -> Result<TransferBundle, MessagingError> {
    if envelope.transport != ProofTransport::LogosMessaging {
        return Err(MessagingError::WrongTransport);
    }
    if !(1..=MAX_CHUNK_BYTES).contains(&chunk_bytes) {
        return Err(MessagingError::InvalidChunkSize);
    }
    validate_transfer_id(transfer_id)?;
    let envelope_bytes = serde_json::to_vec(envelope)
        .map_err(|error| MessagingError::Encoding(error.to_string()))?;
    if envelope_bytes.len() > MAX_TRANSFER_BYTES {
        return Err(MessagingError::TransferTooLarge {
            actual: envelope_bytes.len(),
            maximum: MAX_TRANSFER_BYTES,
        });
    }
    let chunks = envelope_bytes.chunks(chunk_bytes).collect::<Vec<_>>();
    if chunks.len() > MAX_CHUNKS {
        return Err(MessagingError::TooManyChunks {
            actual: chunks.len(),
            maximum: MAX_CHUNKS,
        });
    }
    let chunk_count = u32::try_from(chunks.len()).expect("chunk count is bounded");
    let mut messages = Vec::with_capacity(chunks.len() + 1);
    for (index, chunk) in chunks.into_iter().enumerate() {
        messages.push(encode_wire_message(WireMessage {
            protocol: PROTOCOL.to_owned(),
            version: WIRE_VERSION,
            transfer_id: transfer_id.to_owned(),
            body: WireBody::Chunk {
                chunk_index: u32::try_from(index).expect("chunk index is bounded"),
                chunk_count,
                data_base64: BASE64.encode(chunk),
            },
        })?);
    }
    messages.push(encode_wire_message(WireMessage {
        protocol: PROTOCOL.to_owned(),
        version: WIRE_VERSION,
        transfer_id: transfer_id.to_owned(),
        body: WireBody::Manifest {
            envelope_sha256: digest_to_hex(&sha256(&envelope_bytes)),
            envelope_bytes: u64::try_from(envelope_bytes.len()).expect("transfer size is bounded"),
            chunk_count,
            context_hash: digest_to_hex(&envelope.context_hash()),
            challenge_digest: digest_to_hex(&envelope.challenge.digest()),
        },
    })?);
    Ok(TransferBundle {
        transfer_id: transfer_id.to_owned(),
        messages,
    })
}

fn validate_wire_body(body: &WireBody) -> Result<(), MessagingError> {
    match body {
        WireBody::Manifest {
            envelope_sha256,
            envelope_bytes,
            chunk_count,
            context_hash,
            challenge_digest,
        } => {
            if *chunk_count == 0
                || !matches!(
                    usize::try_from(*chunk_count),
                    Ok(count) if count <= MAX_CHUNKS
                )
                || !matches!(
                    usize::try_from(*envelope_bytes),
                    Ok(bytes) if bytes <= MAX_TRANSFER_BYTES
                )
                || digest_from_hex(envelope_sha256).is_err()
                || digest_from_hex(context_hash).is_err()
                || digest_from_hex(challenge_digest).is_err()
            {
                return Err(MessagingError::InvalidWireMessage(
                    "manifest fields are outside protocol bounds".to_owned(),
                ));
            }
        }
        WireBody::Chunk {
            chunk_index,
            chunk_count,
            data_base64,
        } => {
            if *chunk_count == 0
                || !matches!(
                    usize::try_from(*chunk_count),
                    Ok(count) if count <= MAX_CHUNKS
                )
                || chunk_index >= chunk_count
                || data_base64.len() > MAX_ENCODED_CHUNK_BYTES
            {
                return Err(MessagingError::InvalidWireMessage(
                    "chunk fields are outside protocol bounds".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

pub fn unpack_envelope<I, S>(
    contents: I,
    requested_transfer_id: Option<&str>,
) -> Result<(String, AttestationEnvelope), MessagingError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    if let Some(transfer_id) = requested_transfer_id {
        validate_transfer_id(transfer_id)?;
    }
    let mut transfers: BTreeMap<String, PartialTransfer> = BTreeMap::new();
    let mut manifest_order = Vec::new();
    for content in contents {
        let Ok(message) = serde_json::from_str::<WireMessage>(content.as_ref()) else {
            continue;
        };
        if message.protocol != PROTOCOL || message.version != WIRE_VERSION {
            continue;
        }
        if validate_transfer_id(&message.transfer_id).is_err() {
            continue;
        }
        if requested_transfer_id.is_some_and(|requested| requested != message.transfer_id) {
            continue;
        }
        validate_wire_body(&message.body)?;
        let transfer = transfers.entry(message.transfer_id.clone()).or_default();
        match message.body {
            WireBody::Manifest {
                envelope_sha256,
                envelope_bytes,
                chunk_count,
                context_hash,
                challenge_digest,
            } => {
                let manifest = Manifest {
                    envelope_sha256,
                    envelope_bytes,
                    chunk_count,
                    context_hash,
                    challenge_digest,
                };
                if transfer
                    .manifest
                    .as_ref()
                    .is_some_and(|existing| existing != &manifest)
                {
                    return Err(MessagingError::ConflictingManifest(message.transfer_id));
                }
                if transfer.manifest.is_none() {
                    manifest_order.push(message.transfer_id.clone());
                    transfer.manifest = Some(manifest);
                }
            }
            WireBody::Chunk {
                chunk_index,
                chunk_count,
                data_base64,
            } => {
                let chunk = EncodedChunk {
                    chunk_count,
                    data_base64,
                };
                if transfer
                    .chunks
                    .get(&chunk_index)
                    .is_some_and(|existing| existing != &chunk)
                {
                    return Err(MessagingError::ConflictingChunk(message.transfer_id));
                }
                transfer.chunks.insert(chunk_index, chunk);
            }
        }
    }

    let transfer_id = requested_transfer_id
        .map(str::to_owned)
        .or_else(|| manifest_order.pop());
    let transfer_id = transfer_id.ok_or(MessagingError::NoTransfer)?;
    let transfer = transfers
        .get(&transfer_id)
        .ok_or(MessagingError::NoTransfer)?;
    let envelope = transfer.assemble(&transfer_id)?;
    Ok((transfer_id, envelope))
}

#[derive(Debug, Default)]
struct PendingWireMessages {
    contents: Vec<String>,
    seen: BTreeSet<String>,
}

impl PendingWireMessages {
    fn observe<I, S>(
        &mut self,
        contents: I,
        requested_transfer_id: Option<&str>,
    ) -> Result<(), MessagingError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for content in contents {
            let content = content.as_ref();
            let Ok(message) = serde_json::from_str::<WireMessage>(content) else {
                continue;
            };
            if message.protocol != PROTOCOL
                || message.version != WIRE_VERSION
                || validate_transfer_id(&message.transfer_id).is_err()
                || requested_transfer_id.is_some_and(|requested| requested != message.transfer_id)
            {
                continue;
            }
            validate_wire_body(&message.body)?;
            if self.seen.contains(content) {
                continue;
            }
            if self.contents.len() >= MAX_PENDING_WIRE_MESSAGES {
                return Err(MessagingError::InvalidWireMessage(
                    "too many pending transfer messages".to_owned(),
                ));
            }
            self.seen.insert(content.to_owned());
            self.contents.push(content.to_owned());
        }
        Ok(())
    }

    fn unpack(
        &self,
        requested_transfer_id: Option<&str>,
    ) -> Result<(String, AttestationEnvelope), MessagingError> {
        unpack_envelope(self.contents.iter(), requested_transfer_id)
    }
}

pub fn receive_envelope(
    client: &LogosCoreClient,
    conversation_id: &str,
    requested_transfer_id: Option<&str>,
    expected_sender: Option<&str>,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<(String, AttestationEnvelope), MessagingError> {
    let started = Instant::now();
    let mut pending = PendingWireMessages::default();
    loop {
        let messages = client.get_messages(conversation_id)?;
        let contents = messages
            .iter()
            .filter(|message| !message.from_self)
            .filter(|message| {
                expected_sender.is_none_or(|expected| message.sender.as_deref() == Some(expected))
            })
            .map(|message| message.content.as_str());
        // get_messages is a rolling window, so retain bounded unique protocol
        // messages across polls until a complete transfer can be assembled.
        pending.observe(contents, requested_transfer_id)?;
        match pending.unpack(requested_transfer_id) {
            Ok(envelope) => return Ok(envelope),
            Err(MessagingError::NoTransfer | MessagingError::IncompleteTransfer { .. }) => {}
            Err(error) => return Err(error),
        }
        if started.elapsed() >= timeout {
            return Err(MessagingError::ReceiveTimeout);
        }
        thread::sleep(poll_interval);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PartialTransfer {
    manifest: Option<Manifest>,
    chunks: BTreeMap<u32, EncodedChunk>,
}

impl PartialTransfer {
    fn assemble(&self, transfer_id: &str) -> Result<AttestationEnvelope, MessagingError> {
        let manifest = self.manifest.as_ref().ok_or(MessagingError::NoTransfer)?;
        let expected = usize::try_from(manifest.chunk_count)
            .map_err(|error| MessagingError::InvalidWireMessage(error.to_string()))?;
        if expected == 0 || expected > MAX_CHUNKS {
            return Err(MessagingError::InvalidWireMessage(
                "chunk count is outside protocol bounds".to_owned(),
            ));
        }
        if self.chunks.len() != expected {
            return Err(MessagingError::IncompleteTransfer {
                transfer_id: transfer_id.to_owned(),
                received: self.chunks.len(),
                expected,
            });
        }
        let expected_bytes = usize::try_from(manifest.envelope_bytes)
            .map_err(|error| MessagingError::InvalidWireMessage(error.to_string()))?;
        if expected_bytes > MAX_TRANSFER_BYTES {
            return Err(MessagingError::TransferTooLarge {
                actual: expected_bytes,
                maximum: MAX_TRANSFER_BYTES,
            });
        }
        let mut envelope_bytes = Vec::with_capacity(expected_bytes);
        for index in 0..manifest.chunk_count {
            let chunk =
                self.chunks
                    .get(&index)
                    .ok_or_else(|| MessagingError::IncompleteTransfer {
                        transfer_id: transfer_id.to_owned(),
                        received: self.chunks.len(),
                        expected,
                    })?;
            if chunk.chunk_count != manifest.chunk_count {
                return Err(MessagingError::InvalidWireMessage(
                    "chunk count differs from manifest".to_owned(),
                ));
            }
            let decoded = BASE64.decode(&chunk.data_base64).map_err(|error| {
                MessagingError::InvalidWireMessage(format!("invalid chunk base64: {error}"))
            })?;
            if decoded.len() > MAX_CHUNK_BYTES {
                return Err(MessagingError::InvalidWireMessage(
                    "decoded chunk exceeds protocol limit".to_owned(),
                ));
            }
            envelope_bytes.extend_from_slice(&decoded);
        }
        if envelope_bytes.len() != expected_bytes
            || digest_to_hex(&sha256(&envelope_bytes)) != manifest.envelope_sha256
        {
            return Err(MessagingError::IntegrityFailure);
        }
        digest_from_hex(&manifest.context_hash)
            .map_err(|error| MessagingError::InvalidWireMessage(error.to_string()))?;
        digest_from_hex(&manifest.challenge_digest)
            .map_err(|error| MessagingError::InvalidWireMessage(error.to_string()))?;
        let envelope: AttestationEnvelope = serde_json::from_slice(&envelope_bytes)
            .map_err(|error| MessagingError::InvalidWireMessage(error.to_string()))?;
        if envelope.transport != ProofTransport::LogosMessaging {
            return Err(MessagingError::WrongTransport);
        }
        if digest_to_hex(&envelope.context_hash()) != manifest.context_hash
            || digest_to_hex(&envelope.challenge.digest()) != manifest.challenge_digest
        {
            return Err(MessagingError::IntegrityFailure);
        }
        Ok(envelope)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Manifest {
    envelope_sha256: String,
    envelope_bytes: u64,
    chunk_count: u32,
    context_hash: String,
    challenge_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EncodedChunk {
    chunk_count: u32,
    data_base64: String,
}

fn encode_wire_message(message: WireMessage) -> Result<String, MessagingError> {
    serde_json::to_string(&message).map_err(|error| MessagingError::Encoding(error.to_string()))
}

fn require_result_success(response: &Value) -> Result<(), MessagingError> {
    let result = response
        .get("result")
        .ok_or_else(|| MessagingError::LogosCoreResponse("missing result".to_owned()))?;
    if result.get("success").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    Err(MessagingError::LogosCoreCall(
        result
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("chat_module returned an unsuccessful result")
            .to_owned(),
    ))
}

fn validate_transfer_id(transfer_id: &str) -> Result<(), MessagingError> {
    if transfer_id.len() != 32
        || !transfer_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(MessagingError::InvalidTransferId);
    }
    Ok(())
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn append_string(bytes: &mut Vec<u8>, value: &str) {
    let value = value.as_bytes();
    let len = u32::try_from(value.len()).expect("Logos Chat identifier is too long");
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value);
}

fn admission_binding(
    challenge: &VerificationChallenge,
    group_id: &str,
    member_address: &str,
    random_prefix: [u8; 16],
) -> [u8; 32] {
    let mut binding = Vec::new();
    binding.extend_from_slice(ADMISSION_CHALLENGE_DOMAIN);
    binding.extend_from_slice(&challenge.version.to_le_bytes());
    binding.extend_from_slice(&challenge.gate_context_hash);
    append_string(&mut binding, &challenge.verifier_id);
    binding.extend_from_slice(&challenge.issued_at_unix_ms.to_le_bytes());
    binding.extend_from_slice(&challenge.expires_at_unix_ms.to_le_bytes());
    binding.extend_from_slice(&random_prefix);
    append_string(&mut binding, group_id);
    append_string(&mut binding, member_address);
    sha256(&binding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use attestation_types::{
        AttestationJournal, GateContext, VerificationChallenge, ATTESTATION_JOURNAL_VERSION,
    };

    fn envelope() -> AttestationEnvelope {
        let context = GateContext {
            application_id: "tokenstudio".to_owned(),
            gate_id: "founders".to_owned(),
            token_program_owner: [7; 8],
            token_definition_id: [4; 32],
            threshold: 100,
            verifier_id: "logos-chat:founders".to_owned(),
            expires_at_unix_ms: Some(20_000),
        };
        AttestationEnvelope {
            journal: AttestationJournal {
                version: ATTESTATION_JOURNAL_VERSION,
                context_hash: context.context_hash(),
                token_program_owner: context.token_program_owner,
                token_definition_id: context.token_definition_id,
                threshold: context.threshold,
                commitment_root: [9; 32],
                presenter_public_key: [3; 32],
                issued_at_unix_ms: 1_000,
                expires_at_unix_ms: context.expires_at_unix_ms,
            },
            receipt: vec![42; 220_000],
            challenge: VerificationChallenge::new(
                context.context_hash(),
                [9; 32],
                context.verifier_id,
                [5; 32],
                900,
                1_900,
            ),
            presenter_signature: vec![8; 64],
            transport: ProofTransport::LogosMessaging,
        }
    }

    #[test]
    fn large_envelope_round_trips_in_bounded_messages() {
        let envelope = envelope();
        let bundle = pack_envelope_with_id(
            &envelope,
            DEFAULT_CHUNK_BYTES,
            "00112233445566778899aabbccddeeff",
        )
        .unwrap();

        assert!(bundle.messages.len() > 2);
        assert!(bundle.messages.iter().all(|message| message.len() < 65_536));
        let (transfer_id, unpacked) =
            unpack_envelope(bundle.messages.iter(), Some(&bundle.transfer_id)).unwrap();
        assert_eq!(transfer_id, bundle.transfer_id);
        assert_eq!(unpacked, envelope);
    }

    #[test]
    fn rolling_message_windows_are_accumulated() {
        let envelope = envelope();
        let bundle = pack_envelope_with_id(
            &envelope,
            DEFAULT_CHUNK_BYTES,
            "00112233445566778899aabbccddeeff",
        )
        .unwrap();
        let split = bundle.messages.len() / 2;
        let mut pending = PendingWireMessages::default();

        pending
            .observe(
                bundle.messages[..split].iter().map(String::as_str),
                Some(&bundle.transfer_id),
            )
            .unwrap();
        assert!(matches!(
            pending.unpack(Some(&bundle.transfer_id)),
            Err(MessagingError::NoTransfer)
        ));

        let mut later_window = bundle.messages[split.saturating_sub(1)..].to_vec();
        later_window.reverse();
        pending
            .observe(
                later_window.iter().map(String::as_str),
                Some(&bundle.transfer_id),
            )
            .unwrap();
        let (_, unpacked) = pending.unpack(Some(&bundle.transfer_id)).unwrap();
        assert_eq!(unpacked, envelope);
    }

    #[test]
    fn oversized_encoded_chunks_are_rejected_before_accumulation() {
        let envelope = envelope();
        let bundle = pack_envelope_with_id(
            &envelope,
            DEFAULT_CHUNK_BYTES,
            "00112233445566778899aabbccddeeff",
        )
        .unwrap();
        let mut message: WireMessage = serde_json::from_str(&bundle.messages[0]).unwrap();
        let WireBody::Chunk { data_base64, .. } = &mut message.body else {
            panic!("first message must be a chunk");
        };
        *data_base64 = "A".repeat(MAX_ENCODED_CHUNK_BYTES + 1);
        let content = serde_json::to_string(&message).unwrap();
        let mut pending = PendingWireMessages::default();

        assert!(matches!(
            pending.observe([content.as_str()], Some(&bundle.transfer_id)),
            Err(MessagingError::InvalidWireMessage(_))
        ));
        assert!(pending.contents.is_empty());
    }

    #[test]
    fn manifest_is_sent_last_and_incomplete_transfer_is_rejected() {
        let envelope = envelope();
        let mut bundle = pack_envelope_with_id(
            &envelope,
            DEFAULT_CHUNK_BYTES,
            "00112233445566778899aabbccddeeff",
        )
        .unwrap();
        let last: WireMessage = serde_json::from_str(bundle.messages.last().unwrap()).unwrap();
        assert!(matches!(last.body, WireBody::Manifest { .. }));

        bundle.messages.remove(0);
        assert!(matches!(
            unpack_envelope(bundle.messages.iter(), Some(&bundle.transfer_id)),
            Err(MessagingError::IncompleteTransfer { .. })
        ));
    }

    #[test]
    fn modified_chunk_fails_integrity_check() {
        let envelope = envelope();
        let mut bundle = pack_envelope_with_id(
            &envelope,
            DEFAULT_CHUNK_BYTES,
            "00112233445566778899aabbccddeeff",
        )
        .unwrap();
        let mut first: WireMessage = serde_json::from_str(&bundle.messages[0]).unwrap();
        let WireBody::Chunk { data_base64, .. } = &mut first.body else {
            panic!("first message must be a chunk");
        };
        data_base64.replace_range(..1, "A");
        bundle.messages[0] = serde_json::to_string(&first).unwrap();

        assert!(matches!(
            unpack_envelope(bundle.messages.iter(), Some(&bundle.transfer_id)),
            Err(MessagingError::IntegrityFailure)
        ));
    }

    #[test]
    fn unrelated_chat_messages_are_ignored() {
        let envelope = envelope();
        let bundle = pack_envelope_with_id(
            &envelope,
            DEFAULT_CHUNK_BYTES,
            "00112233445566778899aabbccddeeff",
        )
        .unwrap();
        let mut contents = vec!["hello", "{\"protocol\":\"something-else\"}"];
        contents.extend(bundle.messages.iter().map(String::as_str));

        assert!(unpack_envelope(contents, None).is_ok());
    }

    #[test]
    fn admission_challenge_binds_group_and_member_without_changing_verifier() {
        let challenge = envelope().challenge;
        let verifier_id = challenge.verifier_id.clone();
        let bound = bind_admission_challenge(challenge, "founders", "logos:alice");

        assert_eq!(bound.verifier_id, verifier_id);
        assert!(admission_challenge_matches(
            &bound,
            "founders",
            "logos:alice"
        ));
        assert!(!admission_challenge_matches(
            &bound,
            "founders-2",
            "logos:alice"
        ));
        assert!(!admission_challenge_matches(
            &bound,
            "founders",
            "logos:bob"
        ));
    }

    #[test]
    fn challenge_wire_round_trips_and_ignores_unrelated_messages() {
        let challenge = bind_admission_challenge(envelope().challenge, "founders", "logos:alice");
        let encoded = encode_challenge(&challenge).unwrap();

        assert_eq!(latest_challenge(["hello", &encoded]).unwrap(), challenge);
    }

    #[test]
    fn challenge_selection_is_independent_of_chat_history_order() {
        let older = envelope().challenge;
        let mut newer = older.clone();
        newer.issued_at_unix_ms += 10;
        newer.expires_at_unix_ms += 10;
        let older = encode_challenge(&older).unwrap();
        let newer = encode_challenge(&newer).unwrap();

        assert_eq!(
            latest_challenge([newer.as_str(), older.as_str()]).unwrap(),
            serde_json::from_str::<ChallengeWireMessage>(&newer)
                .unwrap()
                .challenge
        );
    }
}
