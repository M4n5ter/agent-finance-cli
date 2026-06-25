#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const { PACKAGES } = require("./platform");

const root = path.resolve(__dirname, "..");
const packageJson = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const optional = packageJson.optionalDependencies || {};
const expectedPackages = Object.values(PACKAGES).sort();
const optionalPackages = Object.keys(optional).sort();

assertArrayEquals(optionalPackages, expectedPackages, "optionalDependencies must match platform map");

for (const packageName of expectedPackages) {
  if (optional[packageName] !== packageJson.version) {
    fail(`${packageName} version must match root package version ${packageJson.version}`);
  }
}

for (const file of [
  "README.md",
  "README_ZH.md",
  "README_JA.md",
  "README_KO.md",
  "bin/",
  "npm/",
  "skills/",
  "skill-data/",
  "crates/cli/Cargo.toml",
  "crates/cli/src/",
  "crates/binance/Cargo.toml",
  "crates/binance/src/",
  "crates/core/Cargo.toml",
  "crates/core/src/",
  "crates/market/Cargo.toml",
  "crates/market/src/",
  "Cargo.toml",
  "Cargo.lock",
]) {
  if (!packageJson.files.includes(file)) {
    fail(`package.json files must include ${file}`);
  }
}

console.log("npm package metadata is consistent");

function assertArrayEquals(actual, expected, message) {
  if (actual.length !== expected.length || actual.some((value, index) => value !== expected[index])) {
    fail(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
