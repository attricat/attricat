// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../i18n';
import { ExtensionUpdateChip } from './ExtensionUpdateChip';

describe('ExtensionUpdateChip', () => {
  it('names the version an extension can upgrade to', () => {
    render(
      <ExtensionUpdateChip
        update={{
          extension_id: 'acme.widget',
          installed_version: '1.0.0',
          latest_version: '1.1.0',
          repository: 'github:acme/widget',
          registry_source: 'github:attricat/attricat-extensions',
        }}
      />,
    );
    expect(screen.getByText('Update available: 1.1.0')).toBeTruthy();
  });

  it('shows nothing without an update', () => {
    const { container } = render(<ExtensionUpdateChip update={undefined} />);
    expect(container.textContent).toBe('');
  });
});
