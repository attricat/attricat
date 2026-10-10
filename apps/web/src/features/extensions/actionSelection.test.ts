import { describe, expect, it } from 'vitest';
import type { ExtensionContribution } from './api';
import { selectionSources } from './constants';
import {
  contributionContext,
  outletContributions,
} from './outletContributions';

const contribution = (
  version: number,
  outlet: ExtensionContribution['outlet'] = 'explorer_bulk_action',
): ExtensionContribution => ({
  contribution_key: `acme.docs:v${version}`,
  display_order: version,
  navigation_group: null,
  capabilities: [],
  configuration: null,
  extension_id: 'acme.docs',
  extension_name: 'Docs',
  id: `v${version}`,
  kind: 'action',
  outlet,
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: null,
  version,
});

const legacyContext = {
  context_version: 1,
  blueprint_id: '22222222-2222-4222-8222-222222222222',
  blueprint_version: 3,
  record_ids: ['33333333-3333-4333-8333-333333333333'],
};
const selection = {
  source: selectionSources.explorerSelection,
  blueprintId: legacyContext.blueprint_id,
  blueprintVersion: 3,
  contextId: null,
  recordIds: [
    '33333333-3333-4333-8333-333333333333',
    '44444444-4444-4444-8444-444444444444',
  ],
};

describe('selection-aware contribution contexts', () => {
  it('keeps the released context for version 1 contributions', () => {
    expect(contributionContext(contribution(1), legacyContext, selection)).toBe(
      legacyContext,
    );
  });

  it('gives version 2 contributions the normalized ordered selection', () => {
    expect(
      contributionContext(contribution(2), legacyContext, selection),
    ).toEqual({
      context_version: 2,
      selection_source: 'explorer_selection',
      blueprint_id: legacyContext.blueprint_id,
      blueprint_version: 3,
      context_id: null,
      record_ids: selection.recordIds,
    });
  });

  it('mounts version 2 contributions only where a selection is supplied', () => {
    const runtime = [contribution(1), contribution(2)];
    expect(
      outletContributions(runtime, 'explorer_bulk_action', legacyContext).map(
        (item) => item.version,
      ),
    ).toEqual([1]);
    expect(
      outletContributions(
        runtime,
        'explorer_bulk_action',
        legacyContext,
        selection,
      ).map((item) => item.version),
    ).toEqual([1, 2]);
  });

  it('rejects selections the host contract does not allow', () => {
    const tooLarge = {
      ...selection,
      recordIds: Array.from(
        { length: 51 },
        (_, index) =>
          `00000000-0000-4000-8000-${String(index).padStart(12, '0')}`,
      ),
    };
    expect(
      outletContributions(
        [contribution(2)],
        'explorer_bulk_action',
        legacyContext,
        tooLarge,
      ),
    ).toEqual([]);
  });
});
