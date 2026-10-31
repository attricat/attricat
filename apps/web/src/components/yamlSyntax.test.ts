import { describe, expect, it } from 'vitest';
import {
  parseStrictYaml,
  yamlPolicyErrors,
  type YamlPolicyError,
} from './yamlSyntax';

const parsed = (text: string) => {
  const result = parseStrictYaml(text);
  return 'data' in result ? result.data : result.error;
};

describe('parseStrictYaml', () => {
  // As serde-saphyr reads them with the API's options.
  it('types plain scalars as the API does', () => {
    expect(
      parsed(
        [
          'booleans: [true, False, TRUE, tRuE, yes, no, on, y]',
          'nulls: [null, Null, nUlL, ~, nil]',
          'integers: [0, -0, +5, 1_000, 0x1F, 0X1f, -0x10, 0o17, 0b101, 0x_1F]',
          'floats: [1.5, 1e3, .5, 5., 0012.5, 1_0.5, inf, nan]',
          "strings: ['true', !!str 5, ! 5]",
        ].join('\n'),
      ),
    ).toEqual({
      booleans: [true, false, true, true, 'yes', 'no', 'on', 'y'],
      floats: [1.5, 1000, 0.5, 5, 12.5, '1_0.5', 'inf', 'nan'],
      integers: [0, -0, 5, 1000, 31, 31, -16, 15, 5, '0x_1F'],
      nulls: [null, null, null, null, 'nil'],
      strings: ['true', '5', '5'],
    });
  });

  it('rejects values the API does not read exactly', () => {
    const rejected = (text: string, code: YamlPolicyError, value: string) =>
      expect(parsed(text)).toMatchObject({ policy: { code, value } });
    for (const value of ['.inf', '-.Inf', '.NaN', '1e999'])
      rejected(`x: ${value}`, yamlPolicyErrors.nonFiniteFloat, value);
    for (const value of ['012', '-012', '08', '00'])
      rejected(`x: ${value}`, yamlPolicyErrors.integerLeadingZero, value);
    rejected('x: !!int 012', yamlPolicyErrors.integerLeadingZero, '012');
    for (const value of ['18446744073709551616', '-9223372036854775809'])
      rejected(`x: ${value}`, yamlPolicyErrors.integerOutOfRange, value);
    for (const [tag, value] of [
      ['!foo', 'a'],
      ['!!binary', 'aGVsbG8='],
      ['!!timestamp', '2001-12-14'],
      ['!!set', '{a: null}'],
    ] as const)
      rejected(`x: ${tag} ${value}`, yamlPolicyErrors.unsupportedTag, tag);
    expect(parsed('x: !foo {a: 1}')).toMatchObject({
      column: 4,
      line: 1,
      message: 'unsupported tag `!foo`',
    });
  });

  it('reads exact integers, quoted digits and core tags', () => {
    expect(
      parsed(
        "max: 18446744073709551615\nmin: -9223372036854775808\ntext: '012'\n012: key\nd: !!seq [x]\ne: !!map {k: v}\n",
      ),
    ).toEqual({
      '012': 'key',
      d: ['x'],
      e: { k: 'v' },
      max: Number('18446744073709551615'),
      min: -9223372036854775808,
      text: '012',
    });
  });

  it('rejects keys written alike, such as 1 and "1"', () => {
    expect(parsed('1: a\n"1": b\n')).toMatchObject({ column: 1, line: 2 });
    expect(parsed("a: {1: x}\nb: {'1': y}\n")).toEqual({
      a: { 1: 'x' },
      b: { 1: 'y' },
    });
  });
});
