// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ChannelChecksEditor } from './ChannelChecksEditor';
import { MAX_REQUIRED_RULE_CODES } from './constants';

const renderEditor = (requiredRuleCodes: readonly string[]) => {
  const onChange = vi.fn();
  render(
    <ChannelChecksEditor
      disabled={false}
      onChange={onChange}
      requireValidEntity={false}
      requiredRuleCodes={requiredRuleCodes}
      ruleCodes={[]}
    />,
  );
  return { onChange };
};

const addCode = (code: string) => {
  const input = screen.getByRole('combobox');
  fireEvent.change(input, { target: { value: code } });
  fireEvent.keyDown(input, { key: 'Enter' });
};

afterEach(cleanup);

describe('ChannelChecksEditor', () => {
  it('applies required rule codes within the limit', () => {
    const { onChange } = renderEditor([]);
    addCode('has-sku');
    expect(onChange).toHaveBeenCalledWith({ required_rule_codes: ['has-sku'] });
  });

  it('does not apply codes beyond the limit and says why', () => {
    const full = Array.from(
      { length: MAX_REQUIRED_RULE_CODES },
      (_, index) => `rule-${index}`,
    );
    const { onChange } = renderEditor(full);
    addCode('one-too-many');
    expect(onChange).not.toHaveBeenCalled();
    expect(
      screen.getByText(
        `A channel can require at most ${MAX_REQUIRED_RULE_CODES} rules. Remove one before adding another.`,
      ),
    ).toBeTruthy();
  });
});
