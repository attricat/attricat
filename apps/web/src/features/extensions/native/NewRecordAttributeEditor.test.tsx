// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { useEffect } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Attribute } from '../../records/api';
import type { ExtensionContribution } from '../api';
import { NewRecordAttributeEditor } from './NewRecordAttributeEditor';

const native = vi.hoisted(() => ({
  runtime: [] as Partial<ExtensionContribution>[],
  propose: undefined as ((value: unknown) => void) | undefined,
  context: undefined as Record<string, unknown> | undefined,
}));

vi.mock('./NativeUi', () => ({
  NativeUi: ({
    context,
    onPropose,
    onReady,
  }: {
    context: Record<string, unknown>;
    onPropose: (value: unknown) => void;
    onReady: () => void;
  }) => {
    native.propose = onPropose;
    native.context = context;
    useEffect(() => onReady(), [onReady]);
    return <input aria-label="Formula" />;
  },
}));
vi.mock('../useExtensionRuntime', () => ({
  useExtensionRuntime: () => ({ data: native.runtime }),
}));
vi.mock('./nativeRuntime', () => ({
  nativeHostFor: () => ({ settle: () => Promise.resolve() }),
  nativeHostKey: () => 'host',
}));

const attribute = {
  code: 'margin',
  name: 'Margin',
  value_type: 'string',
} as Attribute;
const reference = {
  extension_id: 'acme.formulas',
  id: 'formula',
  version: 1,
  props: {},
};
const contribution = {
  id: 'formula',
  version: 1,
  kind: 'editor',
  outlet: 'attribute_editor',
  rendering: 'native',
  extension_id: 'acme.formulas',
  editor: { value_types: ['string'], props: {} },
} as Partial<ExtensionContribution>;

const renderEditor = (onChange = vi.fn()) => {
  render(
    <NewRecordAttributeEditor
      attribute={attribute}
      contextId={null}
      fallback={<input aria-label="Default control" />}
      onChange={onChange}
      readOnly={false}
      reference={reference}
      required={false}
      value=""
    />,
  );
  return onChange;
};

describe('NewRecordAttributeEditor', () => {
  beforeEach(() => {
    native.runtime = [];
    native.propose = undefined;
    native.context = undefined;
  });

  it('keeps the default control while no extension provides the editor', () => {
    renderEditor();
    expect(screen.getByLabelText('Default control')).toBeTruthy();
    expect(native.context).toBeUndefined();
  });

  it('draws the editor for a record without an ID and keeps its proposals as changes', () => {
    native.runtime = [contribution];
    const onChange = renderEditor();
    expect(screen.queryByLabelText('Default control')).toBeNull();
    expect(native.context?.record_id).toBeNull();
    native.propose?.('price * 2');
    expect(onChange).toHaveBeenCalledWith('price * 2');
  });
});
