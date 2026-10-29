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
  contribution_key: 'example.extension:attribute-panel',
  display_order: 0,
  navigation_group: null,
  capabilities: [],
  configuration: null,
  extension_id: 'example.extension',
  extension_name: 'Example Extension',
  id: 'attribute-panel',
  kind: 'panel',
  outlet: 'record_attribute_panel',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: 'Attribute panel',
  version: 1,
  invoke: null,
};

const collapse = (container: HTMLElement) =>
  container.querySelector<HTMLElement>('.MuiCollapse-root')!;

const renderFrame = () =>
  render(
    <ExtensionFrame
      contribution={contribution}
      placeholder={extensionFramePlaceholders.none}
    />,
  );

beforeEach(() =>
  Object.assign(frameState, { height: 0, measured: false, ready: false }),
);

describe('ExtensionFrame', () => {
  it('holds a skeleton in a reserved area by default', () => {
    render(<ExtensionFrame contribution={contribution} />);

    expect(screen.getByRole('status')).toBeTruthy();
  });

  it('takes no space without a placeholder until it measures content', () => {
    // Ready reports arrive before the first size, still at the default height.
    Object.assign(frameState, {
      height: defaultExtensionFrameHeight,
      ready: true,
    });
    const { container } = renderFrame();

    expect(screen.queryByRole('status')).toBeNull();
    expect(collapse(container).classList).toContain('MuiCollapse-hidden');
    expect(screen.getByTitle('Attribute panel')).toBeTruthy();
  });

  it('stays collapsed for a frame that renders nothing', () => {
    Object.assign(frameState, { height: 0, measured: true, ready: true });
    const { container } = renderFrame();

    expect(collapse(container).classList).toContain('MuiCollapse-hidden');
  });

  it('reveals a frame once it has measured content', () => {
    Object.assign(frameState, { height: 32, measured: true, ready: true });
    const { container } = renderFrame();

    expect(collapse(container).classList).not.toContain('MuiCollapse-hidden');
  });
});
