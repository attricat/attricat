import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  extensionOutletSchema,
  workspaceExtensionLayoutSchema,
} from './schemas';

const validLayout = {
  version: 1 as const,
  outlets: {
    navigation: {
      order: ['acme.navigation:entry'],
      hidden: [],
      promoted: ['acme.navigation:entry'],
    },
    entity_preview_panel: {
      order: ['acme.inventory:summary'],
      hidden: ['acme.legacy:summary'],
    },
  },
};

describe('extension outlet coverage', () => {
  it('keeps the browser schema in sync with the Rust manifest contract', () => {
    const manifest = readFileSync(
      fileURLToPath(
        new URL(
          '../../../../../crates/extension-manifest/src/extensions.rs',
          import.meta.url,
        ),
      ),
      'utf8',
    );
    const variants = manifest.match(/pub enum UiOutlet \{([\s\S]*?)\n\}/)?.[1];
    expect(variants).toBeDefined();
    const manifestOutlets = [
      ...variants!.matchAll(/^\s+([A-Z][A-Za-z0-9]+),\s*$/gm),
    ]
      .map(([, name]) =>
        name.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase(),
      )
      .sort();
    expect([...extensionOutletSchema.options].sort()).toEqual(manifestOutlets);
  });

  it('has a client mount path for every declared outlet', () => {
    const sourceRoot = fileURLToPath(new URL('../../', import.meta.url));
    const sources: string[] = [];
    const visit = (directory: string) => {
      for (const item of readdirSync(directory, { withFileTypes: true })) {
        const path = `${directory}/${item.name}`;
        if (item.isDirectory()) visit(path);
        else if (item.name.endsWith('.tsx') && !item.name.endsWith('.test.tsx'))
          sources.push(readFileSync(path, 'utf8'));
      }
    };
    visit(sourceRoot);
    const mounted = new Set(
      sources.flatMap((source) =>
        [...source.matchAll(/\boutlet(?:=|\s*===\s*)['"]([a-z_]+)['"]/g)].map(
          ([, outlet]) => outlet,
        ),
      ),
    );
    expect(
      [...extensionOutletSchema.options].filter(
        (outlet) => !mounted.has(outlet),
      ),
    ).toEqual([]);
  });
});

describe('workspaceExtensionLayoutSchema', () => {
  it('accepts strict versioned layouts with host-owned navigation promotion', () => {
    expect(workspaceExtensionLayoutSchema.parse(validLayout)).toEqual(
      validLayout,
    );
  });

  it.each([
    {
      ...validLayout,
      outlets: { unknown: { order: [], hidden: [] } },
    },
    {
      ...validLayout,
      outlets: {
        navigation: { order: [], hidden: [], promoted: [], selector: '#app' },
      },
    },
    {
      ...validLayout,
      outlets: {
        entity_preview_panel: {
          order: ['acme.inventory:summary'],
          hidden: ['acme.inventory:summary'],
        },
      },
    },
    {
      ...validLayout,
      outlets: {
        navigation: {
          order: [],
          hidden: ['acme.navigation:entry'],
          promoted: ['acme.navigation:entry'],
        },
      },
    },
    {
      ...validLayout,
      outlets: {
        entity_action: { order: ['not-a-key'], hidden: [] },
      },
    },
    {
      ...validLayout,
      outlets: {
        entity_action: { order: ['acme.shared:action'], hidden: [] },
        entity_preview_panel: { order: ['acme.shared:action'], hidden: [] },
      },
    },
  ])('rejects malformed or ambiguous layout %#', (layout) => {
    expect(workspaceExtensionLayoutSchema.safeParse(layout).success).toBe(
      false,
    );
  });
});
