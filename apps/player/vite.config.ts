import fs from 'node:fs';
import type { IncomingMessage, ServerResponse } from 'node:http';
import path from 'node:path';
import react from '@vitejs/plugin-react';
import { defineConfig, type Connect, type Plugin } from 'vite';

const MIME_BY_EXT: Record<string, string> = {
  '.webp': 'image/webp',
  '.png': 'image/png',
  '.svg': 'image/svg+xml; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
};

function sendJson(res: ServerResponse, statusCode: number, payload: unknown): void {
  const body = JSON.stringify(payload);
  res.statusCode = statusCode;
  res.setHeader('Content-Type', 'application/json; charset=utf-8');
  res.setHeader('Cache-Control', 'no-store');
  res.end(body);
}

function serveSafeFile(
  res: ServerResponse,
  baseDir: string,
  relUrlPath: string,
  allowedExts: ReadonlySet<string>,
): void {
  let decodedPath: string;
  try {
    decodedPath = decodeURIComponent(relUrlPath);
  } catch {
    res.statusCode = 400;
    res.end('Bad Request');
    return;
  }

  const normalizedRel = decodedPath.replace(/^\/+/, '');
  const resolvedBase = path.resolve(baseDir);
  const resolvedFile = path.resolve(resolvedBase, normalizedRel);

  const isInsideBase =
    resolvedFile === resolvedBase ||
    resolvedFile.startsWith(resolvedBase + path.sep);

  if (!isInsideBase) {
    res.statusCode = 403;
    res.end('Forbidden');
    return;
  }

  const ext = path.extname(resolvedFile).toLowerCase();
  if (!allowedExts.has(ext)) {
    res.statusCode = 403;
    res.end('Unsupported extension');
    return;
  }

  if (!fs.existsSync(resolvedFile) || !fs.statSync(resolvedFile).isFile()) {
    res.statusCode = 404;
    res.end('Not Found');
    return;
  }

  const contentType = MIME_BY_EXT[ext] ?? 'application/octet-stream';
  res.statusCode = 200;
  res.setHeader('Content-Type', contentType);
  res.setHeader('Cache-Control', ext === '.json' ? 'no-store' : 'public, max-age=3600');
  fs.createReadStream(resolvedFile).pipe(res);
}

function createBridgeMiddleware(dataDir: string): Connect.NextHandleFunction {
  const assetsDir = path.join(dataDir, 'assets', 'encyclopedia');
  const svgDir = path.join(dataDir, 'apps', 'encyclopedia');
  const jsonDir = path.join(dataDir, 'data', 'encyclopedia');

  const assetExts = new Set(['.webp', '.png', '.svg', '.json']);
  const svgExts = new Set(['.svg']);
  const jsonExts = new Set(['.json']);

  return (req: IncomingMessage, res: ServerResponse, next: Connect.NextFunction): void => {
    const rawUrl = req.url;
    if (!rawUrl || (req.method !== 'GET' && req.method !== 'HEAD')) {
      next();
      return;
    }

    const pathname = rawUrl.split('?')[0] ?? '';

    if (pathname === '/hsr-meta/status') {
      sendJson(res, 200, {
        available: fs.existsSync(assetsDir),
        dataDir,
      });
      return;
    }

    if (pathname.startsWith('/hsr-assets/')) {
      serveSafeFile(res, assetsDir, pathname.slice('/hsr-assets/'.length), assetExts);
      return;
    }

    if (pathname.startsWith('/hsr-svg/')) {
      serveSafeFile(res, svgDir, pathname.slice('/hsr-svg/'.length), svgExts);
      return;
    }

    if (pathname.startsWith('/hsr-data/')) {
      serveSafeFile(res, jsonDir, pathname.slice('/hsr-data/'.length), jsonExts);
      return;
    }

    next();
  };
}

function hsrDataBridgePlugin(): Plugin {
  const envDir =
    process.env.HSR_CURRENCY_WARS_DATA ?? process.env.HSR_DATA_DIR;
  const dataDir = envDir
    ? path.resolve(envDir)
    : path.resolve(__dirname, '../../../hsr-currency-wars-data');

  return {
    name: 'hsr-data-bridge',
    configureServer(server) {
      server.middlewares.use(createBridgeMiddleware(dataDir));
    },
    configurePreviewServer(server) {
      server.middlewares.use(createBridgeMiddleware(dataDir));
    },
  };
}

export default defineConfig({
  plugins: [react(), hsrDataBridgePlugin()],
});
