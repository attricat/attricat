// @vitest-environment jsdom
import { ThemeProvider } from '@mui/material/styles';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { compactIconButtonSize } from '../../../components/iconSizes';
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

  it('keeps a decoration in a fixed slot beside the label', () => {
    render(
      <ThemeProvider theme={theme}>
        <ValueField
          attribute={{ code: 'title', name: 'title', value_type: 'string' }}
          renderAttributeDecoration={() => <span>decoration</span>}
          resolved={{ value: 'Passat' }}
        />
      </ThemeProvider>,
    );

    const slot = screen.getByText('decoration').parentElement!;
    expect(getComputedStyle(slot).height).toBe(`${compactIconButtonSize}px`);
    expect(slot.parentElement!.textContent).toContain('title');
  });

  it('keeps a decoration beside an editor, not in a row below it', () => {
    render(
      <ThemeProvider theme={theme}>
        <ValueField
          attribute={{ code: 'title', name: 'title', value_type: 'string' }}
          renderAttributeDecoration={() => <span>decoration</span>}
          renderEditor={() => <input aria-label="title" />}
        />
      </ThemeProvider>,
    );

    const slot = screen.getByText('decoration').parentElement!;
    expect(getComputedStyle(slot).width).toBe(`${compactIconButtonSize}px`);
    // The editor and the gutter holding the slot share one row.
    const row = slot.parentElement!.parentElement!;
    expect(row.contains(screen.getByLabelText('title'))).toBe(true);
    expect(getComputedStyle(row).display).toBe('flex');
  });
});
