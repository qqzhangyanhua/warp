import { createInterface } from "node:readline";

import { parseProtocolLine } from "./protocol.js";

export async function runProtocolConformanceProcess(
  input: NodeJS.ReadableStream = process.stdin,
  output: NodeJS.WritableStream = process.stdout,
): Promise<void> {
  for await (const line of createInterface({ input })) {
    try {
      parseProtocolLine(line);
      output.write("valid\n");
    } catch {
      output.write("invalid\n");
    }
  }
}
