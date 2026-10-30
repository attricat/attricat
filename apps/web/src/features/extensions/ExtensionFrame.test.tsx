// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { ExtensionContribution } from './api';
import {
  defaultExtensionFrameHeight,
  extensionFramePlaceholders,
} from './constants';

const { frameState } = vi.hoisted(() => ({
  frameState: { height: 0, measured: false, ready: false },
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
  outlet: 'record_tab',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: 'Inspector',
  version: 1,
  invoke: null,
  when: null,
  confirm: null,
  icon: null,
  translations: {},
};

beforeEach(() =>
  Object.assign(frameState, { height: 0, measured: false, ready: false }),
);

const collapse = (container: HTMLElement) =>
  container.querySelector<HTMLElement>('.MuiCollapse-root')!;

const renderInline = () =>
  render(
    <ExtensionFrame
      contribution={contribution}
      placeholder={extensionFramePlaceholders.none}
    />,
  );

describe('ExtensionFrame', () => {
  it('holds a skeleton in a reserved area until the frame is ready', () => {
    const { rerender } = render(<ExtensionFrame contribution={contribution} />);
    expect(screen.getByRole('status')).toBeTruthy();

    Object.assign(frameState, { height: 32, ready: true });
    rerender(<ExtensionFrame contribution={contribution} />);
    expect(screen.queryByRole('status')).toBeNull();
    expect(screen.getByTitle('Inspector').style.height).toBe('32px');
  });

  it('takes no space without a placeholder until it measures content', () => {
    // Ready reports arrive before the first size, still at the default height.
    Object.assign(frameState, {
      height: defaultExtensionFrameHeight,
      ready: true,
    });
    const { container } = renderInline();

    expect(screen.queryByRole('status')).toBeNull();
    expect(collapse(container).classList).toContain('MuiCollapse-hidden');
  });

  it('stays collapsed for a frame that renders nothing', () => {
    Object.assign(frameState, { height: 0, measured: true, ready: true });
    const { container } = renderInline();

    expect(collapse(container).classList).toContain('MuiCollapse-hidden');
  });

  it('reveals a frame once it has measured content', () => {
    Object.assign(frameState, { height: 32, measured: true, ready: true });
    const { container } = renderInline();

    expect(collapse(container).classList).not.toContain('MuiCollapse-hidden');
  });
});
