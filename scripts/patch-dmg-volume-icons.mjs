import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

if (process.platform !== "darwin") {
  console.log("Skipping DMG volume icon patching outside macOS");
  process.exit(0);
}

const scriptDir = dirname(fileURLToPath(import.meta.url));
const patchScript = resolve(scriptDir, "patch-dmg-volume-icons.sh");
const rawArgs = process.argv.slice(2);
const searchRoot = rawArgs[0] === "--" ? rawArgs[1] : rawArgs[0];
const args = searchRoot ? [patchScript, searchRoot] : [patchScript];

execFileSync("bash", args, { stdio: "inherit" });
