# External Integrator Guide

LP-0005 requires at least one of the three testnet applications to be built by a
party outside the submitting team. A team-authored example cannot satisfy that
criterion. This guide keeps the outside integration small while preserving the
attestation trust boundaries.

## Integration Contract

An application must:

1. define its own `GateConfig` with a unique application, gate, and verifier
   identifier;
2. obtain a commitment root independently from the official LEZ testnet;
3. issue and retain a fresh challenge bound to that root;
4. receive an `AttestationEnvelope` from the holder;
5. call `attestation_verifier::verify` with its expected gate, retained
   challenge, verifier ID, current time, and persistent replay state;
6. perform the gated action only after verification returns success.

Do not accept a holder-supplied root, compare only self-asserted journal fields,
or reuse a challenge. Do not log private wallet output, snapshots, account IDs,
or recovery phrases.

## Fastest Reference

Use `crates/proofgate-governance` for a proposal vote, or follow
`demos/private-governance/README.md` and replace the proposal/application IDs.
The governance service already provides:

- proposal-bound challenge issuance;
- exact retained-challenge matching;
- real Risc0 receipt verification through the reusable verifier crate;
- persistent challenge replay protection;
- one vote per proof-bound presenter pseudonym;
- locked atomic state with mode `0600`.

The Logos Chat integration in `demos/token-gated-chat` is the reference for a
Messaging-native consumer.

## Testnet Proof

Use a private token holding that is actually committed in the live testnet
tree. The holder creates a private snapshot with the official wallet, then
ProofGate requests the Merkle proof directly from the sequencer:

```bash
export LEE_WALLET_HOME_DIR=/path/to/private-wallet
target/release/proofgate wallet snapshot \
  --wallet-binary /path/to/wallet \
  --account-id Private/ACCOUNT_ID \
  --output /tmp/private-snapshot.json
target/release/proofgate prove \
  --gate /path/to/external-gate.json \
  --account-snapshot /tmp/private-snapshot.json \
  --sequencer-url https://testnet.lez.logos.co \
  --presenter-public-key-hex PUBLIC_PRESENTER_KEY \
  --output /tmp/external-proof.json
```

Keep `/tmp/private-snapshot.json` and the wallet outside the public repository.
Only the proof, gate policy, transaction IDs, and sanitized application logs
belong in public evidence.

## Evidence To Return

The outside integrator should publish:

- application repository URL and immutable commit;
- author/team identity showing they are outside the submitting team;
- distinct application/gate/verifier identifiers;
- official testnet endpoint and relevant transaction/program IDs;
- an allow log for a real proof and denial logs for wrong-context and replay;
- build/run instructions from a clean checkout;
- license and dependency version;
- a short statement that no private wallet data is committed.

The submitting team should link this evidence without rewriting its authorship
history. An issue, pull request, or signed statement can establish provenance,
but the application code and commit must remain attributable to the outside
party.
