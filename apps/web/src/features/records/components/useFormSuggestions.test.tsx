// @vitest-environment jsdom
import { act, renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { extensionQueryKeys } from '../../extensions/queryKeys';
import type { BlueprintWithAttributes } from '../api';
import {
  useDefaultFromSuggestions,
  useFormAssistSuggestions,
  useFormSuggestions,
  useSuggestionNotes,
} from './useFormSuggestions';

const api = vi.hoisted(() => ({
  getDefaultValues: vi.fn(),
  getRecordLabels: vi.fn(),
}));
vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  getDefaultValues: api.getDefaultValues,
  getRecordLabels: api.getRecordLabels,
}));

const first = '123e4567-e89b-12d3-a456-426614174101';
const second = '123e4567-e89b-12d3-a456-426614174102';
const line: BlueprintWithAttributes = {
  blueprint: {
    id: '123e4567-e89b-12d3-a456-426614174000',
    code: 'line',
    name: 'Line',
    status: 'published',
    version: 2,
    views: {},
  },
  attributes: [
    { code: 'item', value_type: 'relationship', cardinality: 'one' },
    {
      code: 'price',
      value_type: 'number',
      default_from: 'item.sale_price_net',
    },
  ],
  table_path_attributes: [],
};

const prices: Record<string, number> = { [first]: 10, [second]: 20 };

/** The form's suggestion hooks over plain field state. */
const renderSuggestions = () => {
  let fields: Record<string, string> = { item: '', price: '' };
  const client = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const view = renderHook(
    () => {
      const suggestions = useFormSuggestions({
        getFields: () => fields,
        setFields: (next) => {
          fields = next;
        },
      });
      const suggest = useDefaultFromSuggestions({
        blueprint: line,
        contextId: null,
        enabled: true,
        getFields: () => fields,
        offer: suggestions.offer,
      });
      return {
        suggestions,
        suggest,
        notes: useSuggestionNotes(suggestions.suggestions),
      };
    },
    { wrapper },
  );
  return {
    view,
    fields: () => fields,
    /** The user picks `target` as the item. */
    pick: (target: string) =>
      act(() => {
        fields = { ...fields, item: target };
        view.result.current.suggest(line.attributes[0]!, target);
      }),
    /** The user types `value` into `code`. */
    type: (code: string, value: string) =>
      act(() => {
        fields = { ...fields, [code]: value };
        view.result.current.suggestions.markEdited(code);
      }),
  };
};

beforeEach(() => {
  api.getDefaultValues.mockReset();
  api.getDefaultValues.mockImplementation(
    ({ targetRecordId }: { targetRecordId: string }) =>
      Promise.resolve({
        values: [{ attribute_code: 'price', value: prices[targetRecordId] }],
      }),
  );
  api.getRecordLabels.mockResolvedValue({
    items: [
      { id: first, blueprint_code: 'product', display: { default: 'Bolt' } },
    ],
  });
});

