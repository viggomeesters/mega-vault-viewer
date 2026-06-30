import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const configured = process.env.MEGA_VAULT_VIEWER_MINIMAL_STARTER_PATH;
const candidates = [
  configured,
  resolve(repoRoot, "minimal-ai-vault-starter"),
  resolve(repoRoot, "..", "minimal-ai-vault-starter"),
  "/home/viggo/github/minimal-ai-vault-starter",
].filter(Boolean);

const starterPath = candidates.find((candidate) =>
  existsSync(resolve(candidate, "docs", "starter-contract.json")),
);

if (!starterPath) {
  console.error("Missing Minimal AI Vault Starter checkout.");
  console.error("Set MEGA_VAULT_VIEWER_MINIMAL_STARTER_PATH or place it next to mega-vault-viewer.");
  process.exit(1);
}

const result = spawnSync(
  "cargo",
  ["test", "-p", "mvv-core", "--test", "minimal_starter_smoke", "--", "--ignored", "--nocapture"],
  {
    cwd: repoRoot,
    env: {
      ...process.env,
      MEGA_VAULT_VIEWER_MINIMAL_STARTER_PATH: starterPath,
    },
    stdio: "inherit",
    shell: process.platform === "win32",
  },
);

process.exit(result.status ?? 1);
