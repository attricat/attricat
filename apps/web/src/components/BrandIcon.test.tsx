// @vitest-environment jsdom
import { ThemeProvider } from '@mui/material';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { makeTheme } from '../app/theme';
import { BrandIcon } from './BrandIcon';
import wordmarkLight from '../../design/assets/logos/wordmark-light.svg?url';
import wordmarkDark from '../../design/assets/logos/wordmark-dark.svg?url';

describe('BrandIcon', () => {
  it.each(['light', 'dark'] as const)('uses the %s wordmark', (mode) => {
    render(
      <ThemeProvider theme={makeTheme(mode)}>
        <div aria-label="Attricat">
          <BrandIcon variant="wordmark" />
        </div>
      </ThemeProvider>,
    );

    expect(
      screen
        .getByLabelText('Attricat')
        .querySelector('img')
        ?.getAttribute('src'),
    ).toBe(mode === 'light' ? wordmarkLight : wordmarkDark);
  });
});
