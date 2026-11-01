// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../i18n';
import { mainContentId, SkipToContentLink } from './SkipToContentLink';

describe('SkipToContentLink', () => {
  it('moves focus to the main content', () => {
    render(
      <>
        <SkipToContentLink />
        <main id={mainContentId} tabIndex={-1} />
      </>,
    );
    fireEvent.click(screen.getByRole('link', { name: 'Skip to main content' }));
    expect(document.activeElement?.id).toBe(mainContentId);
  });
});
