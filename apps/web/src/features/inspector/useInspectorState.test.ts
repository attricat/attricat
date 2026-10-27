// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { INSPECTOR_STATE_STORAGE_KEY, inspectorPaneIds } from './constants';
import { useInspectorState } from './useInspectorState';

describe('useInspectorState', () => {
  it('reads the legacy expanded flag', () => {
    localStorage.setItem(INSPECTOR_STATE_STORAGE_KEY, 'true');
    const { result } = renderHook(useInspectorState);
    expect(result.current[0]).toEqual({
      activePane: inspectorPaneIds.server,
      expanded: true,
    });
  });

  it('keeps a valid field when the other one is stale', () => {
    localStorage.setItem(
      INSPECTOR_STATE_STORAGE_KEY,
      JSON.stringify({ activePane: 'removed', expanded: true }),
    );
    const { result } = renderHook(useInspectorState);
    expect(result.current[0]).toEqual({
      activePane: inspectorPaneIds.server,
      expanded: true,
    });
  });

  it('applies and stores every update made in one tick', () => {
    const { result } = renderHook(useInspectorState);
    act(() => {
      result.current[1]({ expanded: true });
      result.current[1]({ activePane: inspectorPaneIds.session });
    });
    const expected = { activePane: inspectorPaneIds.session, expanded: true };
    expect(result.current[0]).toEqual(expected);
    expect(
      JSON.parse(localStorage.getItem(INSPECTOR_STATE_STORAGE_KEY)!),
    ).toEqual(expected);
  });
});
