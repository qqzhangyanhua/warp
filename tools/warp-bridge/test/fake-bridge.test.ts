import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

import { expect, test } from "vitest";

import { parseProtocolLine } from "../src/protocol.js";
import { CORE_SCHEMA_HASH } from "../src/protocol-identity.js";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

test("fake Bridge handshakes before emitting scripted events", async () => {
  const acceptedHandshake = readFileSync(
    resolve(root, "protocol/fixtures/valid/handshake-result-accepted.jsonl"),
    "utf8",
  ).trim();
  const child = spawn(
    process.execPath,
    [resolve(root, "test/fake-bridge.mjs"), resolve(root, "protocol/fixtures/valid/run-terminal-outcomes.jsonl")],
    { stdio: ["pipe", "pipe", "pipe"] },
  );
  const lines: string[] = [];
  const exitCodePromise = new Promise<number | null>((resolveExit) => {
    child.once("exit", resolveExit);
  });

  for await (const line of createInterface({ input: child.stdout })) {
    lines.push(line);
    if (lines.length === 1) {
      child.stdin.end(`${acceptedHandshake}\n`);
    }
  }
  const exitCode = await exitCodePromise;

  expect(exitCode).toBe(0);
  const hello = parseProtocolLine(lines[0] ?? "");
  expect(hello.type).toBe("bridge_hello");
  expect(hello.type === "bridge_hello" && hello.core_schema_hash).toBe(CORE_SCHEMA_HASH);
  expect(lines.slice(1).map((line) => parseProtocolLine(line).type)).toEqual([
    "run_finished",
    "run_finished",
    "run_finished",
  ]);
});
