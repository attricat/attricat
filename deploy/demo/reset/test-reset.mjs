#!/usr/bin/env node
// End-to-end test against the published application image. Never uses the VPS.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import net from "node:net";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";

const source = fileURLToPath(new URL("../", import.meta.url));
// Under HOME rather than /tmp so Colima/Docker Desktop can see the bind mounts.
mkdirSync(join(homedir(), ".cache"), { recursive: true });
const work = mkdtempSync(join(homedir(), ".cache/attricat-reset-e2e-"));
const project = `attricat-reset-e2e-${process.pid}`;
const env = { ...process.env };
// Never inherit another deployment's Compose selection or infrastructure secrets.
for (const key of Object.keys(env)) {
  if (/^(COMPOSE_|ATTRICAT_|CATALOG_|POSTGRES_|S3_|DEMO_)/.test(key))
    delete env[key];
}
function run(command, args, { expected = 0, ...options } = {}) {
  const result = spawnSync(command, args, {
    cwd: work,
    env,
    encoding: "utf8",
    timeout: 600_000,
    ...options,
  });
  if (result.status !== expected) {
    console.error(result.stdout, result.stderr);
    throw new Error(
      `${command} ${args.join(" ")} exited ${result.status}, expected ${expected}`,
    );
  }
  return result.stdout.trim();
}
const dc = (...args) => run("docker", ["compose", ...args]);
const loader = (...args) => dc("run", "--rm", "--no-deps", "loader", ...args);
const helper = (...args) =>
  dc(
    "run",
    "--rm",
    "--no-deps",
    "--entrypoint",
    "node",
    "loader",
    "/fixture/test-client.mjs",
    ...args,
  );
