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
    "[theme]",
    'accent = "blue"',
    'selection_background = "magenta"',
    'selection_foreground = "white"',
    "",
    "[trading]",
    'default_profile = "smoke"',
    "",
  ].join("\n"),
);

smokeDumpState();

if (!commandExists("tmux")) {
  console.log("tmux is unavailable; skipping tmux TUI smoke test");
  process.exit(0);
}

const tuiCommand =
  commandWithArgs([
    "--config",
    configPath,
    "--no-persist",
    "--no-account-load",
    "--symbols",
    "AAPL,BTCUSDT",
  ]);
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
    ["Market", "Research", "Watchlist", "Quote / Sessions", "provider: yahoo", "interval=1d"],
    20_000,
  );
  if (!screen) {
    fail("TUI did not render the expected provider-backed cockpit state before timeout");
  }
  if (screen.includes("provider: yahoo-boats")) {
    fail("TUI ignored the configured equity provider and rendered yahoo-boats");
  }

  executePaletteCommand(
    "workspace research",
    ["workspace research", "Workspace research"],
    ["Research", "Polymarket"],
    "research workspace switch",
  );

  executePaletteCommand(
    "toggle live",
    ["toggle live", "Toggle live writes"],
    ["Enable Live Writes", "Enter: enable live writes for this session"],
    "live writes toggle",
  );
  runTmux(["send-keys", "-t", session, "Enter"]);
  if (!waitForScreen(["live:on", "dry-run"], 4_000)) {
    fail("TUI did not enable session live writes while keeping dry-run submit mode");
  }

  executePaletteCommand(
    "focus quote",
    ["focus quote", "Focus quote"],
    ["Quote / Sessions"],
    "quote focus",
  );
  const afterQuoteCommand = waitForScreen(["Quote / Sessions"], 4_000);
  if (!afterQuoteCommand || afterQuoteCommand.includes("Command Palette")) {
    fail("TUI did not execute the filtered command palette action");
  }

  runTmux(["send-keys", "-t", session, "z"]);
  const zoomed = waitForScreen(["Quote / Sessions"], 4_000);
  if (!zoomed || zoomed.includes("Watchlist")) {
    fail("TUI did not zoom the focused pane");
  }

  runTmux(["send-keys", "-t", session, "z"]);
  if (!waitForScreen(["Watchlist", "Polymarket", "News / Research", "Quote / Sessions"], 4_000)) {
    fail("TUI did not restore the workspace layout after zoom");
  }

  editWatchlist();

  stageAndCloseDryRunOrder();

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
    "--no-account-load",
    "--symbols",
    "AAPL,BTCUSDT",
    "--workspace",
    "market",
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

  const requiredKeys = [
    "schema_version",
    "workspace",
    "mode",
    "selected_symbol",
    "config_changes",
    "watchlist_add_query",
    "panes",
    "provider_health",
    "provider_preferences",
    "theme_preferences",
    "tasks",
    "transfer_ticket",
    "futures_state_ticket",
    "staged_changes",
    "pending_staged_confirmation",
  ];
  for (const key of requiredKeys) {
    if (!Object.prototype.hasOwnProperty.call(dump, key)) {
      fail(`dump-state JSON is missing ${key}`);
    }
  }
  if (dump.workspace !== "market") {
    fail(`dump-state workspace mismatch: ${dump.workspace}`);
  }
  if (dump.schema_version !== 14) {
    fail(`dump-state schema_version mismatch: ${dump.schema_version}`);
  }
  if (
    !dump.provider_preferences ||
    dump.provider_preferences.equity !== "yahoo" ||
    dump.provider_preferences.crypto !== "binance"
  ) {
    fail("dump-state provider_preferences missing or unexpected");
  }
  if (
    !dump.theme_preferences ||
    dump.theme_preferences.accent !== "blue" ||
    dump.theme_preferences.selection_background !== "magenta" ||
    dump.theme_preferences.selection_foreground !== "white"
  ) {
    fail("dump-state theme_preferences missing or unexpected");
  }
  if (Object.prototype.hasOwnProperty.call(dump, "write_sessions")) {
    fail("dump-state JSON still exposes legacy write_sessions");
  }
  if (!Array.isArray(dump.staged_changes)) {
    fail("dump-state JSON staged_changes is not an array");
  }
  if (dump.pending_staged_confirmation !== null) {
    fail("dump-state JSON should not have a pending staged confirmation by default");
  }
  if (dump.trading_profile !== "smoke") {
    fail(`dump-state trading_profile mismatch: ${dump.trading_profile}`);
  }
  if (dump.account !== null) {
    fail("dump-state should not load signed account data with --no-account-load");
  }
  if (dump.tasks.some((task) => task.source === "account")) {
    fail("dump-state should not enqueue account load tasks with --no-account-load");
  }
  if (!dump.transfer_ticket || dump.transfer_ticket.asset !== "USDT" || dump.transfer_ticket.direction !== "spot-to-usds-futures") {
    fail("dump-state JSON is missing the default transfer_ticket contract");
  }
  if (!dump.futures_state_ticket || dump.futures_state_ticket.kind !== "leverage" || dump.futures_state_ticket.symbol !== null || dump.futures_state_ticket.ready !== false) {
    fail("dump-state JSON is missing the default futures_state_ticket contract");
  }
  if (!Array.isArray(dump.config_changes) || dump.config_changes.length !== 0 || dump.watchlist_add_query !== "") {
    fail("dump-state JSON is missing the default watchlist edit contract");
  }
  if (!Array.isArray(dump.panes) || !dump.panes.some((pane) => pane.panel === "history" && pane.visible)) {
    fail("dump-state JSON is missing a visible history pane");
  }
}

