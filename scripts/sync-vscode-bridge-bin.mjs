import { access, cp, mkdir } from "node:fs/promises";
import { constants } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const root = path.resolve(__dirname, "..");

const ext = process.platform === "win32" ? ".exe" : "";
const source = path.join(
  root,
  "apps",
  "hdc-bridge-rs",
  "target",
  "debug",
  `hdc-bridge-rs${ext}`
);
const targetDir = path.join(root, "apps", "vscode-extension", "bin");
const target = path.join(targetDir, `hdc-bridge-rs${ext}`);

async function ensureSourceExists() {
  try {
    await access(source, constants.F_OK);
  } catch {
    throw new Error(
      `Bridge binary not found at ${source}. Run \`cargo build --manifest-path apps/hdc-bridge-rs/Cargo.toml\` first.`
    );
  }
}

async function main() {
  await ensureSourceExists();
  await mkdir(targetDir, { recursive: true });
  await cp(source, target, { force: true });
  console.log(`Synced bridge binary to ${target}.`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
