import { spawn } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { createInterface } from "node:readline";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  assertArtifactRootOverrideAllowed,
  RELEASE_TARGETS,
  sha256,
  validateManifest,
  verifyArtifactBytes,
  verifyArtifactTarget,
} from "./artifact-manifest.mjs";

const VALUE_OPTIONS = new Set([
  "--target",
  "--manifest",
  "--artifact-root",
  "--copy-to",
  "--smoke-only",
]);
const BOOLEAN_OPTIONS = new Set(["--release", "--smoke"]);

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(root, "../..");

function argumentValue(args, name) {
  const index = args.indexOf(name);
  if (index === -1) {
    return undefined;
  }
  const value = args[index + 1];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function expectedIdentity() {
  const packageJson = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
  const provenance = JSON.parse(
    readFileSync(resolve(repositoryRoot, "third_party/pi/provenance.json"), "utf8"),
  );
  const piVersions = new Set(provenance.packages.map((entry) => entry.version));
  if (piVersions.size !== 1) {
    throw new Error("Vendored Pi packages must use one exact version");
  }
  return {
    bridgeVersion: packageJson.version,
    piVersion: [...piVersions][0],
    piSourceRevision: provenance.commit,
    coreProtocolVersion: 2,
    coreSchemaHash: `sha256:${sha256(readFileSync(resolve(root, "protocol/core-v2.schema.json")))}`,
    requiredTargets: RELEASE_TARGETS.map((target) => target.rustTarget),
  };
}

async function smokeHandshake(artifactPath, identity) {
  const child = spawn(artifactPath, [], {
    cwd: dirname(artifactPath),
    env:
      process.platform === "win32" && process.env.SystemRoot !== undefined
        ? { SystemRoot: process.env.SystemRoot }
        : {},
    stdio: ["pipe", "pipe", "ignore"],
  });
  const lines = createInterface({ input: child.stdout });
  const startupTimeout = setTimeout(() => child.kill(), 5_000);
  try {
    const next = await lines[Symbol.asyncIterator]().next();
    clearTimeout(startupTimeout);
    if (next.done) {
      throw new Error("Bridge artifact exited before its compatibility handshake");
    }
    const hello = JSON.parse(next.value);
    if (
      hello.type !== "bridge_hello" ||
      hello.protocol_version !== identity.coreProtocolVersion ||
      hello.core_schema_hash !== identity.coreSchemaHash ||
      hello.bridge_version !== identity.bridgeVersion
    ) {
      throw new Error("Bridge artifact compatibility handshake does not match the manifest");
    }
    child.stdin.write(
      `${JSON.stringify({
        type: "handshake_result",
        status: "accepted",
        max_frame_bytes: 1_048_576,
        max_transcript_bytes: 16_777_216,
      })}\n`,
    );
    child.stdin.end("{}\n");
    const exitCode = await new Promise((resolveExit, rejectExit) => {
      const timeout = setTimeout(() => {
        child.kill();
        rejectExit(new Error("Bridge artifact conformance smoke timed out"));
      }, 5_000);
      child.once("error", (error) => {
        clearTimeout(timeout);
        rejectExit(error);
      });
      child.once("exit", (code) => {
        clearTimeout(timeout);
        resolveExit(code);
      });
    });
    if (exitCode === 0) {
      throw new Error("Bridge artifact accepted an invalid Core Protocol frame");
    }
  } finally {
    clearTimeout(startupTimeout);
    lines.close();
    child.kill();
  }
}

async function smokeConformanceFixtures(artifactPath) {
  const fixturesRoot = resolve(root, "protocol/fixtures");
  const fixtures = ["valid", "invalid"].flatMap((kind) =>
    readdirSync(resolve(fixturesRoot, kind))
      .sort()
      .flatMap((name) =>
        readFileSync(resolve(fixturesRoot, kind, name), "utf8")
          .split("\n")
          .filter((line) => line.length > 0)
          .map((line) => ({ expected: kind, line, name })),
      ),
  );
  const child = spawn(artifactPath, ["--protocol-conformance"], {
    cwd: dirname(artifactPath),
    env:
      process.platform === "win32" && process.env.SystemRoot !== undefined
        ? { SystemRoot: process.env.SystemRoot }
        : {},
    stdio: ["pipe", "pipe", "ignore"],
  });
  const output = [];
  const readOutput = (async () => {
    for await (const line of createInterface({ input: child.stdout })) {
      output.push(line);
    }
  })();
  child.stdin.end(`${fixtures.map((fixture) => fixture.line).join("\n")}\n`);
  const exitCode = await new Promise((resolveExit, rejectExit) => {
    const timeout = setTimeout(() => {
      child.kill();
      rejectExit(new Error("Bridge artifact fixture conformance smoke timed out"));
    }, 10_000);
    child.once("error", (error) => {
      clearTimeout(timeout);
      rejectExit(error);
    });
    child.once("exit", (code) => {
      clearTimeout(timeout);
      resolveExit(code);
    });
  });
  await readOutput;
  if (exitCode !== 0 || output.length !== fixtures.length) {
    throw new Error("Bridge artifact fixture conformance process failed");
  }
  fixtures.forEach((fixture, index) => {
    if (output[index] !== fixture.expected) {
      throw new Error(`Bridge artifact fixture conformance mismatch: ${fixture.name}`);
    }
  });
}

async function smokeArtifact(artifactPath, identity) {
  await smokeHandshake(artifactPath, identity);
  await smokeConformanceFixtures(artifactPath);
}

async function main() {
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (BOOLEAN_OPTIONS.has(argument)) {
      continue;
    }
    if (!VALUE_OPTIONS.has(argument)) {
      throw new Error(`Unknown option: ${argument}`);
    }
    index += 1;
    if (args[index] === undefined || args[index].startsWith("--")) {
      throw new Error(`${argument} requires a value`);
    }
  }
  const smokeOnlyPath = argumentValue(args, "--smoke-only");
  const identity = expectedIdentity();
  if (smokeOnlyPath !== undefined) {
    await smokeArtifact(resolve(smokeOnlyPath), identity);
    console.log(resolve(smokeOnlyPath));
    return;
  }
  const target = argumentValue(args, "--target");
  if (target === undefined) {
    throw new Error("--target is required");
  }
  const manifestPath = resolve(
    argumentValue(args, "--manifest") ??
      resolve(repositoryRoot, "resources/bundled/agent-runtime/bridge-manifest.json"),
  );
  const releasePackaging = args.includes("--release");
  const explicitArtifactRoot = argumentValue(args, "--artifact-root");
  const environmentArtifactRoot = process.env.WARP_PI_BRIDGE_ARTIFACT_ROOT;
  const artifactRootOverride = explicitArtifactRoot ?? environmentArtifactRoot;
  assertArtifactRootOverrideAllowed(releasePackaging, artifactRootOverride);
  const artifactRoot = resolve(artifactRootOverride ?? resolve(root, "dist/release"));
  const manifest = validateManifest(JSON.parse(readFileSync(manifestPath, "utf8")), identity);
  const artifact = manifest.artifacts[target];
  if (artifact === undefined) {
    throw new Error(`Bridge Artifact Manifest has no target ${target}`);
  }
  const artifactPath = resolve(artifactRoot, artifact.relative_path);
  const bytes = readFileSync(artifactPath);
  verifyArtifactBytes(manifest, target, bytes);
  verifyArtifactTarget(target, bytes);

  if (args.includes("--smoke")) {
    await smokeArtifact(artifactPath, identity);
  }

  const copyTo = argumentValue(args, "--copy-to");
  if (copyTo !== undefined) {
    const destination = resolve(copyTo);
    mkdirSync(destination, { recursive: true });
    rmSync(resolve(destination, "warp-bridge"), { force: true });
    rmSync(resolve(destination, "warp-bridge.exe"), { force: true });
    const destinationArtifact = resolve(destination, basename(artifactPath));
    copyFileSync(artifactPath, destinationArtifact);
    if (process.platform !== "win32") {
      chmodSync(destinationArtifact, 0o755);
    }
    copyFileSync(manifestPath, resolve(destination, "bridge-manifest.json"));
    verifyArtifactBytes(manifest, target, readFileSync(destinationArtifact));
    console.log(destinationArtifact);
  } else {
    console.log(artifactPath);
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
