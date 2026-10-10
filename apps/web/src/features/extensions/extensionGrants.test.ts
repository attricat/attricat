import { expect, it } from 'vitest';
import { pendingGrants } from './extensionGrants';

it('lists distinct requested grants that are not granted yet', () => {
  expect(
    pendingGrants(
      {
        permissions: ['attricat.read', 'attricat.read'],
        optional_permissions: ['client.commands'],
        host_permissions: [{ id: 'api' }],
        event_contracts: {
          exports: [{ id: 'changed' }],
          consumes: [{ provider: 'other', contract: 'created' }],
        },
      },
      [{ grant_kind: 'capability', grant_id: 'client.commands' }],
    ),
  ).toEqual([
    { kind: 'capability', id: 'attricat.read' },
    { kind: 'host_permission', id: 'api' },
    { kind: 'event_publish', id: 'changed' },
    { kind: 'event_subscribe', id: 'other:created' },
  ]);
});
