#!/usr/bin/env node

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const root = path.resolve(__dirname, "..");
const session = `agent-finance-tui-smoke-${process.pid}`;
const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "agent-finance-tui-"));
const configPath = path.join(tempDir, "tui.toml");
const statusPath = path.join(tempDir, "status");
const baseTuiCommand = process.env.AGENT_FINANCE_TUI_CMD || "cargo run --quiet -- tui";

fs.writeFileSync(
  configPath,
  [
    'watchlist = ["AAPL", "BTCUSDT"]',
    "",
    "[providers]",
    'equity = "yahoo"',
    'crypto = "binance"',
    "",
  ].join("\n"),
);

smokeDumpState();

if (!commandExists("tmux")) {
  console.log("tmux is unavailable; skipping tmux TUI smoke test");
  process.exit(0);
}

const tuiCommand =
  commandWithArgs(["--config", configPath, "--no-persist", "--symbols", "AAPL,BTCUSDT"]);
const wrappedCommand = `cd ${shellQuote(root)} && ${tuiCommand}; printf '%s' "$?" > ${shellQuote(statusPath)}`;

try {
  runTmux([
    "new-session",
    "-d",
    "-s",
    session,
    "-x",
    "140",
    "-y",
    "30",
    "sh",
    "-lc",
    wrappedCommand,
  ]);

  const screen = waitForScreen(
    ["Overview", "Research", "Watchlist", "Quote / Sessions", "provider: yahoo", "interval=1d"],
    20_000,
  );
  if (!screen) {
    fail("TUI did not render the expected provider-backed cockpit state before timeout");
  }
  if (screen.includes("provider: yahoo-boats")) {
    fail("TUI ignored the configured equity provider and rendered yahoo-boats");
  }

  runTmux(["send-keys", "-t", session, "]"]);
  if (!waitForScreen(["Research", "Polymarket"], 4_000)) {
    fail("TUI did not switch to the research workspace");
  }

  runTmux(["send-keys", "-t", session, ":"]);
  if (!waitForScreen(["Command Palette", "Open help"], 4_000)) {
    fail("TUI did not open the command palette");
  }

  runTmux(["send-keys", "-t", session, "quote"]);
  if (!waitForScreen(["quote", "Focus quote", "Toggle quote"], 4_000)) {
    fail("TUI command palette did not filter commands from typed input");
  }

  runTmux(["send-keys", "-t", session, "Enter"]);
  if (!waitForScreen(["mode: normal", "focus: Quote / Sessions"], 4_000)) {
    fail("TUI did not execute the filtered command palette action");
  }

  runTmux(["send-keys", "-t", session, "z"]);
  if (!waitForScreen(["visible: 1/5", "focus: Quote / Sessions"], 4_000)) {
    fail("TUI did not zoom the focused pane");
  }

  runTmux(["send-keys", "-t", session, "z"]);
  if (!waitForScreen(["visible: 5/5", "focus: Quote / Sessions"], 4_000)) {
    fail("TUI did not restore the workspace layout after zoom");
  }

  runTmux(["send-keys", "-t", session, "q"]);
  waitForSessionExit(8_000);

  const status = fs.existsSync(statusPath) ? fs.readFileSync(statusPath, "utf8") : "<missing>";
  if (status !== "0") {
    fail(`TUI exited with status ${status}`);
  }

  console.log("TUI smoke tests passed");
} finally {
  spawnSync("tmux", ["kill-session", "-t", session], { stdio: "ignore" });
  fs.rmSync(tempDir, { recursive: true, force: true });
}

function smokeDumpState() {
  const command = commandWithArgs([
    "--config",
    configPath,
    "--no-persist",
    "--symbols",
    "AAPL,BTCUSDT",
    "--workspace",
    "crypto",
    "--dump-state",
    "--wait-seconds",
    "0",
    "--json",
  ]);
  const result = spawnSync("sh", ["-lc", command], {
    cwd: root,
    encoding: "utf8",
  });
  if (result.error) {
    fail(`dump-state smoke could not start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`dump-state smoke failed: ${result.stderr || result.stdout}`);
  }

  let dump;
  try {
    dump = JSON.parse(result.stdout);
  } catch (error) {
    fail(`dump-state did not emit valid JSON: ${error.message}\n${result.stdout}`);
  }

  const requiredKeys = ["workspace", "mode", "selected_symbol", "panes", "provider_health", "tasks"];
  for (const key of requiredKeys) {
    if (!Object.prototype.hasOwnProperty.call(dump, key)) {
      fail(`dump-state JSON is missing ${key}`);
    }
  }
  if (dump.workspace !== "crypto") {
    fail(`dump-state workspace mismatch: ${dump.workspace}`);
  }
  if (!Array.isArray(dump.panes) || !dump.panes.some((pane) => pane.panel === "history" && pane.visible)) {
    fail("dump-state JSON is missing a visible history pane");
  }
}

function commandWithArgs(args) {
  return `${baseTuiCommand} ${args.map(shellQuote).join(" ")}`;
}

function waitForScreen(markers, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastScreen = "";
  while (Date.now() < deadline) {
    lastScreen = capturePane();
    if (markers.every((marker) => lastScreen.includes(marker))) {
      return lastScreen;
    }
    sleep(250);
  }
  if (lastScreen) {
    process.stderr.write(lastScreen);
  }
  return "";
}

function waitForSessionExit(timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = spawnSync("tmux", ["has-session", "-t", session], {
      encoding: "utf8",
      env: tmuxEnv(),
    });
    if (result.status !== 0) {
      return;
    }
    sleep(250);
  }
  fail("TUI tmux session did not exit after q");
}

function capturePane() {
  const result = spawnSync("tmux", ["capture-pane", "-p", "-t", session], {
    encoding: "utf8",
    env: tmuxEnv(),
  });
  return result.status === 0 ? result.stdout : "";
}

function runTmux(args) {
  const result = spawnSync("tmux", args, {
    encoding: "utf8",
    env: tmuxEnv(),
  });
  if (result.status !== 0) {
    fail(`tmux ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
}

function commandExists(command) {
  return spawnSync("sh", ["-lc", `command -v ${shellQuote(command)}`], {
    stdio: "ignore",
  }).status === 0;
}

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

function shellQuote(value) {
  return `'${String(value).replaceAll("'", "'\\''")}'`;
}

function tmuxEnv() {
  return {
    ...process.env,
    TERM: process.env.TERM || "xterm-256color",
  };
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
