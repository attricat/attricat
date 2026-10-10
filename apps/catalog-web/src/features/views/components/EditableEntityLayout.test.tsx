// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../../i18n';
import type { Attribute, ViewDefinition } from '../../entities/api';
import { entityHeadingComponentId } from './blocks/EntityHeadingDefinition';
import { EditableEntityLayout } from './EditableEntityLayout';

const attribute = (code: string): Attribute => ({ code, value_type: 'string' });
const name = attribute('name');
const notes = attribute('notes');
const sku = attribute('sku');
const view = {
  type: 'stack',
  children: [
    {
      type: 'stack',
      component: { id: entityHeadingComponentId, version: 1, props: {} },
      children: [{ type: 'field', field: 'name' }],
    },
    { type: 'field', field: 'notes' },
  ],
} as ViewDefinition;
const renderEditor = (field: Attribute) => (
  <input aria-label={field.code} readOnly />
);

describe('EditableEntityLayout', () => {
  it('edits heading fields first, then the view, then fields it omits', () => {
    render(
      <EditableEntityLayout
        attributes={[name, notes, sku]}
        fallbackVisibilityScope="detail"
        headingAttributes={[name]}
        otherAttributes={[sku]}
        renderEditor={renderEditor}
        values={{}}
        view={view}
      />,
    );

    expect(
      screen
        .getAllByRole('textbox')
        .map((box) => box.getAttribute('aria-label')),
    ).toEqual(['name', 'notes', 'sku']);
    expect(
      screen.getByRole('heading', { level: 2, name: 'Other attributes' }),
    ).toBeTruthy();
  });

  it('leaves out empty heading and other-attribute groups', () => {
    render(
      <EditableEntityLayout
        attributes={[notes]}
        fallbackVisibilityScope="detail"
        headingAttributes={[]}
        headingLevel="h3"
        otherAttributes={[]}
        renderEditor={renderEditor}
        values={{}}
        view={view}
      />,
    );

    expect(screen.getAllByRole('textbox')).toHaveLength(1);
    expect(screen.queryByText('Other attributes')).toBeNull();
  });
});
