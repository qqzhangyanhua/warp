import { createInterface } from "node:readline";

import { BridgeTextRuntimeSession } from "./bridge-runtime.js";
import type { ProtocolMessage } from "./protocol.js";
import type { TextRuntime } from "./text-runtime.js";

export async function runTextBridgeProcess(
  input: NodeJS.ReadableStream = process.stdin,
  output: NodeJS.WritableStream = process.stdout,
  runtime?: TextRuntime,
): Promise<void> {
  const bridge = new BridgeTextRuntimeSession(
    (message) => writeMessage(output, message),
    runtime,
  );
  await writeMessage(output, bridge.hello());

  try {
    for await (const line of createInterface({ input })) {
      bridge.receive(line);
    }
  } finally {
    bridge.close();
  }
  await bridge.waitForIdle();
}

function writeMessage(
  output: NodeJS.WritableStream,
  message: ProtocolMessage,
): Promise<void> {
  return new Promise((resolve, reject) => {
    output.write(`${JSON.stringify(message)}\n`, (error?: Error | null) => {
      if (error === undefined || error === null) {
        resolve();
      } else {
        reject(error);
      }
    });
  });
}
