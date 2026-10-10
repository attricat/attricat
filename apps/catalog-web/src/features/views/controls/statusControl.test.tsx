// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@mui/material/styles';
import { makeTheme } from '../../../app/theme';
import '../../../i18n';
import type { StatusConfiguration } from '../../records/status';
import { StatusEditor } from './editors';
import { StatusValue } from './values';

const attribute = { code: 'Status', value_type: 'string' as const };

const config: StatusConfiguration = {
  version: 1,
  options: [
    { code: 'draft', label: 'Draft' },
    { code: 'live', label: 'Live', tone: 'success' },
    { code: 'done', label: 'Done' },
  ],
  transitions: [
    { from: null, to: 'draft' },
    { from: 'draft', to: 'live' },
    { from: 'live', to: 'done' },
  ],
};
afterEach(cleanup);
describe('status presentation and input', () => {
  for (const mode of ['light', 'dark'] as const) {
    it(`renders noninteractive status labels in ${mode} mode`, () => {
      render(
        <ThemeProvider theme={makeTheme(mode)}>
          <StatusValue config={config} value="live" />
        </ThemeProvider>,
      );
      expect(screen.getByText('Live')).toBeTruthy();
      expect(screen.queryByRole('button')).toBeNull();
      expect(screen.queryByRole('combobox')).toBeNull();
    });
  }
  it('preserves unknown values visibly', () => {
    render(<StatusValue config={config} value="retired" />);
    expect(screen.getByText('retired')).toBeTruthy();
    expect(screen.getByText('Unknown or retired status: retired')).toBeTruthy();
  });
  it('uses the saved baseline rather than the unsaved selection', () => {
    const onChange = vi.fn();
    render(
      <StatusEditor
        config={config}
        attribute={attribute}
        value="live"
        baseline="draft"
        inheritedValue={null}
        disabled={false}
        onChange={onChange}
      />,
    );
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Status' }));
    expect(
      screen
        .getByRole('option', { name: 'Done' })
        .getAttribute('aria-disabled'),
    ).toBe('true');
    fireEvent.click(screen.getByRole('option', { name: 'Done' }));
    expect(onChange).not.toHaveBeenCalled();
  });
  it('disables destinations whose server conditions are unmet and explains why', () => {
    const onChange = vi.fn();
    render(
      <StatusEditor
        config={config}
        attribute={attribute}
        value="draft"
        baseline="draft"
        inheritedValue={null}
        transitions={[
          {
            attribute_code: attribute.code,
            from: 'draft',
            to: 'live',
            code: null,
            allowed: false,
            denial_code: 'transition_conditions_unmet',
            denial_reason: 'SKU is required before going live',
            unmet: [
              {
                source: 'transition_condition',
                code: 'has-sku',
                message: 'SKU is required before going live',
                contexts: ['default'],
                attributes: ['sku'],
              },
            ],
          },
        ]}
        disabled={false}
        onChange={onChange}
      />,
    );
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Status' }));
    const live = screen.getByRole('option', { name: /Live/ });
    expect(live.getAttribute('aria-disabled')).toBe('true');
    expect(
      screen.getByText('Blocked: SKU is required before going live'),
    ).toBeTruthy();
    fireEvent.click(live);
    expect(onChange).not.toHaveBeenCalled();
  });
  it('disables keyboard and pointer mutation when readonly', () => {
    const onChange = vi.fn();
    render(
      <StatusEditor
        config={config}
        attribute={attribute}
        value="draft"
        baseline="draft"
        inheritedValue={null}
        disabled
        onChange={onChange}
      />,
    );
    const select = screen.getByRole('combobox');
    expect(select.getAttribute('aria-disabled')).toBe('true');
    fireEvent.keyDown(select, { key: 'ArrowDown' });
    fireEvent.mouseDown(select);
    expect(screen.queryByRole('listbox')).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
  });
  it('disables and explains edges the server denies to this user', () => {
    const onChange = vi.fn();
    render(
      <StatusEditor
        config={config}
        attribute={attribute}
        value="draft"
        baseline="draft"
        inheritedValue={null}
        transitions={[
          {
            attribute_code: 'Status',
            from: 'draft',
            to: 'live',
            code: 'approve',
            allowed: false,
            denial_code: 'status_separation_of_duties',
            denial_reason: 'must differ',
          },
        ]}
        disabled={false}
        onChange={onChange}
      />,
    );
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Status' }));
    const live = screen.getByRole('option', { name: /Live/ });
    expect(live.getAttribute('aria-disabled')).toBe('true');
    expect(
      screen.getByText('Must be made by someone other than the previous actor'),
    ).toBeTruthy();
    fireEvent.click(live);
    expect(onChange).not.toHaveBeenCalled();
  });
});
