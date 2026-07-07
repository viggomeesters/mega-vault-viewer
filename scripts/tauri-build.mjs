import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";
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

const tauriEnv = {
  ...process.env,
  // WSL and many CI runners cannot mount AppImage helper binaries through FUSE.
  // This lets linuxdeploy/appimagetool extract and run without requiring libfuse2.
  APPIMAGE_EXTRACT_AND_RUN: process.env.APPIMAGE_EXTRACT_AND_RUN ?? "1",
};

const result = spawnSync("tauri", ["build", "--bundles", bundles], {
  stdio: "inherit",
  shell: process.platform === "win32",
  env: tauriEnv,
});

if (result.status === 0) {
  process.exit(0);
}

if (process.platform === "linux" && bundles.split(",").map((value) => value.trim()).includes("appimage")) {
  const repaired = repairLinuxAppImageBundle();
  if (repaired) {
    process.exit(0);
  }
}

process.exit(result.status ?? 1);

function repairLinuxAppImageBundle() {
  const repoRoot = resolve(import.meta.dirname, "..");
  const appimageDir = join(repoRoot, "target", "release", "bundle", "appimage");
  const appDir = findAppDir(appimageDir);
  if (!appDir) {
    console.error("Linux AppImage repair skipped: AppDir was not generated.");
    return false;
  }

  const desktopFile = findDesktopFile(appDir);
  const iconName = desktopFile ? readDesktopIconName(desktopFile) : "mega-vault-viewer";
  const sourceIcon = join(repoRoot, "apps", "desktop", "src-tauri", "icons", "icon.png");
  const expectedIcon = join(appDir, `${iconName}.png`);
  if (!existsSync(expectedIcon) && existsSync(sourceIcon)) {
    copyFileSync(sourceIcon, expectedIcon);
  }

  const plugin = join(process.env.HOME ?? "", ".cache", "tauri", "linuxdeploy-plugin-appimage.AppImage");
  if (!existsSync(plugin)) {
    console.error(`Linux AppImage repair skipped: missing ${plugin}`);
    return false;
  }

  const before = new Set(listAppImages(repoRoot));
  const packageResult = spawnSync(plugin, ["--appdir", appDir], {
    cwd: repoRoot,
    stdio: "inherit",
    env: tauriEnv,
  });
  if (packageResult.status !== 0) {
    return false;
  }

  mkdirSync(appimageDir, { recursive: true });
  for (const output of listAppImages(repoRoot)) {
    if (before.has(output)) {
      continue;
    }
    const target = join(appimageDir, basename(output));
    renameSync(output, target);
    console.log(`Repaired AppImage written to ${target}`);
  }

  return true;
}

function findAppDir(appimageDir) {
  if (!existsSync(appimageDir)) {
    return null;
  }
  return readdirSync(appimageDir)
    .map((entry) => join(appimageDir, entry))
    .find((entry) => entry.endsWith(".AppDir") && statSync(entry).isDirectory()) ?? null;
}

function findDesktopFile(appDir) {
  const applicationsDir = join(appDir, "usr", "share", "applications");
  if (!existsSync(applicationsDir)) {
    return null;
  }
  return readdirSync(applicationsDir)
    .map((entry) => join(applicationsDir, entry))
    .find((entry) => entry.endsWith(".desktop")) ?? null;
}

function readDesktopIconName(desktopFile) {
  const lines = readFileSync(desktopFile, "utf8").split(/\r?\n/);
  const iconLine = lines.find((line) => line.startsWith("Icon="));
  return iconLine?.slice("Icon=".length).trim() || "mega-vault-viewer";
}

function listAppImages(dir) {
  if (!existsSync(dir)) {
    return [];
  }
  return readdirSync(dir)
    .filter((entry) => entry.endsWith(".AppImage"))
    .map((entry) => join(dir, entry));
}
