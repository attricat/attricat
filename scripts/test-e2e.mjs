import { spawn, spawnSync } from "node:child_process";
import net from "node:net";

const workspaceRoot = new URL("../", import.meta.url).pathname;
const webRoot = new URL("../apps/catalog-web/", import.meta.url).pathname;

const availablePort = () =>
  new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close((error) => (error ? reject(error) : resolve(port)));
    });
  });

const setup = spawnSync(process.execPath, ["scripts/setup-worktree.mjs"], {
  cwd: workspaceRoot,
  stdio: "inherit",
});
if (setup.status !== 0) process.exit(setup.status ?? 1);

const [apiPort, webPort, mailpitSmtpPort, mailpitUiPort] = await Promise.all([
  availablePort(),
  availablePort(),
  availablePort(),
  availablePort(),
]);
const playwright = spawn(
  "node_modules/.bin/playwright",
  ["test", ...process.argv.slice(2)],
  {
    cwd: webRoot,
    env: {
      ...process.env,
      CATALOG_E2E_API_PORT: String(apiPort),
      CATALOG_E2E_WEB_PORT: String(webPort),
      CATALOG_E2E_MAILPIT_SMTP_PORT: String(mailpitSmtpPort),
      CATALOG_E2E_MAILPIT_UI_PORT: String(mailpitUiPort),
    },
    stdio: "inherit",
  },
);

playwright.on("exit", (code) => process.exit(code ?? 1));
