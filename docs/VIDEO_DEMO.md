# LP-0005 Video Demo

The official LP-0005 submission requires a narrated end-to-end video. It must
show terminal proof generation with `RISC0_DEV_MODE=0`, explain the architecture
and key decisions, and demonstrate both the on-chain and Logos Messaging
verification paths. A silent screencast does not satisfy the requirement.

## Before Recording

From the repository root, run the fast recording preflight:

```bash
scripts/video-preflight.sh
```

It verifies the preserved real receipt and presenter binding, proves replay
denial, executes the actual SPEL verifier, re-fetches the included claim from
the official testnet, checks the sanitized live Messaging transcript, and
locates the Basecamp package and screenshot. It does not print private keys,
wallet snapshots, badge keys, or private account data.

The default private bundle is `/tmp/proofgate-testnet-claim-current`. Supply a
different bundle as the first argument when needed. The bundle must contain the
matching `proof.json`, `presenter-key.json`, `balance-prove.log`, and
`claim-submit.log` files.

## Required Proof-Generation Clip

The preflight re-verifies an existing real receipt; it does not replace the
official requirement to show proof generation. Record this command separately:

```bash
RISC0_DEV_MODE=0 RUN_COMPOSITION=0 scripts/demo.sh 2>&1 | \
  tee artifacts/video-real-proof.log
```

Show the command, the `Generate real private balance proof
(RISC0_DEV_MODE=0)` stage, and the final proof output. Real proving can take
well over an hour on an unaccelerated machine, so use a clearly narrated jump
cut or time lapse. Do not imply that the preserved-proof preflight generated a
new receipt.

Run `scripts/demo.sh` without `RUN_COMPOSITION=0` only when the recording also
needs a fresh recursive LEZ PPE proof. The public testnet bundle already records
a real recursive claim and its transaction inclusion.

## Suggested Recording Order

1. Introduce ProofGate and the private threshold problem. State that the public
   threshold is `100`, while the exact account and balance stay hidden.
2. Show `docs/ARCHITECTURE.md`. Explain the Risc0 balance guest, presenter-key
   binding, authorized sequencer root, SPEL GateState, and the two consumers.
3. Show the fresh proof-generation terminal clip above. Point out
   `RISC0_DEV_MODE=0`, the receipt size, and the successful local verification.
4. Run `scripts/video-preflight.sh`. Narrate `ALLOW`, replay denial `[1010]`,
   `ALLOW (LEZ guest execution)`, and the live testnet `claim_counter=1` state.
5. During the Messaging transcript, point out encrypted challenge delivery,
   chunked proof transport, forwarding denial `[1008]`, local verification,
   GroupV2 admission, and replay denial.
6. Show the Basecamp frame:

   ```bash
   xdg-open artifacts/basecamp-current/integration/app-data/proofgate-desktop.png
   ```

7. Close with the public program ID, deployment and claim transaction IDs,
   privacy boundary, proof-generation benchmark, and known testnet CU metadata
   limitation.

## Do Not Show

- `/tmp/proofgate-testnet-claim-current/presenter-key.json`;
- wallet directories, recovery phrases, or account snapshots;
- gate-account or private badge signing keys;
- environment files containing Chat credentials.

The public proof, claim, transaction hashes, GateState, program ID, benchmark
logs, and sanitized Messaging transcript are safe recording material.
