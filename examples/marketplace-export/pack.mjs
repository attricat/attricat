#!/usr/bin/env node
// Builds the marketplace export test extension and writes
// examples/marketplace-export/dist/marketplace-export-test.tar.zst.

import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { zstdCompressSync } from 'node:zlib';

const root = fileURLToPath(new URL('../../', import.meta.url));
const here = fileURLToPath(new URL('./', import.meta.url));
const run = (command, args) => execFileSync(command, args, { cwd: root, stdio: 'inherit' });

run('cargo', ['build', '-p', 'attricat-marketplace-test-component', '--target', 'wasm32-unknown-unknown', '--release']);
const component = `${root}target/marketplace-test.component.wasm`;
run('wasm-tools', ['component', 'new', `${root}target/wasm32-unknown-unknown/release/attricat_marketplace_test_component.wasm`, '-o', component]);

/** A minimal ustar archive of regular files. */
const tar = (files) => {
  const blocks = [];
  for (const [name, bytes] of files) {
    const header = Buffer.alloc(512);
    header.write(name, 0, 100);
    header.write('0000644\0', 100);
    header.write('0000000\0', 108);
    header.write('0000000\0', 116);
    header.write(`${bytes.length.toString(8).padStart(11, '0')}\0`, 124);
    header.write(`${Math.floor(Date.now() / 1000).toString(8).padStart(11, '0')}\0`, 136);
    header.write('        ', 148);
    header.write('0', 156);
    header.write('ustar\0', 257);
    header.write('00', 263);
    const checksum = header.reduce((sum, byte) => sum + byte, 0);
    header.write(`${checksum.toString(8).padStart(6, '0')}\0 `, 148);
    blocks.push(header, bytes, Buffer.alloc((512 - (bytes.length % 512)) % 512));
  }
  blocks.push(Buffer.alloc(1024));
  return Buffer.concat(blocks);
};

mkdirSync(`${here}dist`, { recursive: true });
const archive = `${here}dist/marketplace-export-test.tar.zst`;
writeFileSync(
  archive,
  zstdCompressSync(
    tar([
      ['manifest.json', readFileSync(`${here}extension/manifest.json`)],
      ['icon.svg', readFileSync(`${here}extension/icon.svg`)],
      ['server.wasm', readFileSync(component)],
      ['mapping.js', readFileSync(`${here}extension/mapping.js`)],
    ]),
  ),
);
console.log(archive);
