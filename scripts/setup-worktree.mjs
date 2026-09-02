import { existsSync, readFileSync, writeFileSync } from "node:fs";
import net from "node:net";

/**
 * Provision this worktree's development environment.
 *
 * Each worktree receives stable, otherwise-unused PostgreSQL, API, and web
 * ports, Mailpit SMTP, Mailpit UI, RustFS S3, and RustFS Console ports in `.catalog-worktree`. The assignments are reused on later runs so
 * `just dev` can be stopped and restarted without changing its URLs. The
 * script creates `.env` from `.env.example` when necessary, while preserving
 * existing non-port configuration in an existing `.env` file.
 */
const stateFile = new URL("../.catalog-worktree", import.meta.url);
const envFile = new URL("../.env", import.meta.url);
const exampleEnvFile = new URL("../.env.example", import.meta.url);
const portNames = [
  "POSTGRES_PORT",
  "API_PORT",
  "WEB_PORT",
  "MAILPIT_SMTP_PORT",
  "MAILPIT_UI_PORT",
  "RUSTFS_PORT",
  "RUSTFS_CONSOLE_PORT",
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

if (!portNames.every((name) => ports[name])) {
  const values = await Promise.all(portNames.map(() => availablePort()));
  ports = Object.fromEntries(
    portNames.map((name, index) => [name, values[index]]),
  );
}

const webUrl = `http://127.0.0.1:${ports.WEB_PORT}`;
const mailpitUiUrl = `http://127.0.0.1:${ports.MAILPIT_UI_PORT}`;
const rustfsUiUrl = `http://127.0.0.1:${ports.RUSTFS_CONSOLE_PORT}`;
writeFileSync(
  stateFile,
  `${portNames.map((name) => `${name}=${ports[name]}`).join("\n")}\nWEB_URL=${webUrl}\nMAILPIT_UI_URL=${mailpitUiUrl}\nRUSTFS_UI_URL=${rustfsUiUrl}\n`,
);

// Only connection and listener settings are managed here; retain user overrides.
let env = existsSync(envFile)
  ? readFileSync(envFile, "utf8")
  : readFileSync(exampleEnvFile, "utf8");

env = setEnvValue(
  env,
  "DATABASE_URL",
  `postgres://postgres:postgres@localhost:${ports.POSTGRES_PORT}/catalog`,
);
env = setEnvValue(env, "BIND_ADDR", `127.0.0.1:${ports.API_PORT}`);
env = setEnvValue(env, "CATALOG_API_URL", `http://127.0.0.1:${ports.API_PORT}`);
env = setEnvValue(env, "WEB_PORT", ports.WEB_PORT);
env = setEnvValue(env, "POSTGRES_PORT", ports.POSTGRES_PORT);
env = setEnvValue(env, "MAILPIT_SMTP_PORT", ports.MAILPIT_SMTP_PORT);
env = setEnvValue(env, "MAILPIT_UI_PORT", ports.MAILPIT_UI_PORT);
env = setEnvValue(env, "RUSTFS_PORT", ports.RUSTFS_PORT);
env = setEnvValue(env, "RUSTFS_CONSOLE_PORT", ports.RUSTFS_CONSOLE_PORT);
env = setEnvValue(env, "S3_ENDPOINT", `http://127.0.0.1:${ports.RUSTFS_PORT}`);
env = setEnvValue(env, "S3_REGION", "us-east-1");
env = setEnvValue(env, "S3_BUCKET", "catalog-files");
env = setEnvValue(env, "S3_ACCESS_KEY_ID", "catalog-dev");
env = setEnvValue(env, "S3_SECRET_ACCESS_KEY", "catalog-dev-secret");
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
console.log(`Mailpit: ${mailpitUiUrl}`);
console.log(`RustFS: ${rustfsUiUrl}`);
