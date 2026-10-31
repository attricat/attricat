import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  releasedUiKitSnapshots,
  uiKitFiles,
} from '../../../../../../scripts/ui-kit/generate';

const root = fileURLToPath(new URL('../../../../../../', import.meta.url));
const update = process.env.UPDATE_CONTRACTS === '1';

const write = (path: string, content: string) => {
  mkdirSync(dirname(join(root, path)), { recursive: true });
  writeFileSync(join(root, path), content);
};

describe('UI kit contract', () => {
  it('keeps every generated file current', () => {
    for (const [path, content] of Object.entries(uiKitFiles())) {
      if (update) write(path, content);
      expect(readFileSync(join(root, path), 'utf8'), path).toBe(content);
    }
  });

  it('never changes a released version', () => {
    for (const [path, content] of Object.entries(releasedUiKitSnapshots())) {
      // A new release's snapshot is written once; later runs only compare.
      if (update && !existsSync(join(root, path))) write(path, content);
      expect(readFileSync(join(root, path), 'utf8'), path).toBe(content);
    }
  });
});
