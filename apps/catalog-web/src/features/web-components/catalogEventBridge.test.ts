import { describe, expect, it, vi } from 'vitest';
import {
  catalogEventNames,
  dispatchContextChanged,
  dispatchEntityUpdated,
  requestEntityRefresh,
  requestNavigation,
  requestNotification,
} from './catalogEvents';
import { installCatalogEventBridge } from './catalogEventBridge';

const entityId = '123e4567-e89b-12d3-a456-426614174000';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const correlationId = '123e4567-e89b-12d3-a456-426614174002';

describe('catalog web-component event bridge', () => {
  it('sends only the client-safe entity update contract to a component', () => {
    const component = new EventTarget();
    const received = vi.fn();
    component.addEventListener(catalogEventNames.entityUpdated, (event) =>
      received((event as CustomEvent).detail),
    );

    dispatchEntityUpdated(component, {
      entity_id: entityId,
      change_hints: ['attribute_values'],
      correlation_id: correlationId,
    });

    expect(received).toHaveBeenCalledWith({
      entity_id: entityId,
      change_hints: ['attribute_values'],
      correlation_id: correlationId,
    });
  });

  it('validates component requests before invoking host behavior', () => {
    const host = new EventTarget();
    const onRefreshEntity = vi.fn();
    const onNavigate = vi.fn();
    const onNotify = vi.fn();
    const dispose = installCatalogEventBridge({
      host,
      onRefreshEntity,
      onNavigate,
      onNotify,
    });

    requestEntityRefresh(host, {
      entity_id: entityId,
      change_hints: ['relationships'],
    });
    requestNavigation(host, {
      entity_id: entityId,
      correlation_id: correlationId,
    });
    requestNotification(host, { message: 'Saved', severity: 'success' });
    host.dispatchEvent(
      new CustomEvent(catalogEventNames.refreshEntity, {
        detail: { entity_id: entityId, payload: { secret: true } },
      }),
    );

    expect(onRefreshEntity).toHaveBeenCalledWith({
      entity_id: entityId,
      change_hints: ['relationships'],
    });
    expect(onNavigate).toHaveBeenCalledWith({
      entity_id: entityId,
      correlation_id: correlationId,
    });
    expect(onNotify).toHaveBeenCalledWith({
      message: 'Saved',
      severity: 'success',
    });
    expect(onRefreshEntity).toHaveBeenCalledTimes(1);

    dispose();
    requestEntityRefresh(host, { entity_id: contextId });
    expect(onRefreshEntity).toHaveBeenCalledTimes(1);
  });

  it('rejects invalid host details and emits context changes with the contract', () => {
    const component = new EventTarget();
    const received = vi.fn();
    component.addEventListener(catalogEventNames.contextChanged, (event) =>
      received((event as CustomEvent).detail),
    );

    expect(() =>
      dispatchEntityUpdated(component, {
        entity_id: entityId,
        change_hints: ['outbox_payload' as never],
      }),
    ).toThrow();
    dispatchContextChanged(component, { context_id: contextId });

    expect(received).toHaveBeenCalledWith({ context_id: contextId });
  });
});
