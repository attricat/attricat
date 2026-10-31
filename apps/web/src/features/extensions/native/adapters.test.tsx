// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { StackAdapter, TextFieldAdapter } from './adapters';
import {
  NativeFieldLabelContext,
  type NativeFieldLabel,
} from './nativeContexts';

const adapterProps = (props: Record<string, unknown>) => ({
  props,
  children: null,
  slots: {},
  emit: vi.fn(),
  listens: () => false,
  provideMethods: vi.fn(),
});

describe('native input adapters', () => {
  it('fill a column but keep their own width in a row', () => {
    const field = adapterProps({ label: 'Unit' });
    const { container } = render(
      <>
        <StackAdapter {...adapterProps({ direction: 'vertical', gap: 'sm' })}>
          <TextFieldAdapter {...field} />
        </StackAdapter>
        <StackAdapter {...adapterProps({ direction: 'horizontal', gap: 'sm' })}>
          <TextFieldAdapter {...field} />
        </StackAdapter>
      </>,
    );
    const controls = container.querySelectorAll('.MuiFormControl-root');
    expect(controls[0].classList).toContain('MuiFormControl-fullWidth');
    expect(controls[1].classList).not.toContain('MuiFormControl-fullWidth');
  });

  it('draw the field label on the first input that hides its own', () => {
    let owner: string | undefined;
    const fieldLabel: NativeFieldLabel = {
      text: 'margin formula',
      required: false,
      error: false,
      claim: (id, grant) => {
        if (owner && owner !== id) return;
        owner = id;
        grant(true);
      },
      release: vi.fn(),
    };
    render(
      <NativeFieldLabelContext.Provider value={fieldLabel}>
        <TextFieldAdapter
          {...adapterProps({ label: 'Formula', hideLabel: true })}
        />
        <TextFieldAdapter
          {...adapterProps({ label: 'Note', hideLabel: true })}
        />
      </NativeFieldLabelContext.Provider>,
    );
    // The first input shows the field's label; the second keeps its own as
    // its accessible name only.
    expect(screen.getByLabelText('margin formula')).toBeTruthy();
    expect(screen.getByLabelText('Note')).toBeTruthy();
    expect(screen.queryByText('Note')).toBeNull();
  });
});
