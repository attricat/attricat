// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import type { Attribute } from '../api';
import { ScalarAttributeEditor } from './ScalarAttributeEditor';

const urlEdit = { id: 'catalog.url_edit', version: 1, props: {} };
const status: Attribute = {
  code: 'state',
  value_type: 'string',
  value_schema: {
    type: 'string',
    enum: ['draft'],
    'x-attricat-status': {
      version: 1,
      options: [{ code: 'draft', label: 'Draft' }],
    },
  },
};

const renderEditor = (
  attribute: Attribute,
  component?: typeof urlEdit | null,
  value = '',
) =>
  render(
    <ScalarAttributeEditor
      attribute={attribute}
      component={component}
      value={value}
      disabled={false}
      onChange={vi.fn()}
    />,
  );

afterEach(cleanup);

describe('ScalarAttributeEditor', () => {
  it('prefers the status control over a configured component', () => {
    renderEditor(status, urlEdit);
    expect(screen.getByRole('combobox', { name: 'state' })).toBeTruthy();
  });

  it('uses a configured component that supports the value type', () => {
    renderEditor({ code: 'site', value_type: 'string' }, urlEdit);
    expect(screen.getByRole('textbox').getAttribute('type')).toBe('url');
  });

  it('falls back to the built-in input for unsupported components', () => {
    renderEditor({ code: 'count', value_type: 'integer' }, urlEdit);
    expect(screen.getByRole('spinbutton', { name: 'count' })).toBeTruthy();
  });

  it.each(['', '2026-09-30'])(
    'keeps native date labels floated for value %j',
    (value) => {
      renderEditor({ code: 'recorded_on', value_type: 'date' }, null, value);
      expect(screen.getByLabelText('recorded on').getAttribute('type')).toBe(
        'date',
      );
      expect(
        screen
          .getByText('recorded on', { selector: 'label' })
          .getAttribute('data-shrink'),
      ).toBe('true');
    },
  );

  it('preserves normal floating-label behavior for an empty text field', () => {
    renderEditor({ code: 'name', value_type: 'string' });
    expect(
      screen
        .getByText('name', { selector: 'label' })
        .getAttribute('data-shrink'),
    ).toBe('false');
  });

  it.each([
    ['boolean', 'combobox'],
    ['date', null],
    ['time', 'textbox'],
  ] as const)('renders the built-in %s input', (valueType, role) => {
    const { container } = renderEditor({ code: 'v', value_type: valueType });
    if (role) expect(screen.getByRole(role)).toBeTruthy();
    else expect(container.querySelector('input[type="date"]')).toBeTruthy();
  });
});
