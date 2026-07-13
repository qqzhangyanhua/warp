import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const targets = [
  "bun-darwin-arm64",
  "bun-darwin-x64",
  "bun-linux-arm64",
  "bun-linux-x64",
  "bun-windows-arm64",
  "bun-windows-x64",
];

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const outputRoot = resolve(root, "dist/fake");
mkdirSync(outputRoot, { recursive: true });

for (const target of targets) {
  const executable = target.startsWith("bun-windows") ? "warp-bridge-fake.exe" : "warp-bridge-fake";
  const targetDirectory = resolve(outputRoot, target);
  mkdirSync(targetDirectory, { recursive: true });
  const result = spawnSync(
    "bun",
    [
      "build",
      "test/fake-bridge.mjs",
      "--compile",
      `--target=${target}`,
      `--outfile=${resolve(targetDirectory, executable)}`,
    ],
    { cwd: root, stdio: "inherit" },
  );
  if (result.status !== 0) {
    throw new Error(`Standalone fake Bridge build failed for ${target}`);
  }
}
