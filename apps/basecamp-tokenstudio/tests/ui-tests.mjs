import { writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

// Nix's long build TMPDIR plus the view-module name exceeds AF_UNIX's path limit.
if (process.argv.includes("--ci")) {
  process.env.TMPDIR = "/tmp";
}

const frameworkRoot = process.env.LOGOS_QT_MCP
  || new URL("../result-mcp", import.meta.url).pathname;
const { test, run } = await import(resolve(frameworkRoot, "test-framework/framework.mjs"));

test("ProofGate: loads and connects to its backend", async (app) => {
  await app.waitFor(
    async () => {
      await app.expectTexts(["TokenStudio ProofGate", "Private token access", "Ready"]);
    },
    { timeout: 30000, interval: 500, description: "ProofGate backend to become ready" },
  );
});

test("ProofGate: exposes token and gate controls", async (app) => {
  await app.expectTexts([
    "Token",
    "Create, register, or mint an LEZ token",
    "Save existing",
    "Preview creation",
    "Mint",
    "Gate policy",
    "Create gate",
  ]);
});

test("ProofGate: navigates every primary workflow", async (app) => {
  await app.expectTexts(["Gate", "Prove", "Verify"]);

  await app.click("Prove", { exact: true });
  await app.expectTexts([
    "Presenter",
    "Balance proof",
    "Presentation",
    "Issued challenge",
    "Trusted commitment root (hex)",
    "Read sequencer root",
    "witness.json",
    "300000",
    "Generate proof",
  ]);

  await app.click("Verify", { exact: true });
  await app.expectTexts(["Local", "Messaging", "On-chain", "Verify locally"]);

  await app.click("Messaging", { exact: true });
  await app.expectTexts([
    "Encrypted transport",
    "Trusted commitment root (hex)",
    "Read sequencer root",
    "Verify and admit",
    "Send proof",
    "Verify and admit",
  ]);

  await app.click("On-chain", { exact: true });
  await app.expectTexts([
    "LEZ access badge",
    "Authorized commitment root (hex)",
    "Maximum proof age (ms)",
    "600000",
    "Read sequencer root",
    "Deploy program",
    "Generate private badge account",
    "Read private badge ID",
    "Access badge output",
    "Submit claim",
  ]);
});

test("ProofGate: renders a non-empty desktop frame", async (app) => {
  const frame = await app.screenshot();
  if (frame.error || frame.width <= 0 || frame.height <= 0 || frame.image.length < 1000) {
    throw new Error(`invalid screenshot response: ${JSON.stringify(frame)}`);
  }
  const outputDir = process.env.LOGOS_DATA_DIR || process.cwd();
  await writeFile(join(outputDir, "proofgate-desktop.png"), Buffer.from(frame.image, "base64"));
});

run();
