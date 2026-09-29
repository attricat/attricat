import { describe, expect, it } from 'vitest';
import { parseContextMetadata } from './contextMetadata';

describe('parseContextMetadata', () => {
  it('accepts JSON objects', () => {
    expect(parseContextMetadata('{"locale":"en"}')).toEqual({
      data: { locale: 'en' },
    });
  });

  it('rejects invalid JSON and non-object values', () => {
    expect(parseContextMetadata('{')).toEqual({
      errorKey: 'contexts.invalidMetadataJson',
    });
    expect(parseContextMetadata('[]')).toEqual({
      errorKey: 'contexts.metadataMustBeObject',
    });
    expect(parseContextMetadata('null')).toEqual({
      errorKey: 'contexts.metadataMustBeObject',
    });
  });
});
