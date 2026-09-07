import http from 'node:http';
import { Readable } from 'node:stream';

const upstreamValue = process.env.LLM_UPSTREAM_BASE_URL;
const port = Number(process.env.LLM_LOOPBACK_PROXY_PORT ?? 4010);

if (!upstreamValue) {
  throw new Error('LLM_UPSTREAM_BASE_URL must be set.');
}
if (!Number.isInteger(port) || port < 1 || port > 65535) {
  throw new Error('LLM_LOOPBACK_PROXY_PORT must be an integer between 1 and 65535.');
}

const upstream = new URL(upstreamValue);
if (!['http:', 'https:'].includes(upstream.protocol)) {
  throw new Error('LLM_UPSTREAM_BASE_URL must be an HTTP(S) URL.');
}

http
  .createServer(async (request, response) => {
    try {
      const url = new URL(request.url, upstream);
      const headers = { ...request.headers, host: upstream.host };
      const upstreamResponse = await fetch(url, {
        method: request.method,
        headers,
        body: ['GET', 'HEAD'].includes(request.method) ? undefined : Readable.toWeb(request),
        duplex: 'half',
      });
      response.writeHead(upstreamResponse.status, Object.fromEntries(upstreamResponse.headers));
      if (upstreamResponse.body) {
        Readable.fromWeb(upstreamResponse.body).pipe(response);
      } else {
        response.end();
      }
    } catch {
      response.writeHead(502, { 'content-type': 'text/plain' });
      response.end('LiteLLM loopback proxy could not reach its upstream.');
    }
  })
  .listen(port, '127.0.0.1', () => {
    console.info(`LiteLLM loopback proxy listening at http://127.0.0.1:${port}`);
  });
