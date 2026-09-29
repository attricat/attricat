// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import type { Attribute } from '../../entities/api';
import { EntityView } from './EntityView';

it('places a file panel next to each rendered file without passing file bytes', () => {
  const attribute = {
    id: '44444444-4444-4444-8444-444444444444',
    code: 'attachments',
    name: 'Attachments',
    value_type: 'file',
  } as Attribute;
  const fileId = '55555555-5555-4555-8555-555555555555';
  render(
    <EntityView
      attributes={[attribute]}
      values={{
        attachments: { value: [{ id: fileId, filename: 'report.pdf' }] },
      }}
      renderFilePanel={(field, id) => (
        <div>
          Panel for {field.code}: {id}
        </div>
      )}
    />,
  );
  expect(screen.getByText('report.pdf')).toBeTruthy();
  expect(screen.getByText(`Panel for attachments: ${fileId}`)).toBeTruthy();
});

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
