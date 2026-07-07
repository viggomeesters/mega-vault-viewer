import { spawnSync } from "node:child_process";
import process from "node:process";

const bundlesByPlatform = {
  darwin: "app",
  linux: "appimage",
  win32: "nsis,msi",
};

const bundles = process.env.TAURI_BUNDLES ?? bundlesByPlatform[process.platform];
if (!bundles) {
  console.error(`Unsupported platform for tauri build: ${process.platform}`);
  process.exit(2);
}

const result = spawnSync("tauri", ["build", "--bundles", bundles], {
  stdio: "inherit",
  shell: process.platform === "win32",
});
process.exit(result.status ?? 1);
