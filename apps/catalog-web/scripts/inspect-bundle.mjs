import { gzipSync } from 'node:zlib';
import { readFile, stat } from 'node:fs/promises';
import { resolve } from 'node:path';

const distDirectory = resolve(import.meta.dirname, '..', 'dist');
const manifest = JSON.parse(
  await readFile(resolve(distDirectory, '.vite', 'manifest.json'), 'utf8'),
);
const entry = manifest['index.html'];

if (!entry) {
  throw new Error('The Vite manifest does not contain the index.html entry.');
}

const collectInitialFiles = (chunk, files = new Set()) => {
  if (files.has(chunk.file)) return files;

  files.add(chunk.file);
  for (const importName of chunk.imports ?? []) {
    collectInitialFiles(manifest[importName], files);
  }
  return files;
};

const formatBytes = (bytes) => `${(bytes / 1024).toFixed(1)} KiB`;
const getSize = async (file) => {
  const content = await readFile(resolve(distDirectory, file));
  return {
    file,
    gzip: gzipSync(content).length,
    raw: (await stat(resolve(distDirectory, file))).size,
  };
};

const initialFiles = collectInitialFiles(entry);
const initialAssets = await Promise.all([...initialFiles].map(getSize));
const allJavaScriptAssets = await Promise.all(
  Object.values(manifest)
    .filter((chunk) => chunk.file.endsWith('.js'))
    .map((chunk) => getSize(chunk.file)),
);
const initialGzipBudget = 300 * 1024;
const total = (assets, property) =>
  assets.reduce((size, asset) => size + asset[property], 0);

console.log('Initial JavaScript (index.html and synchronous imports):');
for (const asset of initialAssets.sort((a, b) => b.raw - a.raw)) {
  console.log(
    `  ${asset.file}: ${formatBytes(asset.raw)} (${formatBytes(asset.gzip)} gzip)`,
  );
}
console.log(
  `  total: ${formatBytes(total(initialAssets, 'raw'))} (${formatBytes(total(initialAssets, 'gzip'))} gzip)`,
);
console.log(
  `All JavaScript chunks: ${allJavaScriptAssets.length}, ${formatBytes(total(allJavaScriptAssets, 'raw'))} (${formatBytes(total(allJavaScriptAssets, 'gzip'))} gzip)`,
);

const initialGzipSize = total(initialAssets, 'gzip');
if (initialGzipSize > initialGzipBudget) {
  throw new Error(
    `Initial JavaScript exceeds the ${formatBytes(initialGzipBudget)} gzip budget: ${formatBytes(initialGzipSize)}.`,
  );
}
