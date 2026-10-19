// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { focusFirstField, useInitialFocus } from './initialFocus';

const Form = ({
  focusKey,
  loaded = true,
}: {
  focusKey: string | null;
  loaded?: boolean;
}) => {
  const ref = useInitialFocus<HTMLFormElement>(focusKey);
  return (
    <form ref={ref}>
      {loaded && (
        <>
          <input aria-label="Name" />
          <input aria-label="Code" />
        </>
      )}
    </form>
  );
};

describe('focusFirstField', () => {
  it('skips fields the user cannot change and prefers matching sections', () => {
    render(
      <div data-testid="root">
        <input type="hidden" />
        <input aria-label="Upload" type="file" />
        <input aria-label="Agree" type="checkbox" />
        <input aria-label="Disabled" disabled />
        <input aria-label="Read only" readOnly />
        <div aria-hidden="true">
          <input aria-label="Hidden" />
        </div>
        <input aria-label="Title" />
        <section data-review>
          <textarea aria-label="Notes" />
        </section>
      </div>,
    );
    const root = screen.getByTestId('root');
    expect(focusFirstField(root)).toBe(true);
    expect(document.activeElement).toBe(screen.getByLabelText('Title'));
    focusFirstField(root, '[data-review]');
    expect(document.activeElement).toBe(screen.getByLabelText('Notes'));
  });

  it('reports when nothing can be focused', () => {
    render(<button data-testid="root">Close</button>);
    expect(focusFirstField(screen.getByTestId('root'))).toBe(false);
  });
});

describe('useInitialFocus', () => {
  it('waits for a key, focuses once, and again for a new key', () => {
    const view = render(<Form focusKey={null} />);
    expect(document.activeElement).toBe(document.body);
    view.rerender(<Form focusKey="a" />);
    expect(document.activeElement).toBe(screen.getByLabelText('Name'));
    screen.getByLabelText('Code').focus();
    view.rerender(<Form focusKey="a" />);
    expect(document.activeElement).toBe(screen.getByLabelText('Code'));
    view.rerender(<Form focusKey="b" />);
    expect(document.activeElement).toBe(screen.getByLabelText('Name'));
  });

  it('focuses fields that render later, unless the user acted first', () => {
    const view = render(<Form focusKey="a" loaded={false} />);
    view.rerender(<Form focusKey="a" />);
    expect(document.activeElement).toBe(screen.getByLabelText('Name'));
    view.rerender(<Form focusKey="b" loaded={false} />);
    fireEvent.pointerDown(document.body);
    view.rerender(<Form focusKey="b" />);
    expect(document.activeElement).not.toBe(screen.getByLabelText('Name'));
  });
});
