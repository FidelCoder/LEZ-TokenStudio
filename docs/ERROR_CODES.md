# ProofGate Error Codes

ProofGate uses stable numeric denial codes where callers need deterministic
handling. Error text is explanatory and may grow; integrations should key on
the numeric code.

## Off-Chain Verification Codes

| Code | Meaning |
| ---: | --- |
| 1000 | Invalid gate configuration |
| 1001 | Invalid proof receipt, image, or journal |
| 1002 | Gate context or configured expiry mismatch |
| 1003 | Token program owner or token definition mismatch |
| 1004 | Threshold mismatch |
| 1005 | Expired, stale, future-dated, or invalid time window |
| 1006 | Challenge is invalid or differs from the verifier-issued challenge |
| 1007 | Challenge expired |
| 1008 | Verifier identity mismatch |
| 1009 | Presenter key or signature invalid |
| 1010 | Challenge replay detected |
| 1011 | Malformed claim or envelope |
| 1012 | Proof issue time is invalid |
| 1013 | Proof commitment root is not the verifier's trusted root |

The off-chain verifier formats denials as
`verification denied [<code>]: <message>`.

## On-Chain State Codes

| Code | Meaning |
| ---: | --- |
| 1001 | Unsupported attestation journal version |
| 1002 | Gate context or configured expiry mismatch |
| 1003 | Token program owner or token definition mismatch |
| 1004 | Threshold mismatch |
| 1005 | Invalid proof timestamp validity window |
| 1009 | Presenter key or signature invalid |
| 1011 | Malformed claim or journal encoding |
| 1013 | Journal commitment root differs from the root authorized in GateState |
| 2000 | Gate state is malformed or unsupported |
| 2001 | Gate claim counter overflow |
| 2005 | Gate state or access badge could not be encoded or decoded |

The SPEL guest reports custom failures as
`ProofGate error <code>: <message>`. Account-constraint failures emitted by
SPEL itself retain SPEL's own error representation.

## Handling Guidance

- Treat `1006`, `1007`, and `1010` as recoverable by requesting a new
  verifier challenge.
- Treat `1005` as recoverable by generating a fresh proof while the gate is
  active.
- Treat `1002`, `1003`, and `1004` as configuration mismatch, not proof
  generation failure.
- Treat `1001`, `1008`, `1009`, `1011`, and `1013` as untrusted input
  and deny without retrying the same payload. A legitimate `1013` requires a
  fresh proof for the verifier/operator's authorized root.
- Treat all `2000`-series errors as program/state integration failures that
  require operator attention.
