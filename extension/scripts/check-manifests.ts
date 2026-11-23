#!/usr/bin/env bun
/**
 * Per-browser manifest assertions on the WXT output.
 *
 * Throws unless the generated manifests match the contract the rest of the
 * toolchain relies on: Chrome is MV3 with `_execute_action`, Firefox is MV2
 * with `browser_action` and `_execute_browser_action`, and the Gecko id and
 * minimum version are ours.
 */

import { readFileSync } from "node:fs";

const EXPECTED_VERSION = "0.1.0";
const GECK_ID = "kdeconnect-browser-reborn@muntasirszn.github.io";

function load(path: string): Record<string, unknown> {
  return JSON.parse(readFileSync(path, "utf8")) as Record<string, unknown>;
}

function assert(condition: unknown, message: string): void {
  if (!condition) {
    throw new Error(`manifest check failed: ${message}`);
  }
}

const chrome = load(".output/chrome-mv3/manifest.json");
assert(chrome["manifest_version"] === 3, "chrome: manifest_version is 3");
assert(chrome["version"] === EXPECTED_VERSION, `chrome: version is ${EXPECTED_VERSION}`);
const chromeBg = chrome["background"] as Record<string, unknown>;
assert(chromeBg["service_worker"] === "background.js", "chrome: service_worker is set");
assert(!("scripts" in chromeBg), "chrome: background.scripts is absent");
assert(
  "commands" in chrome && "_execute_action" in (chrome["commands"] as object),
  "chrome: _execute_action command is set",
);
assert(!("key" in chrome), "chrome: no test key in a normal build");

const firefox = load(".output/firefox-mv2/manifest.json");
assert(firefox["manifest_version"] === 2, "firefox: manifest_version is 2");
assert(firefox["version"] === EXPECTED_VERSION, `firefox: version is ${EXPECTED_VERSION}`);
const ffBg = firefox["background"] as Record<string, unknown>;
assert(
  JSON.stringify(ffBg["scripts"]) === JSON.stringify(["background.js"]),
  "firefox: background.scripts is [background.js]",
);
assert(!("service_worker" in ffBg), "firefox: background.service_worker is absent");
assert(
  "_execute_browser_action" in (firefox["commands"] as object),
  "firefox: _execute_browser_action command is set",
);
const gecko = (firefox["browser_specific_settings"] as Record<string, unknown>)["gecko"] as Record<
  string,
  unknown
>;
assert(gecko["id"] === GECK_ID, `firefox: gecko id is ${GECK_ID}`);
assert(gecko["strict_min_version"] === "142.0", "firefox: strict_min_version is 142.0");

console.log("manifests ok");
