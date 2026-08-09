import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import url from "node:url";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const root = path.resolve(repoRoot, "dist");
const port = 4173;
const host = "127.0.0.1";
const runtimeTarget = process.env.VAULT_RUNTIME_TARGET || "http://127.0.0.1:8787";

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".ico": "image/x-icon",
  ".map": "application/json; charset=utf-8",
  ".woff2": "font/woff2",
  ".woff": "font/woff",
  ".txt": "text/plain; charset=utf-8",
};

function sendFile(res, filePath) {
  const ext = path.extname(filePath).toLowerCase();
  fs.readFile(filePath, (err, data) => {
    if (err) {
      res.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
      res.end("Read Error");
      return;
    }
    res.writeHead(200, {
      "Content-Type": MIME[ext] || "application/octet-stream",
      "Cache-Control": "no-store",
      "Content-Length": Buffer.byteLength(data),
    });
    res.end(data);
  });
}

function sendIndex(res) {
  const fp = path.join(root, "index.html");
  fs.readFile(fp, (err, data) => {
    if (err) {
      res.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
      res.end("Index Missing");
      return;
    }
    res.writeHead(200, {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store",
      "Content-Length": Buffer.byteLength(data),
    });
    res.end(data);
  });
}

function sendJson(res, status, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Cache-Control": "no-store",
    "Content-Length": Buffer.byteLength(body),
  });
  res.end(body);
}

function proxyRuntime(req, res) {
  const targetUrl = new URL(req.url || "/", runtimeTarget);
  const downstreamHeaders = { ...req.headers };
  downstreamHeaders.host = targetUrl.host;
  delete downstreamHeaders["accept-encoding"];
  delete downstreamHeaders["origin"];
  delete downstreamHeaders["referer"];
  delete downstreamHeaders["sec-fetch-site"];
  delete downstreamHeaders["sec-fetch-mode"];
  delete downstreamHeaders["sec-fetch-dest"];
  delete downstreamHeaders["sec-fetch-user"];
  if (req.httpVersionMajor < 2) {
    delete downstreamHeaders.connection;
    delete downstreamHeaders["proxy-connection"];
  }
  const hasBodyLength =
    typeof downstreamHeaders["content-length"] === "string" &&
    /^\d+$/.test(downstreamHeaders["content-length"]);
  const isChunked =
    String(downstreamHeaders["transfer-encoding"] || "")
      .toLowerCase()
      .includes("chunked");
  if (!hasBodyLength && !isChunked && req.method && ["POST", "PUT", "PATCH", "DELETE"].includes(req.method.toUpperCase())) {
    downstreamHeaders["transfer-encoding"] = "chunked";
  }

  const options = {
    method: req.method,
    hostname: targetUrl.hostname,
    port: targetUrl.port,
    path: targetUrl.pathname + targetUrl.search,
    headers: downstreamHeaders,
  };

  const upstream = http.request(options, (upRes) => {
    const outHeaders = { ...upRes.headers };
    outHeaders["cache-control"] = "no-store";
    delete outHeaders["content-length"];
    delete outHeaders["transfer-encoding"];
    res.writeHead(upRes.statusCode || 502, outHeaders);
    upRes.pipe(res);
  });

  upstream.on("error", () => {
    sendJson(res, 502, {
      code: "RUNTIME_UNAVAILABLE",
      message: "本地 Runtime 暂不可用，请确认 8787 端口服务已启动。",
      retryable: true,
    });
  });

  req.on("aborted", () => {
    try {
      upstream.destroy(new Error("client aborted"));
    } catch (_e) {
      /* ignore */
    }
  });

  req.pipe(upstream);
}

const server = http.createServer((req, res) => {
  const raw = decodeURIComponent((req.url || "/").split("?")[0]);
  if (raw.startsWith("/api/")) {
    return proxyRuntime(req, res);
  }
  const requested = path.normalize(path.join(root, raw));
  if (!requested.startsWith(root)) {
    res.writeHead(403, { "Content-Type": "text/plain; charset=utf-8" });
    res.end("Forbidden");
    return;
  }

  fs.stat(requested, (err, stat) => {
    if (err) return sendIndex(res);
    if (stat.isDirectory()) {
      const idx = path.join(requested, "index.html");
      return fs.stat(idx, (e) => (e ? sendIndex(res) : sendFile(res, idx)));
    }
    sendFile(res, requested);
  });
});

server.listen(port, host, () => {
  console.log(`[docs-test] static server: http://${host}:${port}`);
  console.log(`[docs-test] /api proxy -> ${runtimeTarget}`);
});
