import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";

const hello = {
  type: "bridge_hello",
  protocol_version: 1,
  core_schema_hash: "sha256:afb439d8518d3ae8f2fb0f314845036f0c673c0c96eb7e849e0e71bdfd87600e",
  bridge_version: "0.1.0-fake",
  capabilities: [],
  prompt_version: "warp.v1",
};

process.stdout.write(`${JSON.stringify(hello)}\n`);

let handshakeAccepted = false;
for await (const line of createInterface({ input: process.stdin })) {
  try {
    const message = JSON.parse(line);
    handshakeAccepted =
      message.type === "handshake_result" && message.status === "accepted";
  } catch {
    handshakeAccepted = false;
  }
  break;
}

if (!handshakeAccepted || process.argv.length !== 3) {
  process.exitCode = 2;
} else {
  const script = readFileSync(process.argv[2], "utf8");
  for (const line of script.split("\n")) {
    if (line.length > 0) {
      process.stdout.write(`${line}\n`);
    }
  }
}
