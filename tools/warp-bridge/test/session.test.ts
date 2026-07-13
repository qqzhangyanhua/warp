import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vitest";

import { CORE_SCHEMA_HASH } from "../src/protocol-identity.js";
import { BridgeProtocolSession, BridgeSessionError } from "../src/session.js";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const acceptedHandshake = readFileSync(
  resolve(root, "protocol/fixtures/valid/handshake-result-accepted.jsonl"),
  "utf8",
).trim();
const runStart = readFileSync(
  resolve(root, "protocol/fixtures/valid/text-run-lifecycle.jsonl"),
  "utf8",
).split("\n")[0];
const transcriptSync = readFileSync(
  resolve(root, "protocol/fixtures/valid/transcript-sync.jsonl"),
  "utf8",
).trim().split("\n");

describe("Bridge Protocol session", () => {
  test("pins the exact Core Protocol schema bytes", () => {
    const schema = readFileSync(resolve(root, "protocol/core-v1.schema.json"));
    const actual = `sha256:${createHash("sha256").update(schema).digest("hex")}`;

    expect(CORE_SCHEMA_HASH).toBe(actual);
  });

  test("rejects run configuration until Warp accepts the handshake", () => {
    const session = new BridgeProtocolSession();

    expect(() => session.receiveInboundLine(runStart ?? "")).toThrow(BridgeSessionError);
    expect(session.isReady()).toBe(false);

    session.receiveInboundLine(acceptedHandshake);

    expect(session.isReady()).toBe(true);
    expect(session.receiveInboundLine(runStart ?? "").type).toBe("run_start");
  });

  test("emits a hello with the pinned schema identity", () => {
    const hello = new BridgeProtocolSession().hello();

    expect(hello.core_schema_hash).toBe(CORE_SCHEMA_HASH);
    expect(hello.protocol_version).toBe(1);
  });

  test("rejects a Transcript Sync above the negotiated total limit", () => {
    const session = new BridgeProtocolSession();
    session.receiveInboundLine(acceptedHandshake);
    const oversizedBegin = (transcriptSync[0] ?? "").replace(
      '"total_bytes":112',
      '"total_bytes":16777217',
    );

    expect(() => session.receiveInboundLine(oversizedBegin)).toThrow(BridgeSessionError);
  });

  test("accepts an ordered complete Transcript Sync", () => {
    const session = new BridgeProtocolSession();
    session.receiveInboundLine(acceptedHandshake);

    for (const line of transcriptSync.slice(0, 3)) {
      session.receiveInboundLine(line);
    }
  });
});
