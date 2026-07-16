import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const piLicense = normalizeLicenseText(
  readFileSync(resolve(repositoryRoot, "third_party/pi/LICENSE"), "utf8"),
);

const [outputArgument] = process.argv.slice(2);
if (outputArgument === undefined) {
  throw new Error("Usage: generate-release-licenses.mjs <output>");
}

const result = spawnSync("pnpm", ["licenses", "list", "--prod", "--json"], {
  encoding: "utf8",
  maxBuffer: 8 * 1024 * 1024,
});
if (result.status !== 0) {
  throw new Error(result.stderr || "Unable to enumerate Bridge production licenses");
}

const licenseGroups = JSON.parse(result.stdout);
const packages = Object.entries(licenseGroups)
  .flatMap(([license, entries]) => entries.map((entry) => ({ ...entry, license })))
  .filter(
    (entry, index, entries) =>
      entries.findIndex(
        (candidate) =>
          candidate.name === entry.name &&
          candidate.versions.join(",") === entry.versions.join(","),
      ) === index,
  )
  .sort((left, right) => left.name.localeCompare(right.name));

function licenseFile(packagePath) {
  return readdirSync(packagePath, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .find((name) => /^(licen[cs]e|copying|notice)([-.]|$)/i.test(name));
}

function mitLicense(author) {
  return `Copyright (c) ${author}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;
}

function normalizeLicenseText(text) {
  return text
    .replaceAll("\r\n", "\n")
    .split("\n")
    .map((line) => line.trimEnd())
    .join("\n")
    .trim();
}

function installedLicenseText(packageName) {
  const entry = packages.find((candidate) => candidate.name === packageName);
  if (entry === undefined) {
    throw new Error(`License source package is not installed: ${packageName}`);
  }
  const file = licenseFile(entry.paths[0]);
  if (file === undefined) {
    throw new Error(`License source package has no license file: ${packageName}`);
  }
  return normalizeLicenseText(
    readFileSync(resolve(entry.paths[0], file), "utf8"),
  );
}

const awsSdkApacheLicense = installedLicenseText(
  "@aws-sdk/credential-provider-web-identity",
);
const awsSdkPackagesWithoutLicenseFiles = new Set([
  "@aws-sdk/credential-provider-http",
  "@aws-sdk/credential-provider-login",
  "@aws-sdk/nested-clients",
]);

function verifiedMissingLicenseText(entry) {
  if (
    entry.name === "data-uri-to-buffer" &&
    entry.license === "MIT" &&
    entry.author === "Nathan Rajlich"
  ) {
    return mitLicense(entry.author);
  }
  if (
    awsSdkPackagesWithoutLicenseFiles.has(entry.name) &&
    entry.license === "Apache-2.0" &&
    entry.homepage?.startsWith("https://github.com/aws/aws-sdk-js-v3/")
  ) {
    return awsSdkApacheLicense;
  }
  throw new Error(
    `No verified license text available for ${entry.name} (${entry.license})`,
  );
}

const sections = packages.map((entry) => {
  const packagePath = entry.paths[0];
  const file = licenseFile(packagePath);
  const text =
    entry.name.startsWith("@earendil-works/pi-")
      ? piLicense
      : file === undefined
        ? verifiedMissingLicenseText(entry)
        : normalizeLicenseText(readFileSync(resolve(packagePath, file), "utf8"));
  return [
    `${entry.name}@${entry.versions.join(", ")} (${entry.license})`,
    entry.homepage ?? "",
    "-".repeat(80),
    text,
  ]
    .filter((line) => line !== "")
    .join("\n");
});

writeFileSync(
  resolve(outputArgument),
  `Agent Runtime Bridge production dependencies\n\n${sections.join("\n\n")}\n`,
);
console.log(`Wrote Bridge licenses to ${basename(outputArgument)}`);
