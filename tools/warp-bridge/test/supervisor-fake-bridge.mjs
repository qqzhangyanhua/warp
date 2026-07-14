import { appendFileSync, existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createInterface } from "node:readline";

const [mode, observerDirectory] = process.argv.slice(2);
const delayedRestartMarker = join(observerDirectory, "delayed-restart");

if (mode === "hang-handshake") {
  process.stdin.once("data", () => {
    writeFileSync(join(observerDirectory, "pre-handshake-stdin"), "observed");
  });
  setInterval(() => {}, 1_000);
} else {
  if (mode === "exit-then-delay-next-handshake" && existsSync(delayedRestartMarker)) {
    await new Promise((resolve) => setTimeout(resolve, 150));
  }
  const hello = {
    type: "bridge_hello",
    protocol_version: 1,
    core_schema_hash: "sha256:b0c4c909ff976b69930e51cb6fb60e12e2e0421992f2e7a69520963d1c95914c",
    bridge_version: "0.1.0-supervisor-fake",
    capabilities: [],
    prompt_version: "warp.v1",
  };
  process.stdout.write(`${JSON.stringify(hello)}\n`);

  let handshakeAccepted = false;
  for await (const line of createInterface({ input: process.stdin })) {
    const message = JSON.parse(line);
    if (!handshakeAccepted) {
      handshakeAccepted = message.type === "handshake_result" && message.status === "accepted";
      if (!handshakeAccepted) {
        process.exitCode = 2;
        break;
      }
      appendFileSync(
        join(observerDirectory, "launches.jsonl"),
        `${JSON.stringify({
          pid: process.pid,
          cwd: process.cwd(),
          environment_keys: Object.keys(process.env).sort(),
        })}\n`,
      );
      const exitMarker = join(observerDirectory, "first-launch-exited");
      if (mode === "exit-first-launch" && !existsSync(exitMarker)) {
        writeFileSync(exitMarker, "exited");
        setTimeout(() => process.exit(23), 50);
      }
      if (mode === "exit-then-delay-next-handshake" && !existsSync(delayedRestartMarker)) {
        writeFileSync(delayedRestartMarker, "delayed");
        process.exit(23);
      }
      if (mode === "stderr-burst") {
        process.stderr.write("sensitive stderr".repeat(1_024));
      }
      continue;
    }

    if (message.type === "run_cancel" && mode !== "hang-cancel") {
      process.stdout.write(
        `${JSON.stringify({
          type: "run_finished",
          conversation_id: message.conversation_id,
          run_id: message.run_id,
          outcome: "cancelled",
        })}\n`,
      );
    }
  }
}