describe('default_from suggestions', () => {
  it('fills on pick and refills while the field is unedited', async () => {
    const form = renderSuggestions();
    await form.pick(first);
    await waitFor(() => expect(form.fields().price).toBe('10'));
    expect(api.getDefaultValues).toHaveBeenCalledWith(
      expect.objectContaining({
        blueprintVersion: 2,
        relationship: 'item',
        targetRecordId: first,
      }),
      expect.any(AbortSignal),
    );
    await waitFor(() =>
      expect(form.view.result.current.notes.price).toBe('Suggested from Bolt.'),
    );

    await form.pick(second);
    await waitFor(() => expect(form.fields().price).toBe('20'));
  });

  it('keeps a value the user typed', async () => {
    const form = renderSuggestions();
    await form.pick(first);
    await waitFor(() => expect(form.fields().price).toBe('10'));
    await form.type('price', '12');
    expect(form.view.result.current.notes.price).toBeUndefined();

    await form.pick(second);
    await waitFor(() => expect(api.getDefaultValues).toHaveBeenCalledTimes(2));
    expect(form.fields().price).toBe('12');
  });

  it('ignores a stale response and a failed one', async () => {
    let resolveFirst: (value: unknown) => void = () => undefined;
    api.getDefaultValues.mockImplementationOnce(
      () => new Promise((resolve) => (resolveFirst = resolve)),
    );
    const form = renderSuggestions();
    await form.pick(first);
    await form.pick(second);
    await waitFor(() => expect(form.fields().price).toBe('20'));
    await act(async () =>
      resolveFirst({ values: [{ attribute_code: 'price', value: 10 }] }),
    );
    expect(form.fields().price).toBe('20');

    // A failed preview leaves the field to the save's own rule, never the
    // previous target's value.
    api.getDefaultValues.mockRejectedValueOnce(new Error('offline'));
    await form.pick(first);
    await waitFor(() => expect(api.getDefaultValues).toHaveBeenCalledTimes(3));
    expect(form.fields().price).toBe('');
  });

  it("withdraws the previous target's values as soon as another is picked", async () => {
    const form = renderSuggestions();
    await form.pick(first);
    await waitFor(() => expect(form.fields().price).toBe('10'));
    api.getDefaultValues.mockImplementationOnce(() => new Promise(() => {}));
    await form.pick(second);
    // Saving now must not store the first item's price for the second.
    expect(form.fields().price).toBe('');
    expect(form.view.result.current.notes.price).toBeUndefined();
  });

  it('empties its default when the item is cleared', async () => {
    const form = renderSuggestions();
    await form.pick(first);
    await waitFor(() => expect(form.fields().price).toBe('10'));
    await form.pick('');
    expect(form.fields().price).toBe('');
  });
});

const assistApi = vi.hoisted(() => ({
  getFormAssists: vi.fn(),
  runFormAssist: vi.fn(),
}));
vi.mock('../../extensions/api', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  getFormAssists: assistApi.getFormAssists,
  runFormAssist: assistApi.runFormAssist,
}));

const assisted: BlueprintWithAttributes = {
  ...line,
  attributes: [
    ...line.attributes,
    { code: 'quantity', value_type: 'number' },
    { code: 'discount', value_type: 'integer' },
    { code: 'note', value_type: 'string' },
  ],
};

/** The form's assist hook over plain field state. */
const renderAssists = () => {
  let fields: Record<string, string> = {
    item: '',
    price: '',
    quantity: '',
    discount: '',
    note: '',
  };
  const client = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const view = renderHook(
    () => {
      const suggestions = useFormSuggestions({
        getFields: () => fields,
        setFields: (next) => {
          fields = next;
        },
      });
      const changed = useFormAssistSuggestions({
        blueprint: assisted,
        contextId: null,
        enabled: true,
        getFields: () => fields,
        suggestions,
      });
      return {
        suggestions,
        changed,
        notes: useSuggestionNotes(suggestions.suggestions),
      };
    },
    { wrapper },
  );
  return {
    view,
    fields: () => fields,
    /** Waits for the form's assists to load. */
    ready: () =>
      waitFor(() =>
        expect(
          client.getQueryData(
            extensionQueryKeys.formAssists({
              blueprintId: assisted.blueprint.id,
              blueprintVersion: assisted.blueprint.version,
            }),
          ),
        ).toBeDefined(),
      ),
    /** The user types `value` into `code`. */
    type: (code: string, value: string) =>
      act(() => {
        fields = { ...fields, [code]: value };
        view.result.current.suggestions.markEdited(code);
        view.result.current.changed(code);
      }),
  };
};

const answer = (proposals: unknown[]) => ({ outcome: 'answered', proposals });

