import { describe, expect, it } from 'vitest';
import type { Attribute } from '../records/api';
import { mergePreviewValues, previewRecordValues } from './previewRecordValues';

const attributes: Attribute[] = [
  { code: 'name', value_type: 'string' },
  { code: 'doors', value_type: 'integer' },
  {
    code: 'dealer',
    value_type: 'relationship',
    target_blueprint_code: 'dealer',
  },
  { code: 'photos', value_type: 'file' },
];

describe('previewRecordValues', () => {
  it('fills the sandbox from scalars and keeps links and files apart', () => {
    const dealer = { items: [{ id: 'd1', display: 'Main dealer' }] };
    const photos = [{ id: 'f1', filename: 'front.jpg' }];
    const fitted = previewRecordValues(attributes, {
      name: { value: 'Golf' },
      doors: { value: 5 },
      dealer: { value: dealer },
      photos: { value: photos },
      // An attribute the previewed version no longer has is ignored.
      removed: { value: 'gone' },
    });
    expect(fitted).toEqual({
      fields: { name: 'Golf', doors: '5' },
      linked: { dealer: { value: dealer }, photos: { value: photos } },
      unfit: [],
    });
  });

  it('reports values whose attribute changed type in the previewed version', () => {
    const fitted = previewRecordValues(attributes, {
      // Stored as text before `doors` became an integer.
      doors: { value: 'five' },
      // Stored as text before `dealer` became a relationship.
      dealer: { value: 'Main dealer' },
    });
    expect(fitted.unfit).toEqual(['doors', 'dealer']);
    expect(fitted.fields).toEqual({});
    expect(fitted.linked).toEqual({});
  });
});

describe('mergePreviewValues', () => {
  it('takes what the sandbox edits from it and links from the record', () => {
    const dealer = { value: { items: [{ id: 'd1' }] } };
    expect(
      mergePreviewValues(
        attributes,
        // A cleared sandbox field stays cleared; the record does not refill it.
        { doors: { value: 3 } },
        { dealer, name: { value: 'ignored' } },
      ),
    ).toEqual({ doors: { value: 3 }, dealer });
  });
});
