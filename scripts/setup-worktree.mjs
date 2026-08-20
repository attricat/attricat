import { existsSync, readFileSync, writeFileSync } from "node:fs";
import net from "node:net";

const stateFile = new URL("../.catalog-worktree", import.meta.url);
const envFile = new URL("../.env", import.meta.url);
const exampleEnvFile = new URL("../.env.example", import.meta.url);
const portNames = ["POSTGRES_PORT", "API_PORT", "WEB_PORT"];

const parseEnv = (contents) =>
  Object.fromEntries(
    contents
      .split("\n")
      .map((line) => line.match(/^([A-Z_][A-Z0-9_]*)=(.*)$/))
      .filter((match) => match)
      .map((match) => [match[1], match[2]]),
  );

const availablePort = () =>
  new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close((error) => (error ? reject(error) : resolve(port)));
    });
  });

const setEnvValue = (contents, name, value) => {
  const lines = contents.trimEnd().split("\n");
  const index = lines.findIndex((line) => line.startsWith(`${name}=`));
  const entry = `${name}=${value}`;

  if (index === -1) lines.push(entry);
  else lines[index] = entry;

  return `${lines.join("\n")}\n`;
};

let ports = existsSync(stateFile)
  ? parseEnv(readFileSync(stateFile, "utf8"))
  : {};

if (!portNames.every((name) => ports[name])) {
  const values = await Promise.all(portNames.map(() => availablePort()));
  ports = Object.fromEntries(
    portNames.map((name, index) => [name, values[index]]),
  );
  writeFileSync(
    stateFile,
    `${portNames.map((name) => `${name}=${ports[name]}`).join("\n")}\n`,
  );
}

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
env = setEnvValue(env, "POSTGRES_PORT", ports.POSTGRES_PORT);
env = setEnvValue(env, "WEB_PORT", ports.WEB_PORT);
writeFileSync(envFile, env);

console.log(`Development web application: http://127.0.0.1:${ports.WEB_PORT}`);
