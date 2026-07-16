import { runTextBridgeProcess } from "./bridge-process.js";
import { runProtocolConformanceProcess } from "./conformance-process.js";

try {
  if (process.argv.includes("--protocol-conformance")) {
    await runProtocolConformanceProcess();
  } else {
    await runTextBridgeProcess();
  }
} catch {
  process.exitCode = 1;
}
