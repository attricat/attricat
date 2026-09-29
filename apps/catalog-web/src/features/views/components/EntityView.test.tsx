// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import type { Attribute } from '../../entities/api';
import { EntityView } from './EntityView';

it('places an attribute panel next to its rendered field', () => {
  const attribute = {
    id: '44444444-4444-4444-8444-444444444444',
    code: 'title',
    name: 'Title',
    value_type: 'string',
  } as Attribute;
  render(
    <EntityView
      attributes={[attribute]}
      values={{ title: { value: 'Example' } }}
      renderAttributePanel={(field) => <div>Panel for {field.code}</div>}
    />,
  );
  expect(screen.getByText('Panel for title')).toBeTruthy();
  expect(screen.getByText('Example')).toBeTruthy();
});
