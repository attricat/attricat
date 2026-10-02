import { describe, expect, it } from 'vitest';
import fixture from '../../../../../contracts/lexicon-references.json';
import { parseLexiconText } from './references';

describe('parseLexiconText', () => {
  it.each(fixture.cases)('matches the shared case $input', (testCase) => {
    const { input, ...expected } = testCase;
    expect(parseLexiconText(input)).toEqual(expected);
  });
});