function executePaletteCommand(query, filterMarkers, resultMarkers, context) {
  runTmux(["send-keys", "-t", session, ":"]);
  if (!waitForScreen(["Command Palette", "Open help"], 4_000)) {
    fail(`TUI did not open the command palette for ${context}`);
  }
  runTmux(["send-keys", "-t", session, query]);
  if (!waitForScreen(filterMarkers, 4_000)) {
    fail(`TUI command palette did not filter ${context}`);
  }
  runTmux(["send-keys", "-t", session, "Enter"]);
  if (!waitForScreen(resultMarkers, 4_000)) {
    fail(`TUI did not complete ${context}`);
  }
}

function editWatchlist() {
  executePaletteCommand(
    "focus watchlist",
    ["focus watchlist", "Focus watchlist"],
    ["Watchlist"],
    "watchlist focus",
  );

  runTmux(["send-keys", "-t", session, "a"]);
  if (!waitForScreen(["Add Symbols"], 4_000)) {
    fail("TUI did not open the watchlist add overlay");
  }
  runTmux(["send-keys", "-t", session, "MSFT", "Enter"]);
  if (!waitForScreen(["MSFT"], 4_000)) {
    fail("TUI did not add MSFT to the watchlist");
  }
  runTmux(["send-keys", "-t", session, "d"]);
  if (!waitForScreen(["removed MSFT"], 4_000)) {
    fail("TUI did not return to the original watchlist after deleting MSFT");
  }
  const watchlist = panelTextByTitle(capturePane(), "Watchlist");
  if (!watchlist.includes("AAPL") || !watchlist.includes("BTCUSDT") || watchlist.includes("MSFT")) {
    fail("TUI watchlist panel did not return to the original symbols after deleting MSFT");
  }
}

function stageAndCloseDryRunOrder() {
  executePaletteCommand(
    "workspace trade",
    ["workspace trade", "Workspace trade"],
    ["Trade", "Order Ticket", "Intent Review"],
    "trade workspace switch",
  );
  executePaletteCommand(
    "focus order",
    ["focus order", "Focus order ticket"],
    ["staged order", "quantity: -", "blocked: quantity is required"],
    "order ticket focus",
  );

  fillMinimalOrderTicket();
  executePaletteCommand(
    "stage order",
    ["stage order", "Stage order ticket"],
    ["staged intents"],
    "order ticket staging",
  );
  if (
    !waitForScreen(
      ["staged intents", "ready", "dry-run", "order", "buy 0.001", "market", "spot", "[smoke]"],
      4_000,
    )
  ) {
    fail("TUI did not stage a dry-run order intent from the order ticket");
  }
  runTmux(["send-keys", "-t", session, "d"]);
  if (!waitForScreen(["No staged changes.", "order ticket candidate ready to stage"], 4_000)) {
    fail("TUI did not close the staged dry-run order intent");
  }
}

function fillMinimalOrderTicket() {
  runTmux(["send-keys", "-t", session, "Down", "Down", "Left", "Left"]);
  if (!waitForScreen(["kind: market"], 4_000)) {
    fail("TUI did not set the order kind to market before staging");
  }
  runTmux(["send-keys", "-t", session, "Down", "Right"]);
  if (!waitForScreen(["quantity: 0.001", "ready for intent review"], 4_000)) {
    fail("TUI did not set a ready market order quantity before staging");
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

function panelTextByTitle(screen, title) {
  const lines = screen.split("\n");
  const titleIndex = lines.findIndex((line) => line.includes(title));
  if (titleIndex === -1) {
    return "";
  }
  const panelLines = [];
  for (let index = titleIndex; index < lines.length; index += 1) {
    const line = lines[index];
    panelLines.push(line);
    if (index > titleIndex && line.includes("┘")) {
      break;
    }
  }
  return panelLines
    .map((line) => {
      const cells = [...line.matchAll(/│([^│]*)/g)].map((match) => match[1]);
      return cells.join("\n");
    })
    .join("\n");
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
