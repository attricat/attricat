// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ExtensionPermissionsSection } from './ExtensionPermissionsSection';

vi.mock('./managementApi', () => ({
  grantExtension: vi.fn(),
  revokeExtensionGrant: vi.fn(),
}));

describe('ExtensionPermissionsSection', () => {
  it('names the permission each button grants or revokes', () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ExtensionPermissionsSection
          enabled
          extensionId="example"
          grants={[{ grant_kind: 'capability', grant_id: 'attricat.read' }]}
          manifest={{ permissions: ['attricat.read', 'attricat.write'] }}
          onChanged={() => undefined}
        />
      </QueryClientProvider>,
    );
    expect(
      screen.getByRole('button', {
        name: 'Revoke Capability: attricat.read',
      }),
    ).toBeTruthy();
    expect(
      screen.getByRole('button', { name: 'Grant Capability: attricat.write' }),
    ).toBeTruthy();
  });
});
