import { describe, expect, test } from "vitest";

import { PiTextRuntime, type TextRunEvent } from "../src/text-runtime.js";
import {
  idSequence,
  runStart,
  scriptedStream,
  successfulCallbacks,
  transcript,
} from "./text-runtime-test-helpers.js";

describe("Pi text runtime transport failures", () => {
  test("retries a response-body transport failure before model output", async () => {
    let attempts = 0;
    const runtime = new PiTextRuntime({
      stream: () => {
        attempts += 1;
        return attempts === 1
          ? scriptedStream([
              {
                type: "error",
                errorMessage: "socket closed",
                requestProvider: true,
                consumeProviderBody: true,
              },
            ])
          : scriptedStream([{ type: "text", text: "Recovered." }]);
      },
      createId: idSequence(),
    });
    const events: TextRunEvent[] = [];
    const previousFetch = globalThis.fetch;
    globalThis.fetch = async () =>
      new Response(
        new ReadableStream({
          pull(controller) {
            controller.error(new Error("recognizable provider detail"));
          },
        }),
        { status: 200 },
      );

    try {
      await runtime.run(transcript(), runStart(), {
        ...successfulCallbacks(),
        emit: (event) => {
          events.push(event);
        },
      });
    } finally {
      globalThis.fetch = previousFetch;
    }

    expect(attempts).toBe(2);
    expect(events.at(-1)).toMatchObject({
      type: "run_finished",
      outcome: "completed",
    });
  });
});
