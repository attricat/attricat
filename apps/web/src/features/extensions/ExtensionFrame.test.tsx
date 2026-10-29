// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { ExtensionContribution } from './api';

const { frameState } = vi.hoisted(() => ({
  frameState: { height: 0, ready: false },
}));

vi.mock('./useExtensionFrame', () => ({
  useExtensionFrame: () => ({
    errorKey: undefined,
    frameKey: 'frame',
    handleFrameLoad: () => {},
    iframeRef: { current: null },
    ...frameState,
  }),
}));

import { ExtensionFrame } from './ExtensionFrame';

const contribution: ExtensionContribution = {
  contribution_key: 'example.extension:inspector',
  display_order: 0,
  navigation_group: null,
  capabilities: [],
  configuration: null,
  extension_id: 'example.extension',
  extension_name: 'Example Extension',
  id: 'inspector',
  kind: 'embedded',
  outlet: 'record_preview_panel',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: 'Inspector',
  version: 1,
  invoke: null,
};

beforeEach(() => Object.assign(frameState, { height: 0, ready: false }));

describe('ExtensionFrame', () => {
  it('holds a skeleton in a reserved area until the frame is ready', () => {
    const { rerender } = render(<ExtensionFrame contribution={contribution} />);
    expect(screen.getByRole('status')).toBeTruthy();

    Object.assign(frameState, { height: 32, ready: true });
    rerender(<ExtensionFrame contribution={contribution} />);
    expect(screen.queryByRole('status')).toBeNull();
    expect(screen.getByTitle('Inspector').style.height).toBe('32px');
  });
});
