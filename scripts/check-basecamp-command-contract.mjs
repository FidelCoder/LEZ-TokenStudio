import { readFile } from "node:fs/promises";

const source = await readFile(
  new URL("../apps/basecamp-tokenstudio/src/qml/pages/VerifyPage.qml", import.meta.url),
  "utf8",
);
const required = [
  '"on-chain", "private-account-generate"',
  '"on-chain", "private-account-id"',
  '"--badge-key", badgeAccountKey.text',
  '"--output-badge", badgeOutput.text',
];
for (const snippet of required) {
  if (!source.includes(snippet))
    throw new Error(`missing private badge QML command: ${snippet}`);
}
if (source.includes('"on-chain", "fetch-badge"'))
  throw new Error("private badge flow must not fetch plaintext from a public account");
if ((source.match(/"--output-badge"/g) || []).length !== 2)
  throw new Error("compose and claim-submit must both write the private badge");

console.log("Basecamp private badge command contract passed");
