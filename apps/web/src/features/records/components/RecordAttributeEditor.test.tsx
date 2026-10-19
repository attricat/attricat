// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import '../../../i18n';
import type { Attribute } from '../api';
import { RecordAttributeEditor } from './RecordAttributeEditor';

const code: Attribute = { code: 'code', value_type: 'string' };

/** An editor whose stored value "abc" the new revision cannot take. */
const ReviewedEditor = ({ disabled = false }: { disabled?: boolean }) => {
  const [value, setValue] = useState('abc');
  const [discarded, setDiscarded] = useState(false);
  return (
    <RecordAttributeEditor
      attribute={code}
      contextId={null}
      disabled={disabled}
      files={[]}
      migrationReview={{
        discarded,
        fieldValue: 'abc',
        message: 'Now shorter',
        onDiscardChange: setDiscarded,
        previousValue: 'abc',
      }}
      onChange={setValue}
      reviewDisabled={false}
      value={value}
    />
  );
};

afterEach(cleanup);

describe('RecordAttributeEditor', () => {
  it('keeps a typed replacement when a discard is undone', async () => {
    const user = userEvent.setup();
    render(<ReviewedEditor />);
    const field = screen.getByRole('textbox', { name: 'code' });

    await user.click(
      screen.getByRole('button', { name: 'Discard previous value' }),
    );
    expect((field as HTMLInputElement).value).toBe('');
    await user.type(field, '42');
    await user.click(screen.getByRole('button', { name: 'Undo' }));

    expect((field as HTMLInputElement).value).toBe('42');
  });

  it('puts the stored value back when the field was left empty', async () => {
    const user = userEvent.setup();
    render(<ReviewedEditor />);

    await user.click(
      screen.getByRole('button', { name: 'Discard previous value' }),
    );
    await user.click(screen.getByRole('button', { name: 'Undo' }));

    expect(
      (screen.getByRole('textbox', { name: 'code' }) as HTMLInputElement).value,
    ).toBe('abc');
  });

  it('offers the discard on a field the user may not edit', () => {
    render(<ReviewedEditor disabled />);

    expect(
      (
        screen.getByRole('button', {
          name: 'Discard previous value',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false);
  });
});
