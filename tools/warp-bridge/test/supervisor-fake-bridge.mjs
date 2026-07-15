import { appendFileSync, existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createInterface } from "node:readline";

const [mode, observerDirectory] = process.argv.slice(2);
const delayedRestartMarker = join(observerDirectory, "delayed-restart");
const supportsTextRuns =
  mode === "text-runs" ||
  mode === "text-run-exit" ||
  mode === "text-run-cancel" ||
  mode === "text-run-hang-cancel" ||
  mode === "text-run-hang-sync";

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
    protocol_version: 2,
    core_schema_hash: "sha256:7a44caef7fc85b2719d1c3ae7f98bab98f221287a4de6541d6386d1f590c578c",
    bridge_version: "0.1.0-supervisor-fake",
    capabilities: [],
    prompt_version: "warp.v1",
  };
  process.stdout.write(`${JSON.stringify(hello)}\n`);

  let handshakeAccepted = false;
  let transcriptCandidate;
  let acceptedTranscript;
  let activeRun;
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

    if (supportsTextRuns && message.type === "transcript_sync_begin") {
      transcriptCandidate = {
        syncId: message.sync_id,
        conversationId: message.conversation_id,
        revision: message.revision,
        expectedItems: message.item_count,
        expectedBytes: message.total_bytes,
        receivedBytes: 0,
        items: [],
      };
      continue;
    }
    if (supportsTextRuns && message.type === "transcript_sync_item") {
      if (
        transcriptCandidate === undefined ||
        message.sync_id !== transcriptCandidate.syncId ||
        message.index !== transcriptCandidate.items.length
      ) {
        process.exit(31);
      }
      transcriptCandidate.items.push(message.item);
      transcriptCandidate.receivedBytes += Buffer.byteLength(line, "utf8");
      continue;
    }
    if (supportsTextRuns && message.type === "transcript_sync_commit") {
      if (
        transcriptCandidate === undefined ||
        message.sync_id !== transcriptCandidate.syncId ||
        transcriptCandidate.items.length !== transcriptCandidate.expectedItems ||
        transcriptCandidate.receivedBytes !== transcriptCandidate.expectedBytes
      ) {
        process.exit(32);
      }
      if (mode === "text-run-hang-sync") {
        continue;
      }
      acceptedTranscript = transcriptCandidate;
      transcriptCandidate = undefined;
      appendFileSync(
        join(observerDirectory, "accepted-transcripts.jsonl"),
        `${JSON.stringify(acceptedTranscript)}\n`,
      );
      process.stdout.write(
        `${JSON.stringify({
          type: "transcript_sync_result",
          sync_id: acceptedTranscript.syncId,
          status: "accepted",
          revision: acceptedTranscript.revision,
        })}\n`,
      );
      continue;
    }
    if (supportsTextRuns && message.type === "run_start") {
      if (
        acceptedTranscript === undefined ||
        message.conversation_id !== acceptedTranscript.conversationId ||
        message.transcript_revision !== acceptedTranscript.revision
      ) {
        process.exit(33);
      }
      activeRun = message;
      process.stdout.write(
        `${JSON.stringify({
          type: "run_status",
          conversation_id: message.conversation_id,
          run_id: message.run_id,
          status: "running",
        })}\n`,
      );
      if (mode === "text-run-cancel" || mode === "text-run-hang-cancel") {
        process.stdout.write(
          `${JSON.stringify({
            type: "text_delta",
            conversation_id: message.conversation_id,
            run_id: message.run_id,
            event_id: `event-${message.run_id}`,
            delta: "Partial output",
          })}\n`,
        );
        continue;
      }
      const text = message.run_id === "run-1" ? "Partial output" : "Completed output";
      process.stdout.write(
        `${JSON.stringify({
          type: "text_delta",
          conversation_id: message.conversation_id,
          run_id: message.run_id,
          event_id: `event-${message.run_id}`,
          delta: text,
        })}\n`,
      );
      if (mode === "text-run-exit") {
        await new Promise((resolve) => process.stdout.write("", resolve));
        process.exit(35);
      }
      if (message.run_id === "run-1") {
        process.stdout.write(
          `${JSON.stringify({
            type: "run_finished",
            conversation_id: message.conversation_id,
            run_id: message.run_id,
            outcome: "failed",
            error_code: "provider_transport_error",
            diagnostic_id: "diag-run-1",
          })}\n`,
        );
        activeRun = undefined;
      } else {
        process.stdout.write(
          `${JSON.stringify({
            type: "run_status",
            conversation_id: message.conversation_id,
            run_id: message.run_id,
            status: "waiting_for_commit",
          })}\n`,
        );
        process.stdout.write(
          `${JSON.stringify({
            type: "assistant_message_commit",
            conversation_id: message.conversation_id,
            run_id: message.run_id,
            event_id: `event-${message.run_id}`,
            commit_id: "commit-run-2",
            message_id: "assistant-run-2",
            expected_revision: message.transcript_revision,
            content: [{ type: "text", text }],
          })}\n`,
        );
      }
      continue;
    }
    if (supportsTextRuns && message.type === "commit_result") {
      if (
        activeRun?.run_id !== message.run_id ||
        message.commit_id !== "commit-run-2" ||
        message.revision !== activeRun.transcript_revision + 1
      ) {
        process.exit(34);
      }
      process.stdout.write(
        `${JSON.stringify({
          type: "run_finished",
          conversation_id: message.conversation_id,
          run_id: message.run_id,
          outcome: "completed",
        })}\n`,
      );
      activeRun = undefined;
      continue;
    }

    if (
      message.type === "run_cancel" &&
      mode !== "hang-cancel" &&
      mode !== "text-run-hang-cancel" &&
      mode !== "text-run-hang-sync"
    ) {
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
