import { readFileSync, readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vitest";

import { parseProtocolLine } from "../src/protocol.js";

const fixturesRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../protocol/fixtures");

function fixtureLines(kind: "valid" | "invalid"): Array<{ name: string; line: string }> {
  const directory = resolve(fixturesRoot, kind);
  return readdirSync(directory)
    .sort()
    .flatMap((name) =>
      readFileSync(resolve(directory, name), "utf8")
        .split("\n")
        .filter((line) => line.length > 0)
        .map((line, index) => ({ name: `${name}:${index + 1}`, line })),
    );
}

describe("Bridge Protocol conformance fixtures", () => {
  for (const fixture of fixtureLines("valid")) {
    test(`accepts ${fixture.name}`, () => {
      expect(parseProtocolLine(fixture.line)).toBeDefined();
    });
  }

  for (const fixture of fixtureLines("invalid")) {
    test(`rejects ${fixture.name}`, () => {
      expect(() => parseProtocolLine(fixture.line)).toThrow();
    });
  }
});
