import { existsSync, readFileSync, writeFileSync } from "node:fs";
import net from "node:net";

/**
 * Provision this worktree's development environment.
 *
 * Each worktree receives stable, otherwise-unused PostgreSQL, API, web,
 * documentation ports, Mailpit SMTP, Mailpit UI, Jaeger OTLP,
 * Jaeger UI, RustFS S3, RustFS Console, Redis, and file worker operations ports in
 * `.worktree`. The assignments are reused on later runs so
 * `just dev` can be stopped and restarted without changing its URLs. The
 * script creates `.env` from `.env.example` when necessary, while preserving
 * existing non-port configuration in an existing `.env` file.
 */
const stateFile = new URL("../.worktree", import.meta.url);
const envFile = new URL("../.env", import.meta.url);
const exampleEnvFile = new URL("../.env.example", import.meta.url);
const portNames = [
  "POSTGRES_PORT",
  "API_PORT",
  "WEB_PORT",
  "DOCS_PORT",
  "MAILPIT_SMTP_PORT",
  "MAILPIT_UI_PORT",
  "JAEGER_OTLP_GRPC_PORT",
  "JAEGER_UI_PORT",
  "RUSTFS_PORT",
  "RUSTFS_CONSOLE_PORT",
  "FILE_WORKER_OPERATIONS_PORT",
  "REDIS_PORT",
];

/** Parse simple KEY=VALUE entries from .env-style files. */
const parseEnv = (contents) =>
  Object.fromEntries(
    contents
      .split("\n")
      .map((line) => line.match(/^([A-Z_][A-Z0-9_]*)=(.*)$/))
      .filter((match) => match)
      .map((match) => [match[1], match[2]]),
  );

/** Ask the OS for an available loopback TCP port, then release it for use. */
const availablePort = () =>
  new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close((error) => (error ? reject(error) : resolve(port)));
    });
  });

/** Replace or append one unquoted .env entry without changing other lines. */
const setEnvValue = (contents, name, value) => {
  const lines = contents.trimEnd().split("\n");
  const index = lines.findIndex((line) => line.startsWith(`${name}=`));
  const entry = `${name}=${value}`;

  if (index === -1) lines.push(entry);
  else lines[index] = entry;

  return `${lines.join("\n")}\n`;
};

// Keep generated port assignments out of version control but stable per worktree.
let ports = existsSync(stateFile)
  ? parseEnv(readFileSync(stateFile, "utf8"))
  : {};

const missingPortNames = portNames.filter((name) => !ports[name]);
if (missingPortNames.length > 0) {
  const values = await Promise.all(missingPortNames.map(() => availablePort()));
  for (const [index, name] of missingPortNames.entries()) {
    ports[name] = values[index];
  }
}

const webUrl = `http://127.0.0.1:${ports.WEB_PORT}`;
const docsUrl = `http://127.0.0.1:${ports.DOCS_PORT}`;
const mailpitUiUrl = `http://127.0.0.1:${ports.MAILPIT_UI_PORT}`;
const jaegerUiUrl = `http://127.0.0.1:${ports.JAEGER_UI_PORT}`;
const rustfsUiUrl = `http://127.0.0.1:${ports.RUSTFS_CONSOLE_PORT}`;
writeFileSync(
  stateFile,
  `${portNames.map((name) => `${name}=${ports[name]}`).join("\n")}\nWEB_URL=${webUrl}\nDOCS_URL=${docsUrl}\nMAILPIT_UI_URL=${mailpitUiUrl}\nJAEGER_UI_URL=${jaegerUiUrl}\nRUSTFS_UI_URL=${rustfsUiUrl}\n`,
);

// Only connection and listener settings are managed here; retain user overrides.
let env = existsSync(envFile)
  ? readFileSync(envFile, "utf8")
  : readFileSync(exampleEnvFile, "utf8");