const reset = (options) => run("bash", ["./reset.sh"], options);
let completed = false;
let resetChild;
console.log(`Disposable test deployment: ${work}`);
try {
  for (const path of [
    "compose.yml",
    "reset.sh",
    "reset",
    "traefik/traefik.yml",
    "traefik/dynamic/tls.yml",
    "traefik/dynamic/application.yml",
  ]) {
    cpSync(join(source, path), join(work, path), { recursive: true });
  }
  mkdirSync(join(work, "traefik/certs"), { recursive: true });
  run("openssl", [
    "req",
    "-x509",
    "-newkey",
    "rsa:2048",
    "-nodes",
    "-keyout",
    "traefik/certs/origin.key",
    "-out",
    "traefik/certs/origin.pem",
    "-days",
    "1",
    "-subj",
    "/CN=demo.attricat.com",
  ]);
  writeFileSync(
    join(work, ".env"),
    `COMPOSE_PROJECT_NAME=${project}\nATTRICAT_IMAGE=${process.env.TEST_ATTRICAT_IMAGE ?? "ghcr.io/attricat/attricat:latest"}\nATTRICAT_OWNER_EMAIL=owner@example.com\nATTRICAT_OWNER_PASSWORD=test\nPOSTGRES_PASSWORD=reset-test-only\nS3_SECRET_ACCESS_KEY=reset-test-only-secret\n`,
  );
  const portReservation = net.createServer().listen(0, "127.0.0.1");
  await once(portReservation, "listening");
  const port = portReservation.address().port;
  await new Promise((resolve) => portReservation.close(resolve));
  writeFileSync(
    join(work, "compose.override.yml"),
    `services:
  traefik:
    ports: !override ["127.0.0.1:${port}:443"]
  mailpit:
    ports: !reset []
  api:
    platform: linux/amd64
  file-worker:
    platform: linux/amd64
  migrate:
    platform: linux/amd64
`,
  );
  // Pull missing prerequisites only. Do not silently move a cached :latest.
  for (const image of dc("config", "--images").split("\n")) {
    if (
      spawnSync("docker", ["image", "inspect", image], { stdio: "ignore" })
        .status !== 0
    )
      run("docker", ["pull", image]);
  }
  reset();
  loader("verify");
  run("docker", ["compose", "run", "--rm", "--no-deps", "loader", "load"], {
    expected: 1,
  });
  console.log("PASS: clean reset, exact verification, nonempty-load refusal");

  const oldState = helper("mutate");
  // Store an arbitrary visitor object, and captured email, in their real services.
  dc(
    "run",
    "--rm",
    "--no-deps",
    "--entrypoint",
    "aws",
    "rustfs-init",
    "--endpoint-url",
    "http://rustfs:9000",
    "s3api",
    "put-object",
    "--bucket",
    "catalog-files",
    "--key",
    "visitor-marker",
    "--body",
    "/etc/hostname",
  );
  dc(
    "run",
    "--rm",
    "--no-deps",
    "--entrypoint",
    "node",
    "loader",
    "--input-type=module",
    "-e",
    `const r = await fetch('http://mailpit:8025/api/v1/send', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ From: { Email: 'visitor@example.test' }, To: [{ Email: 'owner@example.com' }], Subject: 'Reset test', Text: 'Visitor mail' }) }); if (!r.ok) throw new Error(await r.text());`,
  );
  const mailboxCount = () =>
    dc(
      "run",
      "--rm",
      "--no-deps",
      "--entrypoint",
      "node",
      "loader",
      "--input-type=module",
      "-e",
      "console.log((await (await fetch('http://mailpit:8025/api/v1/messages')).json()).total)",
    );
  assert.equal(mailboxCount(), "1");

  const address = dc("port", "traefik", "443");
  resetChild = spawn("bash", ["./reset.sh"], {
    cwd: work,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  resetChild.stdout.on("data", (data) => {
    output += data;
  });
  resetChild.stderr.on("data", (data) => {
    output += data;
  });
  const done = once(resetChild, "close");
  let closed = false;
  for (let i = 0; i < 100; i++) {
    const probe = spawnSync(
      "curl",
      [
        "-sk",
        "--max-time",
        "2",
        "-o",
        "/dev/null",
        "-w",
        "%{http_code}",
        "-H",
        "Host: demo.attricat.com",
        `https://${address}/health/ready`,
      ],
      { encoding: "utf8" },
    );
    if (probe.stdout === "503") {
      closed = true;
      break;
    }
    await delay(100);
  }
  assert.ok(closed, `reset must close public access\n${output}`);
  assert.match(reset({ expected: 1 }), /^$/); // refusal is on stderr
  const status = run("curl", [
    "-sk",
    "-o",
    "/dev/null",
    "-w",
    "%{http_code}",
    "-X",
    "POST",
    "-H",
    "Host: demo.attricat.com",
    "-H",
    `Cookie: ${JSON.parse(oldState).cookie}`,
    "-H",
    "Content-Type: application/json",
    "-d",
    "{}",
    `https://${address}/v1/entities`,
  ]);
  assert.equal(status, "503", "public writes must be blocked during reset");
  const [code] = await done;
  resetChild = undefined;
  assert.equal(code, 0, output);
  dc(
    "run",
    "--rm",
    "--no-deps",
    "-e",
    `OLD_STATE=${oldState}`,
    "--entrypoint",
    "node",
    "loader",
    "/fixture/test-client.mjs",
    "check-old-credentials",
  );
  const objects = JSON.parse(
    dc(
      "run",
      "--rm",
      "--no-deps",
      "--entrypoint",
      "aws",
      "rustfs-init",
      "--endpoint-url",
      "http://rustfs:9000",
      "s3api",
      "list-objects-v2",
      "--bucket",
      "catalog-files",
      "--output",
      "json",
    ),
  );
  assert.deepEqual(objects.Contents ?? [], []);
  assert.equal(mailboxCount(), "0");
  console.log(
    "PASS: concurrent reset blocked, public writes blocked, edits/deletions/extras restored, old sessions/PATs/objects/mail removed",
  );

  // Test failure and notification without adding a production failure switch.
  const loadPath = join(work, "reset/load.mjs");
  const original = readFileSync(loadPath, "utf8");
  writeFileSync(
    loadPath,
    original.replace(
      'if (mode === "load") await load(client.request, fixture);',
      'if (mode === "load") throw new Error("injected test failure");',
    ),
  );
  writeFileSync(
    join(work, "failure-hook.sh"),
    '#!/bin/sh\nprintf "%s\\n" "$2" > "$1/notification.txt"\n',
    { mode: 0o755 },
  );
  env.DEMO_RESET_FAILURE_HOOK = join(work, "failure-hook.sh");
  reset({ expected: 1 });
  assert.equal(
    readFileSync(join(work, "notification.txt"), "utf8").trim(),
    "load",
  );
  dc(
    "run",
    "--rm",
    "--no-deps",
    "--entrypoint",
    "node",
    "loader",
    "/fixture/maintenance.mjs",
    "closed",
  );
  assert.equal(dc("ps", "--status", "running", "-q", "api"), "");
  assert.equal(
    JSON.parse(readFileSync(join(work, ".reset-status.json"))).success,
    false,
  );
  writeFileSync(loadPath, original);
  reset();
  loader("verify");
  console.log(
    "PASS: load failure stays closed, stops writers, invokes notification; next reset recovers",
  );
  completed = true;
} finally {
  if (resetChild) {
    resetChild.kill("SIGTERM");
    await once(resetChild, "close");
  }
  // Only this uniquely named, disposable Compose project is removed.
  try {
    dc("down", "--volumes", "--remove-orphans");
  } catch (error) {
    console.error(error);
  }
  if (completed) rmSync(work, { recursive: true, force: true });
  else console.error(`Failure artifacts retained at ${work}`);
}
