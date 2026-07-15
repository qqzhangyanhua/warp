import { runTextBridgeProcess } from "./bridge-process.js";

try {
  await runTextBridgeProcess();
} catch {
  process.exitCode = 1;
}
