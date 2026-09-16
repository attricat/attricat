import { describe, expect, it } from 'vitest';
import { workspaceExtensionLayoutSchema } from './schemas';

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
