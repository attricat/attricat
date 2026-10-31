// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useRef, useState } from 'react';
import { expect, it, vi } from 'vitest';
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

const Follower = () => {
  const root = useRef<HTMLDivElement>(null);
  const { reveal } = useFieldReveal(root);
  return (
    <div data-testid="panel" style={{ overflowY: 'auto' }}>
      <div ref={root}>
        <button onClick={() => reveal('far', { focus: false })} type="button">
          Follow
        </button>
        <div data-field-code="far" data-testid="far" />
      </div>
    </div>
  );
};

/** Lays the panel out to show `panelHeight` of 500px, the field below it. */
const layOut = (panelHeight: number) => {
  const panel = screen.getByTestId('panel');
  Object.defineProperties(panel, {
    clientHeight: { value: panelHeight },
    scrollHeight: { value: 500 },
    scrollTop: { value: 0, writable: true },
  });
  screen.getByTestId('far').getBoundingClientRect = () =>
    ({ bottom: 340, top: 300 }) as DOMRect;
  return panel;
};

it('follows a field within its panel without scrolling the page', async () => {
  const scrollIntoView = vi.fn();
  Element.prototype.scrollIntoView = scrollIntoView;
  render(<Follower />);
  const panel = layOut(100);

  fireEvent.click(screen.getByRole('button', { name: 'Follow' }));

  await waitFor(() => expect(panel.scrollTop).toBe(240));
  expect(scrollIntoView).not.toHaveBeenCalled();
});

it('leaves a panel that does not scroll on its own as it is', async () => {
  render(<Follower />);
  const panel = layOut(500);

  fireEvent.click(screen.getByRole('button', { name: 'Follow' }));

  await new Promise(requestAnimationFrame);
  expect(panel.scrollTop).toBe(0);
  expect(document.documentElement.scrollTop).toBe(0);
});
