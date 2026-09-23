// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../i18n';
import { BlueprintVersionMetadata } from './BlueprintVersionMetadata';
import type { BlueprintWithAttributes } from './api';

const blueprint = {
  blueprint: {
    id: '123e4567-e89b-12d3-a456-426614174000',
    code: 'product',
    name: 'Product',
    version: 1,
    status: 'published',
    definition: '',
    views: {},
  },
  attributes: [],
  table_path_attributes: [],
} as unknown as BlueprintWithAttributes;

describe('BlueprintVersionMetadata', () => {
  it('gives each instance its own tab and panel IDs', () => {
    render(
      <>
        <BlueprintVersionMetadata blueprint={blueprint} />
        <BlueprintVersionMetadata blueprint={blueprint} />
      </>,
    );
    const tabs = screen.getAllByRole('tab');
    expect(new Set(tabs.map((tab) => tab.id)).size).toBe(tabs.length);
    const panels = screen.getAllByRole('tabpanel');
    expect(panels).toHaveLength(2);
    panels.forEach((panel) => {
      const tab = tabs.find(
        (candidate) => candidate.id === panel.getAttribute('aria-labelledby'),
      );
      expect(tab?.getAttribute('aria-controls')).toBe(panel.id);
    });
  });
});
