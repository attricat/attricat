import http from "node:http";
import https from "node:https";
import { setTimeout as delay } from "node:timers/promises";

if (process.argv[2] === "serve") {
  http
    .createServer((_request, response) => {
      response.writeHead(503, {
        "content-type": "text/plain; charset=utf-8",
        "cache-control": "no-store",
        "retry-after": "60",
        "x-demo-maintenance": "true",
      });
      response.end(
        "The shared Attricat demo is being restored. Please try again shortly.\nAll demo changes are discarded every hour, at HH:00 UTC.\n",
      );
    })
    .listen(8080, "0.0.0.0");
} else if (["closed", "open"].includes(process.argv[2])) {
  const closed = process.argv[2] === "closed";
  // Probe the origin from inside Docker, not Cloudflare. The origin certificate
  // is not publicly trusted; this is only a readiness probe, with no credentials.
  const probe = () =>
    new Promise((resolve, reject) => {
      const request = https.get(
        {
          hostname: "traefik",
          port: 443,
          path: "/health/ready",
          servername: process.env.DEMO_HOST ?? "demo.attricat.com",
          headers: { host: process.env.DEMO_HOST ?? "demo.attricat.com" },
          rejectUnauthorized: false,
          timeout: 5000,
        },
        (response) => {
          response.resume();
          resolve(
            closed
              ? response.statusCode === 503 &&
                  response.headers["x-demo-maintenance"] === "true"
              : response.statusCode === 200 &&
                  !response.headers["x-demo-maintenance"],
          );
        },
      );
      request.on("timeout", () => request.destroy(new Error("probe timeout")));
      request.on("error", reject);
    });
  let ready = false;
  for (let attempt = 0; attempt < 60; attempt++) {
    if (await probe().catch(() => false)) {
      ready = true;
      break;
    }
    await delay(1000);
  }
  if (!ready)
    throw new Error(`Proxy did not become ${closed ? "closed" : "open"}`);
  console.log(
    `Proxy verified ${closed ? "closed (maintenance)" : "open (ready)"}`,
  );
} else {
  throw new Error("usage: maintenance.mjs serve|closed|open");
}
