// @vitest-environment jsdom
import { ThemeProvider } from '@mui/material/styles';
import { fireEvent, render, screen } from '@testing-library/react';
import { parse } from 'smol-toml';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import '../../i18n';
import { makeTheme } from '../../app/theme';
import { attributeSchema, blueprintSchema } from '../records/schemas';
import type { ComponentReference } from '../records/api';
import {
  headingEditableAttributes,
  unplacedEditableAttributes,
} from '../records/recordFormAttributes';
import { RecordView } from '../views/components/RecordView';
import { recordHeadingComponentId } from '../views/components/blocks/RecordHeadingDefinition';
import { resolveEditComponent } from '../views/components/registry';
import { blueprintTemplates } from './blueprintEditorUtils';

const recordTemplates = blueprintTemplates
  .map(({ definition }) => parse(definition))
  .filter((definition) => definition.kind === 'record')
  .map((definition) => ({
    code: definition.code,
    views: blueprintSchema.shape.views.parse(definition.views),
    attributes: z
      .array(
        attributeSchema.extend({
          value_schema: z
            .string()
            .transform((value) => JSON.parse(value))
            .pipe(attributeSchema.shape.value_schema)
            .optional(),
        }),
      )
      .parse(definition.attributes),
  }));

describe.each(['light', 'dark'] as const)(
  'blueprint templates in %s mode',
  (mode) => {
    it.each(recordTemplates)(
      '$code edits every attribute through its detail layout',
      ({ views, attributes }) => {
        const renderEditor = (
          attribute: (typeof attributes)[number],
          component?: ComponentReference | null,
        ) => (
          <div data-testid={attribute.code}>
            {resolveEditComponent(component)?.id ?? 'default'}
          </div>
        );
        render(
          <ThemeProvider theme={makeTheme(mode)}>
            <RecordView
              attributes={headingEditableAttributes(attributes, views.detail)}
              values={{}}
              renderEditor={renderEditor}
            />
            <RecordView
              view={views.detail}
              attributes={attributes}
              values={{}}
              renderEditor={renderEditor}
              skipComponentId={recordHeadingComponentId}
            />
            <RecordView
              attributes={unplacedEditableAttributes(attributes, views.detail)}
              values={{}}
              renderEditor={renderEditor}
            />
          </ThemeProvider>,
        );
        expect(views.edit).toBeUndefined();
        // Tabs render only their active panel, so visit each one.
        const seen = new Map<string, number>();
        const collect = () => {
          for (const attribute of attributes)
            if (screen.queryByTestId(attribute.code))
              seen.set(
                attribute.code,
                screen.getAllByTestId(attribute.code).length,
              );
        };
        collect();
        for (const tab of screen.queryAllByRole('tab')) {
          fireEvent.click(tab);
          collect();
        }
        for (const attribute of attributes) {
          expect(seen.get(attribute.code)).toBe(1);
        }
        fireEvent.click(screen.queryAllByRole('tab')[0] ?? document.body);
        const markdownField = attributes.some(({ code }) => code === 'notes')
          ? 'notes'
          : 'description';
        expect(screen.getByTestId(markdownField).textContent).toBe(
          'catalog.markdown_edit',
        );
      },
    );

    it('renders product controls and switches preview tabs', () => {
      const product = recordTemplates.find(({ code }) => code === 'product')!;
      render(
        <ThemeProvider theme={makeTheme(mode)}>
          <RecordView
            view={product.views.detail}
            attributes={product.attributes}
            values={{
              name: { value: 'Example product' },
              description: { value: '**Product notes**' },
              color: { value: '#123456' },
              website: { value: 'https://example.com/product' },
              status: { value: 'draft' },
            }}
          />
        </ThemeProvider>,
      );
      expect(screen.getByText('Product notes').tagName).toBe('STRONG');
      expect(
        screen
          .getByRole('link', {
            name: 'https://example.com/product (opens in a new tab)',
          })
          .getAttribute('href'),
      ).toBe('https://example.com/product');
      expect(screen.getByText('#123456')).toBeTruthy();
      expect(screen.getByText('Draft')).toBeTruthy();
      fireEvent.click(screen.getByRole('tab', { name: 'Images' }));
      expect(screen.getByRole('tabpanel').textContent).toContain('Main photo');
      expect(screen.getByRole('tabpanel').textContent).toContain('Gallery');
      expect(screen.queryByText('Product notes')).toBeNull();
    });
  },
);
