// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import type { Attribute, ComponentReference } from '../../entities/api';
import { EntityAttributeEditor } from '../../entities/components/EntityAttributeEditor';
import { EntityFormAttributeEditor } from '../../entities/components/EntityFormAttributeEditor';
import { EntityView } from './EntityView';
import { PhoneValue } from './values/PhoneValue';

const attribute: Attribute = { code: 'phone', value_type: 'string' };
const component: ComponentReference = {
  id: 'catalog.phone_edit',
  version: 1,
  props: {},
};
afterEach(cleanup);

it('renders a safe link, preserves its label and does not activate its row', () => {
  const onClick = vi.fn();
  render(
    <div role="button" tabIndex={0} onClick={onClick} onKeyDown={onClick}>
      <PhoneValue attribute={attribute} value="+48 22 123 45 67" />
    </div>,
  );
  const link = screen.getByRole('link');
  expect(link.textContent).toBe('+48 22 123 45 67');
  expect(link.getAttribute('href')).toBe('tel:+48221234567');
  link.addEventListener('click', (event) => event.preventDefault());
  fireEvent.click(link);
  expect(onClick).not.toHaveBeenCalled();
});

it('keeps national and unsafe values readable without links', () => {
  render(
    <>
      <PhoneValue attribute={attribute} value="020 7946 0958" />
      <PhoneValue attribute={attribute} value="javascript:alert(1)" />
    </>,
  );
  expect(screen.queryByRole('link')).toBeNull();
  expect(screen.getByText('020 7946 0958')).toBeTruthy();
});

it.each([null, undefined, '', {}, 123])(
  'renders missing or malformed data safely: %s',
  (value) => {
    render(<PhoneValue attribute={attribute} value={value} />);
    expect(screen.queryByRole('link')).toBeNull();
  },
);

it.each([
  { ...attribute, readonly: true },
  { ...attribute, context_editable: 'default' as const },
])('keeps restricted phone fields disabled', (restrictedAttribute) => {
  render(
    <EntityFormAttributeEditor
      attribute={restrictedAttribute}
      component={component}
      contextId="translation"
      defaultContextId="default"
      existingValues={[]}
      fieldErrors={{}}
      highlightedAttributes={[]}
      migrationReviewMessages={{}}
      resolvedValues={{}}
      onChange={vi.fn()}
      value="+48 22 123 45 67"
    />,
  );
  expect((screen.getByRole('textbox') as HTMLInputElement).disabled).toBe(true);
});

it('passes configured components to the form-owned editor', () => {
  const editor = vi.fn(() => <span>Editor</span>);
  render(
    <EntityView
      attributes={[attribute]}
      values={{}}
      renderEditor={editor}
      view={{
        type: 'stack',
        children: [{ type: 'field', field: 'phone', component }],
      }}
    />,
  );
  expect(editor).toHaveBeenCalledWith(attribute, component);
});

it('uses telephone input without normalizing edits and respects disabled state', () => {
  const onChange = vi.fn();
  const props = {
    attribute,
    component,
    contextId: null,
    disabled: false,
    files: [],
    onChange,
    showMigrationBadge: false,
    value: '020 7946 0958',
  };
  const { rerender } = render(<EntityAttributeEditor {...props} />);
  const input = screen.getByRole('textbox') as HTMLInputElement;
  expect(input.type).toBe('tel');
  expect(input.inputMode).toBe('tel');
  fireEvent.change(input, { target: { value: '+1 (202) 555-0123 x0042' } });
  expect(onChange).toHaveBeenCalledWith('+1 (202) 555-0123 x0042');
  rerender(<EntityAttributeEditor {...props} disabled />);
  expect(input.disabled).toBe(true);
  rerender(<EntityAttributeEditor {...props} component={undefined} />);
  expect(input.type).toBe('text');
});
