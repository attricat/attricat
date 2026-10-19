// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useRef, useState } from 'react';
import { expect, it } from 'vitest';
import {
  FieldRevealContext,
  useFieldReveal,
  useFieldRevealRequest,
} from './fieldReveal';

// A collapsed block that only mounts once its tab is selected.
const Section = () => {
  const [open, setOpen] = useState(false);
  useFieldRevealRequest((code) => {
    if (code === 'nested') setOpen(true);
  });
  return open ? (
    <div data-field-code="nested">
      <input aria-label="nested" />
    </div>
  ) : null;
};

const Tabs = () => {
  const [tab, setTab] = useState(1);
  useFieldRevealRequest(() => setTab(2));
  return tab === 2 ? <Section /> : <input aria-label="first" />;
};

const Form = () => {
  const root = useRef<HTMLDivElement>(null);
  const { request, reveal } = useFieldReveal(root);
  return (
    <div ref={root}>
      <button onClick={() => reveal('nested')} type="button">
        Reveal
      </button>
      <FieldRevealContext value={request}>
        <Tabs />
      </FieldRevealContext>
    </div>
  );
};

it('opens a block that mounts while a reveal is open', async () => {
  render(<Form />);

  fireEvent.click(screen.getByRole('button', { name: 'Reveal' }));

  await waitFor(() =>
    expect(document.activeElement).toBe(screen.getByLabelText('nested')),
  );
});
