const REDIRECT_STATUSES = new Set([301, 302, 303, 307, 308]);

export class ProviderTransportError extends Error {
  constructor() {
    super("Provider transport policy rejected the request");
    this.name = "ProviderTransportError";
  }
}

export type ProviderTransportFailure =
  | { type: "http"; status: number }
  | { type: "transport" }
  | { type: "redirect_not_allowed" };

export function createProviderFetch(
  providerOrigin: string,
  maxRedirects: number,
  fetchImpl: typeof fetch = globalThis.fetch,
  onFailure: (failure: ProviderTransportFailure) => void = () => {},
): typeof fetch {
  const allowedOrigin = parseHttpOrigin(providerOrigin);
  if (!Number.isInteger(maxRedirects) || maxRedirects < 0 || maxRedirects > 3) {
    throw new ProviderTransportError();
  }

  return async (input, init) => {
    let request = new Request(input, init);
    let redirects = 0;

    while (true) {
      if (new URL(request.url).origin !== allowedOrigin) {
        throw new ProviderTransportError();
      }
      const replayable = request.clone();
      let response: Response;
      try {
        response = await fetchImpl(request, { redirect: "manual" });
      } catch {
        onFailure({ type: "transport" });
        throw new ProviderTransportError();
      }
      if (!REDIRECT_STATUSES.has(response.status)) {
        if (!response.ok) {
          onFailure({ type: "http", status: response.status });
        }
        return monitorResponseBody(response, onFailure);
      }
      const location = response.headers.get("location");
      if (location === null) {
        throw new ProviderTransportError();
      }
      if (redirects >= maxRedirects) {
        onFailure({ type: "redirect_not_allowed" });
        throw new ProviderTransportError();
      }
      const redirectUrl = new URL(location, request.url);
      if (redirectUrl.origin !== allowedOrigin) {
        onFailure({ type: "redirect_not_allowed" });
        throw new ProviderTransportError();
      }

      redirects += 1;
      request = redirectedRequest(response.status, redirectUrl, replayable);
    }
  };
}

function monitorResponseBody(
  response: Response,
  onFailure: (failure: ProviderTransportFailure) => void,
): Response {
  if (response.body === null) {
    return response;
  }
  const reader = response.body.getReader();
  let failureReported = false;
  const body = new ReadableStream<Uint8Array>({
    async pull(controller) {
      try {
        const next = await reader.read();
        if (next.done) {
          controller.close();
        } else {
          controller.enqueue(next.value);
        }
      } catch {
        if (!failureReported) {
          failureReported = true;
          onFailure({ type: "transport" });
        }
        controller.error(new ProviderTransportError());
      }
    },
    cancel(reason) {
      return reader.cancel(reason);
    },
  });
  return new Response(body, {
    status: response.status,
    statusText: response.statusText,
    headers: response.headers,
  });
}

function parseHttpOrigin(value: string): string {
  try {
    const url = new URL(value);
    if ((url.protocol !== "http:" && url.protocol !== "https:") || url.origin !== value) {
      throw new ProviderTransportError();
    }
    return url.origin;
  } catch {
    throw new ProviderTransportError();
  }
}

function redirectedRequest(status: number, url: URL, previous: Request): Request {
  if (status === 303 || ((status === 301 || status === 302) && previous.method === "POST")) {
    const headers = new Headers(previous.headers);
    headers.delete("content-length");
    headers.delete("content-type");
    return new Request(url, { method: "GET", headers, signal: previous.signal });
  }
  return new Request(url, previous);
}
