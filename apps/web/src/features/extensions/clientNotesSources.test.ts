import { describe, expect, it } from 'vitest';
import { mergeClientAnswer } from './clientNotesSources';
import type { FieldNotesResponse } from './fieldNotesData';
import { handlerFrameDocument } from './frameDocument';

const source = {
  extension_id: 'acme.client',
  extension_name: 'Acme client',
  release_id: '11111111-1111-4111-8111-111111111111',
  capabilities: ['attricat.read'],
};
const record = '33333333-3333-4333-8333-333333333333';

describe('client field notes sources', () => {
  it('adds a valid answer for readable records only, in extension order', () => {
    const records: FieldNotesResponse['records'] = {
      [record]: [
        {
          extension_id: 'zz.server',
          extension_name: 'Server',
          attributes: {},
          files: {},
          actions: {},
        },
      ],
    };
    expect(
      mergeClientAnswer(records, source, {
        records: {
          [record]: { attributes: { price: { note: 'From the client' } } },
          '44444444-4444-4444-8444-444444444444': { attributes: {} },
        },
      }),
    ).toBe(true);
    expect(records[record].map((entry) => entry.extension_id)).toEqual([
      'acme.client',
      'zz.server',
    ]);
    expect(records[record][0].attributes.price.note).toBe('From the client');
    expect(Object.keys(records)).toEqual([record]);
  });

  it('drops an answer that breaks the contract', () => {
    const records: FieldNotesResponse['records'] = { [record]: [] };
    expect(
      mergeClientAnswer(records, source, {
        records: {
          [record]: {
            attributes: { price: { badge: { label: 'x'.repeat(25) } } },
          },
        },
      }),
    ).toBe(false);
    expect(mergeClientAnswer(records, source, '<b>markup</b>')).toBe(false);
    expect(records[record]).toEqual([]);
  });

  it('runs handlers headless, without network or markup output', () => {
    expect(handlerFrameDocument).toContain("connect-src 'none'");
    expect(handlerFrameDocument).toContain(
      'module.handle(message.request, api)',
    );
    expect(handlerFrameDocument).not.toContain('mount(');
  });
});
