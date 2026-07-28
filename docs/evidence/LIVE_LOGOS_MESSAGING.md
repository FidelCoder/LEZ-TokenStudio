# Live Logos Messaging Evidence

Current root-bound validation completed on 2026-07-28 against two isolated
Logos Core/Chat instances. The run used VerificationChallenge v2, an
independently authorized root, exact retained-challenge matching, and a fresh
real Risc0 receipt.

## Official Chat Baseline

The official encrypted Chat exchange doctest was validated first against the
pinned Chat revision:

```bash
nix run github:logos-co/logos-doctest -- \
  run doctests/chat-module-exchange.test.yaml \
  --release-for logos-chat-module=afb965589afb193a8559faf911233221a681af80
```

Result: **20 passed, 0 failed, 0 skipped**. It started two isolated daemons,
created an encrypted DirectV1 conversation, exchanged messages in both
directions, and shut both daemons down cleanly.

## Current ProofGate Admission Run

The verifier issued a root-bound challenge for one holder and GroupV2
conversation. The holder received it over encrypted Chat and signed the fresh
real receipt presentation.

```text
Commitment root: 510dbd3aa09bae25bc9e683e65ff2104eba32565f9c10b91582596955505b778
Challenge digest: dd9a7bda14e65506ee7cbbe0eccdf27c669f7151c4f4801938e271047f96af54
Forwarded presentation denied [1008]
Transfer ID: 05c84bf097d9ddcdb0227ed549577af7
Messages: 11 unique x 2 copies
ALLOW (Logos Messaging)
Consumed challenges: 1
Member admitted: 2eab11a6e774df5e9a66de6e205ea625dc5a0914e66456b0d4eaf7eb1b220d20
PASS: encrypted challenge and proof transport, local verification,
      bound admission, forwarding denial, replay denial
```

The receiver reconstructed the 299,830-byte presentation from ten bounded proof
chunks plus one manifest. Delivery redundancy produced two copies of each
unique message; transfer-ID/chunk-index deduplication accepted the complete set
once. The receiver checked the encrypted sender, exact retained challenge,
authorized root, Risc0 receipt, and presenter signature locally before calling
`add_group_member`. It polled `list_group_members` until the holder appeared
in the MLS roster. A forwarded presentation failed with `1008`, and a second
verification of the admitted presentation failed with persistent replay code
`1010`.

## Artifact Integrity

The public evidence is stored under `artifacts/live-chat-current`. Its
`SHA256SUMS` manifest verifies every included artifact.

| Public artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Fresh proof JSON | 299,244 | `999eebc71fe09acf02a287bcf1ab4a370d325ef2e12f1861b74df9e9567a07eb` |
| Redacted live transcript | 1,811 | `b5b2c8a51e378216ae85207539d238c132a7c3a9dc6b2128fbe97df7dcb3151a` |
| Verified presentation | 299,830 | `f71fb66b8c0d9025fe6ba8d9b74e0d1e4314bb36d908dfaa0f6611484705cbbb` |

Presenter keys, private witnesses, daemon state, and live endpoint credentials
are not included. The transcript contains only public addresses, transfer and
group identifiers, hashes, result codes, and aggregate sizes.