if (!/^ATTRICAT_DEVTOOLS=/m.test(env)) {
  env = setEnvValue(env, "ATTRICAT_DEVTOOLS", "true");
}
if (!/^ATTRICAT_SAMPLE_ACCOUNTS=/m.test(env)) {
  env = setEnvValue(env, "ATTRICAT_SAMPLE_ACCOUNTS", "true");
}

env = setEnvValue(
  env,
  "DATABASE_URL",
  `postgres://postgres:postgres@localhost:${ports.POSTGRES_PORT}/attricat`,
);
env = setEnvValue(env, "BIND_ADDR", `127.0.0.1:${ports.API_PORT}`);
env = setEnvValue(env, "ATTRICAT_API_URL", `http://127.0.0.1:${ports.API_PORT}/api`);
env = setEnvValue(env, "WEB_PORT", ports.WEB_PORT);
env = setEnvValue(env, "DOCS_PORT", ports.DOCS_PORT);
env = setEnvValue(env, "POSTGRES_PORT", ports.POSTGRES_PORT);
env = setEnvValue(env, "MAILPIT_SMTP_PORT", ports.MAILPIT_SMTP_PORT);
env = setEnvValue(env, "MAILPIT_UI_PORT", ports.MAILPIT_UI_PORT);
env = setEnvValue(env, "JAEGER_OTLP_GRPC_PORT", ports.JAEGER_OTLP_GRPC_PORT);
env = setEnvValue(env, "JAEGER_UI_PORT", ports.JAEGER_UI_PORT);
env = setEnvValue(
  env,
  "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
  `http://127.0.0.1:${ports.JAEGER_OTLP_GRPC_PORT}`,
);
env = setEnvValue(env, "RUSTFS_PORT", ports.RUSTFS_PORT);
// The query cache stays in memory unless CACHE_BACKEND=redis is set.
env = setEnvValue(env, "REDIS_PORT", ports.REDIS_PORT);
env = setEnvValue(env, "REDIS_URL", `redis://127.0.0.1:${ports.REDIS_PORT}`);
env = setEnvValue(env, "RUSTFS_CONSOLE_PORT", ports.RUSTFS_CONSOLE_PORT);
env = setEnvValue(
  env,
  "FILE_WORKER_OPERATIONS_BIND_ADDR",
  `127.0.0.1:${ports.FILE_WORKER_OPERATIONS_PORT}`,
);
env = setEnvValue(env, "S3_ENDPOINT", `http://127.0.0.1:${ports.RUSTFS_PORT}`);
env = setEnvValue(env, "S3_REGION", "us-east-1");
env = setEnvValue(env, "S3_BUCKET", "attricat-files");
env = setEnvValue(env, "S3_ACCESS_KEY_ID", "attricat-dev");
env = setEnvValue(env, "S3_SECRET_ACCESS_KEY", "attricat-dev-secret");
env = setEnvValue(env, "S3_FORCE_PATH_STYLE", "true");
env = setEnvValue(env, "S3_UPLOAD_TIMEOUT_SECONDS", "30");
env = setEnvValue(env, "S3_DOWNLOAD_TIMEOUT_SECONDS", "30");
env = setEnvValue(env, "SMTP_PORT", ports.MAILPIT_SMTP_PORT);
env = setEnvValue(
  env,
  "PASSWORD_RESET_URL",
  `http://127.0.0.1:${ports.WEB_PORT}/password-reset/confirm`,
);
env = setEnvValue(
  env,
  "WORKSPACE_INVITATION_URL",
  `http://127.0.0.1:${ports.WEB_PORT}/invitations/accept`,
);
env = setEnvValue(
  env,
  "WORKSPACE_ONBOARDING_URL",
  `http://127.0.0.1:${ports.WEB_PORT}/onboarding`,
);
writeFileSync(envFile, env);

console.log(`Development web application: ${webUrl}`);
console.log(`Documentation site: ${docsUrl}`);
console.log(`Mailpit: ${mailpitUiUrl}`);
console.log(`Jaeger: ${jaegerUiUrl}`);
console.log(`RustFS: ${rustfsUiUrl}`);
