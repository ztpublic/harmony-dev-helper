import { cp, mkdir, rm, access } from "node:fs/promises";
import { constants } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const root = path.resolve(__dirname, "..");

const source = path.join(root, "apps", "webview", "dist");
const targets = [
  path.join(root, "apps", "vscode-extension", "media", "webview"),
  path.join(root, "apps", "intellij-plugin", "src", "main", "resources", "webview")
];

async function ensureSourceExists() {
  try {
    await access(source, constants.F_OK);
  } catch {
    throw new Error("Webview dist not found. Run `pnpm --filter @harmony/webview build` first.");
  }
}

async function syncTarget(target) {
  await rm(target, { recursive: true, force: true });
  await mkdir(path.dirname(target), { recursive: true });
  await cp(source, target, { recursive: true });
}

async function main() {
  await ensureSourceExists();
  await Promise.all(targets.map(syncTarget));
  console.log("Synced webview assets to plugin hosts.");
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
