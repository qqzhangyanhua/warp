import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

import {
  RELEASE_TARGETS,
  sha256,
  validateManifest,
  verifyArtifactBytes,
  verifyArtifactTarget,
} from "./artifact-manifest.mjs";

const args = process.argv.slice(2);
if (args.some((argument) => argument !== "--update-manifest")) {
  throw new Error(`Unknown option: ${args.find((argument) => argument !== "--update-manifest")}`);
}
const updateManifest = args.includes("--update-manifest");

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const bunExecutable = createRequire(import.meta.url).resolve("bun/bin/bun.exe");
const repositoryRoot = resolve(root, "../..");
const outputRoot = resolve(root, "dist/release");
const manifestPath = resolve(
  repositoryRoot,
  "resources/bundled/agent-runtime/bridge-manifest.json",
);
const packageJson = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
const provenance = JSON.parse(
  readFileSync(resolve(repositoryRoot, "third_party/pi/provenance.json"), "utf8"),
);
const coreSchema = readFileSync(resolve(root, "protocol/core-v2.schema.json"));
const artifacts = {};

for (const target of RELEASE_TARGETS) {
  const targetDirectory = resolve(outputRoot, target.rustTarget);
  const artifactPath = resolve(targetDirectory, target.executable);
  mkdirSync(targetDirectory, { recursive: true });
  const result = spawnSync(
    bunExecutable,
    [
      "build",
      "src/main.ts",
      "--compile",
      `--target=${target.bunTarget}`,
      `--outfile=${artifactPath}`,
    ],
    {
      cwd: root,
      env: { ...process.env, NODE_PATH: "" },
      stdio: "inherit",
    },
  );
  if (result.status !== 0) {
    throw new Error(`Standalone Bridge build failed for ${target.rustTarget}`);
  }
  const bytes = readFileSync(artifactPath);
  verifyArtifactTarget(target.rustTarget, bytes);
  artifacts[target.rustTarget] = {
    relative_path: `${target.rustTarget}/${target.executable}`,
    size: bytes.byteLength,
    sha256: sha256(bytes),
  };
}

const piVersion = provenance.packages[0]?.version;
if (
  typeof piVersion !== "string" ||
  provenance.packages.some((entry) => entry.version !== piVersion)
) {
  throw new Error("Vendored Pi packages must use one exact version");
}

const manifest = {
  manifest_version: 1,
  bridge_version: packageJson.version,
  pi_version: piVersion,
  pi_source_revision: provenance.commit,
  core_protocol_version: 2,
  core_schema_hash: `sha256:${sha256(coreSchema)}`,
  artifacts,
};
mkdirSync(dirname(manifestPath), { recursive: true });
if (updateManifest) {
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
} else {
  const checkedManifest = validateManifest(
    JSON.parse(readFileSync(manifestPath, "utf8")),
    {
      bridgeVersion: manifest.bridge_version,
      piVersion: manifest.pi_version,
      piSourceRevision: manifest.pi_source_revision,
      coreProtocolVersion: manifest.core_protocol_version,
      coreSchemaHash: manifest.core_schema_hash,
      requiredTargets: RELEASE_TARGETS.map((target) => target.rustTarget),
    },
  );
  for (const target of RELEASE_TARGETS) {
    verifyArtifactBytes(
      checkedManifest,
      target.rustTarget,
      readFileSync(resolve(outputRoot, checkedManifest.artifacts[target.rustTarget].relative_path)),
    );
  }
}
const licenseResult = spawnSync(
  process.execPath,
  [
    resolve(root, "scripts/generate-release-licenses.mjs"),
    resolve(dirname(manifestPath), "THIRD_PARTY_LICENSES.txt"),
  ],
  { cwd: root, stdio: "inherit" },
);
if (licenseResult.status !== 0) {
  throw new Error("Bridge production license generation failed");
}
console.log(
  `${updateManifest ? "Updated" : "Verified"} Bridge Artifact Manifest at ${manifestPath}`,
);
