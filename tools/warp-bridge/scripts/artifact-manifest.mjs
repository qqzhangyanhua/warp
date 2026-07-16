import { createHash, timingSafeEqual } from "node:crypto";
import { isAbsolute, normalize, sep } from "node:path";

export const RELEASE_TARGETS = [
  { rustTarget: "aarch64-apple-darwin", bunTarget: "bun-darwin-arm64", executable: "warp-bridge" },
  { rustTarget: "x86_64-apple-darwin", bunTarget: "bun-darwin-x64", executable: "warp-bridge" },
  {
    rustTarget: "aarch64-unknown-linux-gnu",
    bunTarget: "bun-linux-arm64",
    executable: "warp-bridge",
  },
  {
    rustTarget: "x86_64-unknown-linux-gnu",
    bunTarget: "bun-linux-x64",
    executable: "warp-bridge",
  },
  {
    rustTarget: "aarch64-pc-windows-msvc",
    bunTarget: "bun-windows-arm64",
    executable: "warp-bridge.exe",
  },
  {
    rustTarget: "x86_64-pc-windows-msvc",
    bunTarget: "bun-windows-x64",
    executable: "warp-bridge.exe",
  },
];

export function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function expectRecord(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value;
}

function expectIdentity(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`${label} mismatch: expected ${expected}, found ${String(actual)}`);
  }
}

export function validateManifest(value, expected) {
  const manifest = expectRecord(value, "Bridge Artifact Manifest");
  expectIdentity(manifest.manifest_version, 1, "Manifest version");
  expectIdentity(manifest.bridge_version, expected.bridgeVersion, "Bridge version");
  expectIdentity(manifest.pi_version, expected.piVersion, "Pi version");
  expectIdentity(manifest.pi_source_revision, expected.piSourceRevision, "Pi source revision");
  expectIdentity(
    manifest.core_protocol_version,
    expected.coreProtocolVersion,
    "Core Protocol version",
  );
  expectIdentity(manifest.core_schema_hash, expected.coreSchemaHash, "Core Protocol schema hash");

  const artifacts = expectRecord(manifest.artifacts, "Manifest artifacts");
  const actualTargets = Object.keys(artifacts).sort();
  const requiredTargets = [...expected.requiredTargets].sort();
  if (
    actualTargets.length !== requiredTargets.length ||
    actualTargets.some((value, index) => value !== requiredTargets[index])
  ) {
    throw new Error(
      `Manifest target set mismatch: expected ${requiredTargets.join(", ")}, found ${actualTargets.join(", ")}`,
    );
  }

  for (const target of requiredTargets) {
    const artifact = expectRecord(artifacts[target], `Artifact for target ${target}`);
    if (typeof artifact.relative_path !== "string" || artifact.relative_path.length === 0) {
      throw new Error(`Artifact path for target ${target} must be non-empty`);
    }
    const normalizedPath = normalize(artifact.relative_path);
    if (
      isAbsolute(artifact.relative_path) ||
      normalizedPath === ".." ||
      normalizedPath.startsWith(`..${sep}`)
    ) {
      throw new Error(`Artifact path for target ${target} must stay within the artifact root`);
    }
    if (!Number.isSafeInteger(artifact.size) || artifact.size <= 0) {
      throw new Error(`Artifact size for target ${target} must be a positive integer`);
    }
    if (typeof artifact.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(artifact.sha256)) {
      throw new Error(`Artifact SHA-256 for target ${target} is invalid`);
    }
  }

  return manifest;
}

export function verifyArtifactBytes(manifest, target, bytes) {
  const artifact = manifest.artifacts[target];
  if (artifact === undefined) {
    throw new Error(`Bridge Artifact Manifest has no target ${target}`);
  }
  if (bytes.byteLength !== artifact.size) {
    throw new Error(
      `Bridge artifact size mismatch for target ${target}: expected ${artifact.size}, found ${bytes.byteLength}`,
    );
  }
  const actualHash = Buffer.from(sha256(bytes), "hex");
  const expectedHash = Buffer.from(artifact.sha256, "hex");
  if (!timingSafeEqual(actualHash, expectedHash)) {
    throw new Error(`Bridge artifact SHA-256 mismatch for target ${target}`);
  }
  return artifact;
}

export function verifyArtifactTarget(target, bytes) {
  const buffer = Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let actualTarget;
  if (buffer.length >= 12 && buffer.readUInt32LE(0) === 0xfeedfacf) {
    actualTarget =
      buffer.readUInt32LE(4) === 0x0100000c
        ? "aarch64-apple-darwin"
        : buffer.readUInt32LE(4) === 0x01000007
          ? "x86_64-apple-darwin"
          : undefined;
  } else if (
    buffer.length >= 20 &&
    buffer.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46]))
  ) {
    actualTarget =
      buffer.readUInt16LE(18) === 183
        ? "aarch64-unknown-linux-gnu"
        : buffer.readUInt16LE(18) === 62
          ? "x86_64-unknown-linux-gnu"
          : undefined;
  } else if (buffer.length >= 64 && buffer.subarray(0, 2).toString("ascii") === "MZ") {
    const peOffset = buffer.readUInt32LE(0x3c);
    if (
      peOffset + 6 <= buffer.length &&
      buffer.readUInt32LE(peOffset) === 0x00004550
    ) {
      actualTarget =
        buffer.readUInt16LE(peOffset + 4) === 0xaa64
          ? "aarch64-pc-windows-msvc"
          : buffer.readUInt16LE(peOffset + 4) === 0x8664
            ? "x86_64-pc-windows-msvc"
            : undefined;
    }
  }
  if (actualTarget !== target) {
    throw new Error(
      `Bridge artifact target mismatch: expected ${target}, found ${actualTarget ?? "unknown"}`,
    );
  }
}

export function assertArtifactRootOverrideAllowed(releasePackaging, artifactRootOverride) {
  if (releasePackaging && artifactRootOverride !== undefined && artifactRootOverride !== "") {
    throw new Error("A local Bridge artifact root override is forbidden for release packaging");
  }
}
