// @vitest-environment jsdom
import { ThemeProvider } from '@mui/material/styles';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { makeTheme } from '../../../app/theme';
import '../../../i18n';
import { FieldDiagnosticsContext } from '../../records/fieldDiagnostics';
import type { Finding } from '../../rules/schemas';
import { ValueField } from './ValueField';

const theme = makeTheme('light');
const polishContextId = crypto.randomUUID();
const finding: Finding = {
  id: crypto.randomUUID(),
  rule_id: crypto.randomUUID(),
  rule_version: 1,
  record_id: crypto.randomUUID(),
  context_id: polishContextId,
  severity: 'error',
  message: "'title' is required",
  evidence: { attribute_code: 'title' },
  state: 'open',
  acknowledged_at: null,
  resolved_at: null,
  created_at: '2026-10-20T13:54:17Z',
  updated_at: '2026-10-20T13:54:17Z',
};

const renderField = (code: string) =>
  render(
    <ThemeProvider theme={theme}>
      <FieldDiagnosticsContext
        value={{
          contextCode: 'cars_pl',
          contextId: polishContextId,
          findings: [finding],
        }}
      >
        <ValueField
          attribute={{ code, name: code, value_type: 'string' }}
          resolved={{ value: 'Passat' }}
        />
      </FieldDiagnosticsContext>
    </ThemeProvider>,
  );

// The browser's own spelling of a CSS color, to compare computed styles.
const cssColor = (color: string) => {
  const element = document.createElement('div');
  element.style.backgroundColor = color;
  return element.style.backgroundColor;
};

// The label sits in the field's own stack, which carries the tint.
const fieldOf = (label: string) =>
  screen.getByText(label).closest<HTMLElement>('.MuiStack-root')!;

describe('ValueField', () => {
  it('tints a field with data quality findings', () => {
    renderField('title');

    expect(getComputedStyle(fieldOf('title')).backgroundColor).toBe(
      cssColor(theme.palette.error.tint!),
    );
  });

  it('leaves a field without findings unmarked', () => {
    renderField('description');

    expect(getComputedStyle(fieldOf('description')).backgroundColor).not.toBe(
      cssColor(theme.palette.error.tint!),
    );
  });
});
