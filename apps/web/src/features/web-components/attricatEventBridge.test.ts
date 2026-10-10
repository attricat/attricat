import { describe, expect, it, vi } from 'vitest';
import {
  attricatEventNames,
  dispatchContextChanged,
  dispatchRecordUpdated,
  requestRecordRefresh,
  requestNavigation,
  requestNotification,
} from './attricatEvents';
import { installAttricatEventBridge } from './attricatEventBridge';

const recordId = '123e4567-e89b-12d3-a456-426614174000';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const correlationId = '123e4567-e89b-12d3-a456-426614174002';

describe('catalog web-component event bridge', () => {
  it('sends only the client-safe record update contract to a component', () => {
    const component = new EventTarget();
    const received = vi.fn();
    component.addEventListener(attricatEventNames.recordUpdated, (event) =>
      received((event as CustomEvent).detail),
    );

    dispatchRecordUpdated(component, {
      record_id: recordId,
      change_hints: ['attribute_values'],
      correlation_id: correlationId,
    });

    expect(received).toHaveBeenCalledWith({
      record_id: recordId,
      change_hints: ['attribute_values'],
      correlation_id: correlationId,
    });
  });

  it('validates component requests before invoking host behavior', () => {
    const host = new EventTarget();
    const onRefreshRecord = vi.fn();
    const onNavigate = vi.fn();
    const onNotify = vi.fn();
    const dispose = installAttricatEventBridge({
      host,
      onRefreshRecord,
      onNavigate,
      onNotify,
    });

    requestRecordRefresh(host, {
      record_id: recordId,
      change_hints: ['relationships'],
    });
    requestNavigation(host, {
      record_id: recordId,
      correlation_id: correlationId,
    });
    requestNotification(host, { message: 'Saved', severity: 'success' });
    host.dispatchEvent(
      new CustomEvent(attricatEventNames.refreshRecord, {
        detail: { record_id: recordId, payload: { secret: true } },
      }),
    );

    expect(onRefreshRecord).toHaveBeenCalledWith({
      record_id: recordId,
      change_hints: ['relationships'],
    });
    expect(onNavigate).toHaveBeenCalledWith({
      record_id: recordId,
      correlation_id: correlationId,
    });
    expect(onNotify).toHaveBeenCalledWith({
      message: 'Saved',
      severity: 'success',
    });
    expect(onRefreshRecord).toHaveBeenCalledTimes(1);

    dispose();
    requestRecordRefresh(host, { record_id: contextId });
    expect(onRefreshRecord).toHaveBeenCalledTimes(1);
  });

  it('rejects invalid host details and emits context changes with the contract', () => {
    const component = new EventTarget();
    const received = vi.fn();
    component.addEventListener(attricatEventNames.contextChanged, (event) =>
      received((event as CustomEvent).detail),
    );

    expect(() =>
      dispatchRecordUpdated(component, {
        record_id: recordId,
        change_hints: ['outbox_payload' as never],
      }),
    ).toThrow();
    dispatchContextChanged(component, { context_id: contextId });

    expect(received).toHaveBeenCalledWith({ context_id: contextId });
  });
});
