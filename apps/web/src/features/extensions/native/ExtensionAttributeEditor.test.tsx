// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { useEffect } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Attribute } from '../../records/api';
import type { ExtensionContribution } from '../api';
import { resolveAttributeEditor } from './attributeEditors';
import { ExtensionAttributeEditor } from './ExtensionAttributeEditor';

/** How the fake native UI behaves for one test. */
const native = vi.hoisted(() => ({
  outcome: 'ready' as 'ready' | 'failed' | 'starting',
  propose: undefined as ((value: unknown) => void) | undefined,
  context: undefined as Record<string, unknown> | undefined,
}));

vi.mock('./NativeUi', () => ({
  NativeUi: ({
    context,
    onFailure,
    onPropose,
    onReady,
  }: {
    context: Record<string, unknown>;
    onFailure: () => void;
    onPropose: (value: unknown) => void;
    onReady: () => void;
  }) => {
    native.propose = onPropose;
    native.context = context;
    useEffect(() => {
      if (native.outcome === 'ready') onReady();
      if (native.outcome === 'failed') onFailure();
    }, [onFailure, onReady]);
    return <input aria-label="Formula" />;
  },
}));
vi.mock('../useExtensionRuntime', () => ({
  useExtensionRuntime: () => ({ data: [] }),
}));
vi.mock('./nativeRuntime', () => ({
  nativeHostFor: () => ({ settle: () => Promise.resolve() }),
}));

const attribute = {
  code: 'price_gross',
  name: 'Gross price',
  value_type: 'string',
} as Attribute;

const editor = (overrides: Partial<ExtensionContribution> = {}) =>
  ({
    id: 'formula',
    version: 1,
    kind: 'editor',
    outlet: 'attribute_editor',
    rendering: 'native',
    extension_id: 'acme.formulas',
    extension_name: 'Formulas',
    editor: { value_types: ['string'], props: {} },
    ...overrides,
  }) as ExtensionContribution;

const reference = { id: 'formula', version: 1, props: { rows: 2 } };

const renderEditor = () => {
  const onChange = vi.fn();
  const onCommit = vi.fn();
  render(
    <ExtensionAttributeEditor
      attribute={attribute}
      contextId={null}
      contribution={editor()}
      fallback={<input aria-label="Default control" />}
      onChange={onChange}
      onCommit={onCommit}
      recordId="33333333-3333-4333-8333-333333333333"
      reference={reference}
      required
      value="price_net * 2"
    />,
  );
  return { onChange, onCommit };
};

beforeEach(() => {
  native.outcome = 'ready';
  native.propose = undefined;
});

describe('attribute editors', () => {
  it('use only an enabled native editor declared for the value type', () => {
    expect(
      resolveAttributeEditor([editor()], reference, attribute),
    ).toBeDefined();
    // Disabled or ungranted extensions are absent from the runtime.
    expect(resolveAttributeEditor([], reference, attribute)).toBeUndefined();
    expect(
      resolveAttributeEditor([editor({ version: 2 })], reference, attribute),
    ).toBeUndefined();
    expect(
      resolveAttributeEditor(
        [editor({ editor: { value_types: ['number'], props: {} } })],
        reference,
        attribute,
      ),
    ).toBeUndefined();
    expect(resolveAttributeEditor([editor()], null, attribute)).toBeUndefined();
  });

  it('gives the editor the value, attribute and props in a strict context', () => {
    renderEditor();
    expect(native.context).toMatchObject({
      value: 'price_net * 2',
      attribute: {
        code: 'price_gross',
        name: 'Gross price',
        value_type: 'string',
      },
      required: true,
      errors: [],
      props: { rows: 2 },
    });
    // The host draws the label.
    expect(screen.getByText('Gross price')).toBeTruthy();
    expect(screen.queryByLabelText('Default control')).toBeNull();
  });

  it('keeps the default control while the editor starts or after it fails', () => {
    native.outcome = 'starting';
    renderEditor();
    expect(screen.getByLabelText('Default control')).toBeTruthy();
    native.outcome = 'failed';
    renderEditor();
    expect(screen.getAllByLabelText('Default control')).toHaveLength(2);
  });

  it('keeps proposals as typing, and saves them on Enter or once focus has left', async () => {
    const { onChange, onCommit } = renderEditor();
    const input = screen.getByLabelText('Formula');
    input.focus();
    native.propose?.('price_net * 3');
    expect(onChange).toHaveBeenCalledWith('price_net * 3');
    expect(onCommit).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: 'Enter' });
    native.propose?.('price_net * 4');
    expect(onCommit).toHaveBeenCalledWith('price_net * 4');
    input.blur();
    native.propose?.('price_net * 5');
    expect(onCommit).toHaveBeenLastCalledWith('price_net * 5');
    // A value that cannot be the field's type never reaches the form.
    native.propose?.(42);
    expect(onCommit).toHaveBeenCalledTimes(2);
  });
});
