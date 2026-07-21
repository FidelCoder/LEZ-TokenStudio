# Live Logos Messaging Evidence

Validated on 2026-07-21 against `logos.test` with two isolated Logos Core
daemons and `logos-chat-module` revision
`afb965589afb193a8559faf911233221a681af80`.

This is historical transport and admission evidence from before
VerificationChallenge v2 added verifier-authorized roots and exact retained
challenge matching. It does not claim that the current root-bound revision has
completed the same live network run.

## Official Chat Baseline

The current official encrypted exchange doctest was run first:

```bash
nix shell nixpkgs#jq -c \
  nix run github:logos-co/logos-doctest -- \
    run doctests/chat-module-exchange.test.yaml \
    --release-for logos-chat-module=afb965589afb193a8559faf911233221a681af80
```

Result: **20 passed, 0 failed, 0 skipped**. The run built the pinned Chat and
Delivery modules, started two isolated daemons, reached `online`, created an
encrypted DirectV1 conversation, exchanged a message in each direction, and
shut both daemons down cleanly.

## ProofGate Admission Run

After the baseline passed, `demos/token-gated-chat/run.sh` was executed against
a fresh verifier-owned GroupV2 conversation. The proof was generated after the
two nodes, direct conversation, and target group were ready:

```text
RISC0_DEV_MODE: disabled (real proving)
Receipt bytes: 223970
Proof issued: 2026-07-21T04:48:44.041Z
Holder address: 174f01c8da921830133ea06c2bb5fbb8befc417ca514880c1ea4277c99e4e9c1
Target group: ef0b676431
Challenge digest: a4caabbb3bb36f89e5a5391ea12a9fe5e5e9c7e6c1f444262063c36312f21a01
Forwarded presentation denied [1008]
Transfer ID: 8d0ef26ff357eba814a7c385effbc6f8
Messages: 11
ALLOW (Logos Messaging)
Admission requested for the challenge-bound holder and group
Member admitted: 174f01c8da921830133ea06c2bb5fbb8befc417ca514880c1ea4277c99e4e9c1
PASS: encrypted challenge and proof transport, local verification, bound admission, forwarding denial, replay denial
```

The 11 messages are ten bounded proof chunks followed by one commit manifest.
The receiver reconstructed the 299,730-byte presentation, checked the expected
encrypted sender, verified the Risc0 receipt and presenter signature locally,
then called `add_group_member`. The script polled `list_group_members` until the
holder appeared in the real MLS roster. Its final step only passes when a
second verification is rejected with replay code `[1010]`.

## Artifact Integrity

| Disposable local artifact | SHA-256 |
| --- | --- |
| Fresh proof JSON | `07c1a4d9d501f33a32ea19cdef257bed029c9f1abc614f8affb1fac69d2415de` |
| Redacted live transcript | `aa83391c91c7b4e610e8e336e6b5c018e02f581bd459fb2a0cbfa92bcb78154b` |
| Verified presentation | `4d73271b285c8badefa32824d61906f3a52d94835c1d80d2865acf31227ce407` |

The disposable presenter key and private witness are not committed. The
transcript contains only public addresses, group/transfer identifiers, hashes,
result codes, and aggregate sizes.