describe('form assist suggestions', () => {
  beforeEach(() => {
    assistApi.getFormAssists.mockResolvedValue({
      assists: [
        {
          extension_id: 'acme.pricing',
          extension_name: 'Pricing',
          id: 'customer_price',
          version: 1,
          outlet: 'record_form',
          watch: ['quantity'],
          proposes: ['price', 'discount'],
        },
      ],
    });
    assistApi.runFormAssist.mockReset();
  });

  it('runs for a field the form opened with, once the assists load', async () => {
    let loaded: (value: unknown) => void = () => undefined;
    assistApi.getFormAssists.mockImplementationOnce(
      () => new Promise((resolve) => (loaded = resolve)),
    );
    assistApi.runFormAssist.mockResolvedValue(
      answer([{ attribute_code: 'price', value: 30 }]),
    );
    const form = renderAssists();
    // The form reports its starting value before the assists have loaded.
    act(() => {
      form.view.result.current.changed('quantity');
    });
    expect(assistApi.runFormAssist).not.toHaveBeenCalled();
    await act(async () =>
      loaded({
        assists: [
          {
            extension_id: 'acme.pricing',
            extension_name: 'Pricing',
            id: 'customer_price',
            version: 1,
            outlet: 'record_form',
            watch: ['quantity'],
            proposes: ['price', 'discount'],
          },
        ],
      }),
    );
    await waitFor(() => expect(assistApi.runFormAssist).toHaveBeenCalled());
  });

  it('fills unedited proposed fields with valid values', async () => {
    assistApi.runFormAssist.mockResolvedValue(
      answer([
        { attribute_code: 'price', value: 30, note: 'Customer price' },
        // Not an integer, so the form leaves it out.
        { attribute_code: 'discount', value: 2.5 },
      ]),
    );
    const form = renderAssists();
    await form.ready();
    await form.type('quantity', '3');
    await waitFor(() => expect(form.fields().price).toBe('30'));
    expect(form.fields().discount).toBe('');
    expect(assistApi.runFormAssist).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'customer_price',
        values: { quantity: 3 },
        edited: [],
        record_id: null,
      }),
      expect.any(AbortSignal),
    );
    // The note can land a render after the value.
    await waitFor(() =>
      expect(form.view.result.current.notes.price).toBe('Customer price'),
    );
  });

  it('never replaces a field the user edited, and says so to the assist', async () => {
    assistApi.runFormAssist.mockResolvedValue(
      answer([{ attribute_code: 'price', value: 30 }]),
    );
    const form = renderAssists();
    await form.ready();
    await form.type('price', '12');
    await form.type('quantity', '3');
    await waitFor(() => expect(assistApi.runFormAssist).toHaveBeenCalled());
    expect(assistApi.runFormAssist.mock.calls[0]![0].edited).toEqual(['price']);
    expect(form.fields().price).toBe('12');
  });

  it('withdraws its proposals when a watched field changes, even if the next call fails', async () => {
    assistApi.runFormAssist.mockResolvedValueOnce(
      answer([{ attribute_code: 'price', value: 30 }]),
    );
    const form = renderAssists();
    await form.ready();
    await form.type('quantity', '3');
    await waitFor(() => expect(form.fields().price).toBe('30'));
    assistApi.runFormAssist.mockResolvedValue({
      outcome: 'timed_out',
      proposals: [],
    });
    await form.type('quantity', '4');
    expect(form.fields().price).toBe('');
    await waitFor(() =>
      expect(assistApi.runFormAssist).toHaveBeenCalledTimes(2),
    );
    expect(form.fields().price).toBe('');
  });

  it('ignores answers to values that changed and failed calls', async () => {
    let resolveFirst: (value: unknown) => void = () => undefined;
    assistApi.runFormAssist.mockImplementationOnce(
      () => new Promise((resolve) => (resolveFirst = resolve)),
    );
    const form = renderAssists();
    await form.ready();
    await form.type('quantity', '3');
    await waitFor(() => expect(assistApi.runFormAssist).toHaveBeenCalled());
    // The quantity changes before the first answer arrives.
    act(() => {
      form.view.result.current.suggestions.markEdited('quantity');
    });
    await act(async () => {
      await form.type('quantity', '4');
      resolveFirst(answer([{ attribute_code: 'price', value: 30 }]));
    });
    expect(form.fields().price).toBe('');

    assistApi.runFormAssist.mockResolvedValue({
      outcome: 'failed',
      proposals: [],
    });
    await waitFor(() =>
      expect(assistApi.runFormAssist).toHaveBeenCalledTimes(2),
    );
    expect(form.fields().price).toBe('');
  });
});
