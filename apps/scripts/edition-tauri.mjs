import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [edition, command, ...args] = process.argv.slice(2);
if (!["player", "research"].includes(edition) || !command) {
  throw new Error("Specify player or research and a Tauri command.");
}
const root = fileURLToPath(new URL("../", import.meta.url));
// The Tauri CLI builds the same Cargo package before renaming the research executable.
// Give the editions separate target directories so the second build cannot overwrite the first.
const target = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR, edition)
  : resolve(root, "src-tauri", edition === "research" ? "target" : "target-player");
const config = edition === "research" && ["dev", "build"].includes(command)
  ? ["--config", "src-tauri/tauri.research.conf.json"] : [];
const cli = fileURLToPath(new URL("../node_modules/@tauri-apps/cli/tauri.js", import.meta.url));
const env = { ...process.env, CARGO_TARGET_DIR: target };
if (edition === "research" && command === "dev" && !env.HYACIMICON_DATA_DIR) {
  const localData = resolve(root, "../docs/game-data/runtime");
  if (existsSync(localData)) env.HYACIMICON_DATA_DIR = localData;
}
const result = spawnSync(process.execPath, [cli, command, ...config, ...args], {
  cwd: root, stdio: "inherit", env,
});
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
