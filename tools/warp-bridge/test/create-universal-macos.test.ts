import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "vitest";

const MH_MAGIC_64 = 0xfeedfacf;
const CPU_TYPE_X86_64 = 0x01000007;
const CPU_TYPE_ARM64 = 0x0100000c;

function thinMachO(cpuType: number, marker: string): Buffer {
  const bytes = Buffer.alloc(64);
  bytes.writeUInt32LE(MH_MAGIC_64, 0);
  bytes.writeUInt32LE(cpuType, 4);
  bytes.writeUInt32LE(0, 8);
  bytes.write(marker, 16);
  return bytes;
}

test("creates a two-slice macOS universal Bridge without loading full artifacts", () => {
  const directory = mkdtempSync(resolve(tmpdir(), "warp-bridge-universal-test-"));
  try {
    const armPath = resolve(directory, "arm-bridge");
    const intelPath = resolve(directory, "intel-bridge");
    const outputPath = resolve(directory, "universal-bridge");
    writeFileSync(armPath, thinMachO(CPU_TYPE_ARM64, "arm"));
    writeFileSync(intelPath, thinMachO(CPU_TYPE_X86_64, "intel"));

    const testDirectory = dirname(fileURLToPath(import.meta.url));
    execFileSync(process.execPath, [
      resolve(testDirectory, "../scripts/create-universal-macos.mjs"),
      outputPath,
      armPath,
      intelPath,
    ]);

    const universal = readFileSync(outputPath);
    expect(universal.readUInt32BE(0)).toBe(0xcafebabe);
    expect(universal.readUInt32BE(4)).toBe(2);
    expect(universal.readUInt32BE(8)).toBe(CPU_TYPE_ARM64);
    expect(universal.readUInt32BE(28)).toBe(CPU_TYPE_X86_64);
    expect(
      universal.subarray(universal.readUInt32BE(16)).includes(Buffer.from("arm")),
    ).toBe(true);
    expect(
      universal.subarray(universal.readUInt32BE(36)).includes(Buffer.from("intel")),
    ).toBe(true);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
