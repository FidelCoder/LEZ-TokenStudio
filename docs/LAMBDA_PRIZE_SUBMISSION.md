# Lambda Prize Submission Playbook

## Official Route

The implementation lives in this public repo:

`https://github.com/FidelCoder/LEZ-TokenStudio`

The Lambda Prize submission itself is a pull request to:

`https://github.com/logos-co/lambda-prize`

That PR must add exactly one file:

`solutions/LP-0005.md`

The PR title should be:

`Solution: LP-0005 — TokenStudio ProofGate`

## What The Solution File Must Contain

Use `solutions/LP-0000.md` from the Lambda Prize repo as the template.

Required sections enforced by the official validator:

- `## Summary`
- `## Repository`
- `## Approach`
- `## Success Criteria Checklist`
- `## FURPS Self-Assessment`
- `### Functionality`
- `### Usability`
- `### Reliability`
- `### Performance`
- `### Supportability`
- `## Terms & Conditions`

The file must include a real GitHub repo link, not a placeholder.

## Validator Constraints

The official validation workflow checks:

- PR title starts with `Solution: LP-0005`.
- A solution PR only touches `solutions/`.
- `solutions/LP-0005.md` exists.
- The linked implementation repo is public and cloneable.
- The linked repo has a license.
- The linked repo has a README.
- The linked repo has CI.
- If the prize requires a demo script, the linked repo has `demo.sh` or
  `demo.bash`.
- If the prize requires SPEL/IDL, the linked repo has `.idl` or `.idl.json`.
- The solution file links the mandatory narrated video.
- If the prize references a Basecamp mini-app, the repo should include
  `module.json`.
- If the prize references Messaging/Waku/Chat, the source should reference
  Waku, Logos Messaging, or Logos Chat.
- The linked repo should not be trivial or nearly empty.
- Test files should be present.

## Payment Route

Payment is not claimed when the implementation repo is finished. Payment is
claimed only after the solution PR has been accepted and merged into
`logos-co/lambda-prize`.

After merge:

1. Open the Lambda Prize payment claim issue.
2. Provide full legal name.
3. Provide country of residence.
4. Provide Ethereum wallet address.
5. Receive payment in USDT on Ethereum if Logos verifies the claim.

Use a single-use Ethereum address if public wallet linkage is a concern because
the payment claim issue exposes the wallet address publicly.

## Working Git Flow

### Implementation Repo

Work here:

```bash
cd "/home/core/Desktop/LOGOS PRIVACY/LEZ-TokenStudio"
git checkout main
```

Build, test, document, and push this repo:

```bash
git push origin main
```

### Lambda Prize Fork

Fork `logos-co/lambda-prize` under the builder's GitHub account.

Then clone or add the fork remote:

```bash
git clone https://github.com/<builder>/lambda-prize.git
cd lambda-prize
git remote add upstream https://github.com/logos-co/lambda-prize.git
git checkout -b solution/lp-0005-proofgate
```

Create:

```text
solutions/LP-0005.md
```

Open a pull request from the fork branch to:

```text
logos-co/lambda-prize:master
```

PR title:

```text
Solution: LP-0005 — TokenStudio ProofGate
```

## Submission Readiness Checklist

- Public repo under MIT or Apache-2.0.
- README explains build, run, and demo.
- CI green on default branch.
- `scripts/demo.sh` works from a clean environment.
- Demo runs against local sequencer with `RISC0_DEV_MODE=0`.
- Risc0 proof generation visible in terminal output.
- On-chain verifier path demonstrated.
- Verifier program ID and transactions verified on public LEZ testnet.
- Private claim demonstrated against the public testnet gate.
- Off-chain Logos Messaging path demonstrated.
- Basecamp module present and documented.
- SPEL IDL present.
- Benchmarks include proof generation time and on-chain cost.
- Narrated video linked in `solutions/LP-0005.md`.
- FURPS self-assessment complete.
- Terms acknowledged.

## Official Sources

- Lambda Prize README: <https://github.com/logos-co/lambda-prize>
- LP-0005: <https://github.com/logos-co/lambda-prize/blob/master/prizes/LP-0005.md>
- Solution template: <https://github.com/logos-co/lambda-prize/blob/master/solutions/LP-0000.md>
- Payment claim template: <https://github.com/logos-co/lambda-prize/issues/new?template=lambda-prize-claim.yml>
