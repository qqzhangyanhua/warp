import { describe, expect, test } from "vitest";

import {
  ProviderTransportError,
  createProviderFetch,
  type ProviderTransportFailure,
} from "../src/provider-transport.js";

describe("Provider transport", () => {
  test("network deny permits only the configured Provider origin", async () => {
    const requestedUrls: string[] = [];
    const providerFetch = createProviderFetch(
      "https://provider.example",
      3,
      async (input) => {
        requestedUrls.push(new Request(input).url);
        return new Response("ok", { status: 200 });
      },
    );

    await expect(
      providerFetch("https://app.warp.dev/api/telemetry", {
        headers: { authorization: "Bearer provider-secret" },
      }),
    ).rejects.toBeInstanceOf(ProviderTransportError);
    await expect(
      providerFetch("https://provider.example/v1/chat/completions"),
    ).resolves.toHaveProperty("status", 200);

    expect(requestedUrls).toEqual([
      "https://provider.example/v1/chat/completions",
    ]);
  });

  test("follows at most three same-origin redirects", async () => {
    const requestedUrls: string[] = [];
    const fetchImpl: typeof fetch = async (input) => {
      const request = new Request(input);
      requestedUrls.push(request.url);
      const redirectNumber = requestedUrls.length;
      if (redirectNumber <= 3) {
        return new Response(null, {
          status: 307,
          headers: { location: `/redirect-${redirectNumber}` },
        });
      }
      return new Response("ok", { status: 200 });
    };
    const providerFetch = createProviderFetch(
      "https://provider.example",
      3,
      fetchImpl,
    );

    const response = await providerFetch("https://provider.example/start", {
      headers: { authorization: "Bearer secret" },
    });

    expect(response.status).toBe(200);
    expect(requestedUrls).toEqual([
      "https://provider.example/start",
      "https://provider.example/redirect-1",
      "https://provider.example/redirect-2",
      "https://provider.example/redirect-3",
    ]);
  });

  test("rejects a cross-origin redirect before forwarding credentials", async () => {
    const requests: Request[] = [];
    const failures: ProviderTransportFailure[] = [];
    const fetchImpl: typeof fetch = async (input) => {
      const request = new Request(input);
      requests.push(request);
      return new Response(null, {
        status: 307,
        headers: { location: "https://attacker.example/collect" },
      });
    };
    const providerFetch = createProviderFetch(
      "https://provider.example",
      3,
      fetchImpl,
      (failure) => failures.push(failure),
    );

    await expect(
      providerFetch("https://provider.example/start", {
        headers: { authorization: "Bearer secret" },
      }),
    ).rejects.toBeInstanceOf(ProviderTransportError);
    expect(requests).toHaveLength(1);
    expect(requests[0]?.url).toBe("https://provider.example/start");
    expect(failures).toEqual([{ type: "redirect_not_allowed" }]);
  });

  test("rejects redirects beyond the configured hop limit", async () => {
    const failures: ProviderTransportFailure[] = [];
    const fetchImpl: typeof fetch = async () =>
      new Response(null, { status: 307, headers: { location: "/again" } });
    const providerFetch = createProviderFetch(
      "https://provider.example",
      3,
      fetchImpl,
      (failure) => failures.push(failure),
    );

    await expect(providerFetch("https://provider.example/start")).rejects.toBeInstanceOf(
      ProviderTransportError,
    );
    expect(failures).toEqual([{ type: "redirect_not_allowed" }]);
  });

  test("reports typed HTTP failures without exposing the response body", async () => {
    const failures: ProviderTransportFailure[] = [];
    const providerFetch = createProviderFetch(
      "https://provider.example",
      3,
      async () => new Response("sensitive provider detail", { status: 503 }),
      (failure) => failures.push(failure),
    );

    const response = await providerFetch("https://provider.example/start");

    expect(response.status).toBe(503);
    expect(failures).toEqual([{ type: "http", status: 503 }]);
    expect(JSON.stringify(failures)).not.toContain("sensitive provider detail");
  });

  test("reports transport failures while consuming the response body", async () => {
    const failures: ProviderTransportFailure[] = [];
    const providerFetch = createProviderFetch(
      "https://provider.example",
      3,
      async () =>
        new Response(
          new ReadableStream({
            pull(controller) {
              controller.error(new Error("recognizable provider detail"));
            },
          }),
          { status: 200 },
        ),
      (failure) => failures.push(failure),
    );

    const response = await providerFetch("https://provider.example/start");

    await expect(response.text()).rejects.toBeInstanceOf(ProviderTransportError);
    expect(failures).toEqual([{ type: "transport" }]);
    expect(JSON.stringify(failures)).not.toContain("recognizable provider detail");
  });
});
