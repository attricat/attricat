// @vitest-environment jsdom
import { act, fireEvent, render } from '@testing-library/react';
import { useRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  gesture,
  navigationIconTransition,
  useNavigationIconActive,
} from './navigationIconMotion';

let reducedMotion = false;
vi.mock('motion/react', async (importOriginal) => ({
  ...(await importOriginal<typeof import('motion/react')>()),
  useReducedMotion: () => reducedMotion,
}));

const Probe = () => {
  const ref = useRef<SVGSVGElement>(null);
  const active = useNavigationIconActive(ref);
  return <svg data-active={String(active)} ref={ref} />;
};

const renderProbe = () => {
  const view = render(
    <button data-navigation-icon-trigger type="button">
      <Probe />
    </button>,
  );
  return {
    ...view,
    trigger: view.getByRole('button'),
    isActive: () =>
      view.container.querySelector('svg')?.getAttribute('data-active') ===
      'true',
  };
};

describe('gesture', () => {
  it('plays with the shared timing and rests instantly', () => {
    expect(gesture({ y: [0, -2, 0] }, { y: 0 }, 0.12)).toEqual({
      active: {
        y: [0, -2, 0],
        transition: { ...navigationIconTransition, delay: 0.12 },
      },
      rest: { y: 0, transition: { duration: 0 } },
    });
  });
});

describe('useNavigationIconActive', () => {
  beforeEach(() => {
    reducedMotion = false;
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it('finishes a started gesture after the pointer moves on', () => {
    const { isActive, trigger } = renderProbe();
    expect(isActive()).toBe(false);

    fireEvent.pointerEnter(trigger);
    expect(isActive()).toBe(true);
    fireEvent.pointerLeave(trigger);
    expect(isActive()).toBe(true);

    act(() => vi.advanceTimersByTime(1_000));
    expect(isActive()).toBe(false);
  });

  it('stays active while the row is still hovered', () => {
    const { isActive, trigger } = renderProbe();
    fireEvent.pointerEnter(trigger);
    act(() => vi.advanceTimersByTime(1_000));
    expect(isActive()).toBe(true);
    fireEvent.pointerLeave(trigger);
    expect(isActive()).toBe(false);
  });

  it('plays on keyboard focus but not on pointer focus', () => {
    const { isActive, trigger } = renderProbe();
    const matches = vi.spyOn(trigger, 'matches');

    matches.mockReturnValue(false);
    fireEvent.focus(trigger);
    expect(isActive()).toBe(false);

    fireEvent.blur(trigger);
    matches.mockReturnValue(true);
    fireEvent.focus(trigger);
    act(() => vi.advanceTimersByTime(1_000));
    expect(isActive()).toBe(true);
    fireEvent.blur(trigger);
    expect(isActive()).toBe(false);
  });

  it('never plays when reduced motion is preferred', () => {
    reducedMotion = true;
    const { isActive, trigger } = renderProbe();
    fireEvent.pointerEnter(trigger);
    expect(isActive()).toBe(false);
  });

  it('removes its listeners and timer on unmount', () => {
    const { trigger, unmount } = renderProbe();
    const removed = vi.spyOn(trigger, 'removeEventListener');
    fireEvent.pointerEnter(trigger);
    unmount();

    expect(removed.mock.calls.map(([type]) => type).sort()).toEqual([
      'blur',
      'focus',
      'pointerenter',
      'pointerleave',
    ]);
    expect(vi.getTimerCount()).toBe(0);
  });
});
