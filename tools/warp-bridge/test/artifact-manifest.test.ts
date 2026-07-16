import { createHash } from "node:crypto";

import { describe, expect, test } from "vitest";

import {
  assertArtifactRootOverrideAllowed,
  RELEASE_TARGETS,
  validateManifest,
  verifyArtifactBytes,
  verifyArtifactTarget,
} from "../scripts/artifact-manifest.mjs";

const artifactBytes = Buffer.from("verified bridge artifact");
const artifactHash = createHash("sha256").update(artifactBytes).digest("hex");

function manifest() {
  return {
    manifest_version: 1,
    bridge_version: "0.1.0",
    pi_version: "0.80.6",
    pi_source_revision: "2b3fda9921b5590f285165287bd442a25817f17b",
    core_protocol_version: 2,
    core_schema_hash: "sha256:schema",
    artifacts: {
      "aarch64-apple-darwin": {
        relative_path: "bun-darwin-arm64/warp-bridge",
        size: artifactBytes.byteLength,
        sha256: artifactHash,
      },
    },
  };
}

describe("Bridge Artifact Manifest", () => {
  test("accepts an artifact only when target, size, and digest match", () => {
    const parsed = validateManifest(manifest(), {
      bridgeVersion: "0.1.0",
      piVersion: "0.80.6",
      piSourceRevision: "2b3fda9921b5590f285165287bd442a25817f17b",
      coreProtocolVersion: 2,
      coreSchemaHash: "sha256:schema",
      requiredTargets: ["aarch64-apple-darwin"],
    });

    expect(verifyArtifactBytes(parsed, "aarch64-apple-darwin", artifactBytes)).toEqual(
      parsed.artifacts["aarch64-apple-darwin"],
    );
    const changedArtifactBytes = Buffer.from(artifactBytes);
    changedArtifactBytes[0] = changedArtifactBytes[0]! ^ 0xff;
    expect(() =>
      verifyArtifactBytes(parsed, "aarch64-apple-darwin", changedArtifactBytes),
    ).toThrow(/SHA-256/);
    expect(() => verifyArtifactBytes(parsed, "x86_64-apple-darwin", artifactBytes)).toThrow(
      /target/,
    );
  });

  test("pins the complete six-target release matrix", () => {
    expect(RELEASE_TARGETS.map((entry) => entry.rustTarget)).toEqual([
      "aarch64-apple-darwin",
      "x86_64-apple-darwin",
      "aarch64-unknown-linux-gnu",
      "x86_64-unknown-linux-gnu",
      "aarch64-pc-windows-msvc",
      "x86_64-pc-windows-msvc",
    ]);
  });

  test("rejects a valid executable for the wrong target", () => {
    const machO = Buffer.alloc(12);
    machO.writeUInt32LE(0xfeedfacf, 0);
    machO.writeUInt32LE(0x0100000c, 4);
    expect(() => verifyArtifactTarget("aarch64-apple-darwin", machO)).not.toThrow();
    expect(() => verifyArtifactTarget("x86_64-apple-darwin", machO)).toThrow(/target mismatch/);
  });

  test("rejects protocol or supply-chain identity drift", () => {
    expect(() =>
      validateManifest(manifest(), {
        bridgeVersion: "0.1.0",
        piVersion: "0.80.6",
        piSourceRevision: "different-revision",
        coreProtocolVersion: 2,
        coreSchemaHash: "sha256:schema",
        requiredTargets: ["aarch64-apple-darwin"],
      }),
    ).toThrow(/Pi source revision/);
  });

  test("rejects local artifact roots for release packaging", () => {
    expect(() => assertArtifactRootOverrideAllowed(true, "/tmp/local-bridge")).toThrow(
      /release packaging/,
    );
    expect(() => assertArtifactRootOverrideAllowed(false, "/tmp/local-bridge")).not.toThrow();
  });
});
