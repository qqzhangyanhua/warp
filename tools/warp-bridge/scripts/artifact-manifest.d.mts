export interface BridgeArtifact {
  relative_path: string;
  size: number;
  sha256: string;
}

export interface BridgeArtifactManifest {
  manifest_version: 1;
  bridge_version: string;
  pi_version: string;
  pi_source_revision: string;
  core_protocol_version: number;
  core_schema_hash: string;
  artifacts: Record<string, BridgeArtifact>;
}

export interface ExpectedArtifactIdentity {
  bridgeVersion: string;
  piVersion: string;
  piSourceRevision: string;
  coreProtocolVersion: number;
  coreSchemaHash: string;
  requiredTargets: readonly string[];
}

export const RELEASE_TARGETS: readonly {
  rustTarget: string;
  bunTarget: string;
  executable: string;
}[];

export function sha256(bytes: Uint8Array): string;
export function validateManifest(
  value: unknown,
  expected: ExpectedArtifactIdentity,
): BridgeArtifactManifest;
export function verifyArtifactBytes(
  manifest: BridgeArtifactManifest,
  target: string,
  bytes: Uint8Array,
): BridgeArtifact;
export function verifyArtifactTarget(target: string, bytes: Uint8Array): void;
export function assertArtifactRootOverrideAllowed(
  releasePackaging: boolean,
  artifactRootOverride: string | undefined,
): void;
