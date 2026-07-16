import {
  closeSync,
  fstatSync,
  openSync,
  readSync,
  writeSync,
} from "node:fs";
import { resolve } from "node:path";

const FAT_MAGIC = 0xcafebabe;
const MH_MAGIC_64 = 0xfeedfacf;
const CPU_TYPE_X86_64 = 0x01000007;
const CPU_TYPE_ARM64 = 0x0100000c;
const ALIGNMENT_POWER = 14;
const ALIGNMENT = 2 ** ALIGNMENT_POWER;

function alignedOffset(offset) {
  return Math.ceil(offset / ALIGNMENT) * ALIGNMENT;
}

function thinArchitecture(path, expectedCpuType) {
  const fd = openSync(path, "r");
  try {
    const header = Buffer.alloc(12);
    if (readSync(fd, header, 0, header.length, 0) !== header.length) {
      throw new Error(`Mach-O header is truncated: ${path}`);
    }
    if (header.readUInt32LE(0) !== MH_MAGIC_64) {
      throw new Error(`Expected a thin 64-bit Mach-O artifact: ${path}`);
    }
    const cpuType = header.readUInt32LE(4);
    if (cpuType !== expectedCpuType) {
      throw new Error(`Unexpected Mach-O architecture: ${path}`);
    }
    return {
      path,
      cpuType,
      cpuSubtype: header.readUInt32LE(8),
      size: fstatSync(fd).size,
    };
  } finally {
    closeSync(fd);
  }
}

function copyAt(sourcePath, destinationFd, destinationOffset) {
  const sourceFd = openSync(sourcePath, "r");
  const buffer = Buffer.alloc(1024 * 1024);
  let sourceOffset = 0;
  try {
    while (true) {
      const bytesRead = readSync(sourceFd, buffer, 0, buffer.length, sourceOffset);
      if (bytesRead === 0) {
        return;
      }
      writeSync(destinationFd, buffer, 0, bytesRead, destinationOffset + sourceOffset);
      sourceOffset += bytesRead;
    }
  } finally {
    closeSync(sourceFd);
  }
}

const [outputArgument, armArgument, intelArgument] = process.argv.slice(2);
if (outputArgument === undefined || armArgument === undefined || intelArgument === undefined) {
  throw new Error("Usage: create-universal-macos.mjs <output> <arm64> <x86_64>");
}

const outputPath = resolve(outputArgument);
const architectures = [
  thinArchitecture(resolve(armArgument), CPU_TYPE_ARM64),
  thinArchitecture(resolve(intelArgument), CPU_TYPE_X86_64),
];
let nextOffset = alignedOffset(8 + architectures.length * 20);
for (const architecture of architectures) {
  architecture.offset = nextOffset;
  nextOffset = alignedOffset(nextOffset + architecture.size);
}

const fatHeader = Buffer.alloc(8 + architectures.length * 20);
fatHeader.writeUInt32BE(FAT_MAGIC, 0);
fatHeader.writeUInt32BE(architectures.length, 4);
architectures.forEach((architecture, index) => {
  const offset = 8 + index * 20;
  fatHeader.writeUInt32BE(architecture.cpuType, offset);
  fatHeader.writeUInt32BE(architecture.cpuSubtype, offset + 4);
  fatHeader.writeUInt32BE(architecture.offset, offset + 8);
  fatHeader.writeUInt32BE(architecture.size, offset + 12);
  fatHeader.writeUInt32BE(ALIGNMENT_POWER, offset + 16);
});

const outputFd = openSync(outputPath, "w", 0o755);
try {
  writeSync(outputFd, fatHeader, 0, fatHeader.length, 0);
  for (const architecture of architectures) {
    copyAt(architecture.path, outputFd, architecture.offset);
  }
} finally {
  closeSync(outputFd);
}
