// @vitest-environment jsdom
import { fireEvent, render, screen, cleanup } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import '../../i18n';
import { MarkdownContent } from './MarkdownContent';
import { markdownUrlTransform } from './markdownUrls';
import { MarkdownEditor } from './MarkdownEditor';

afterEach(cleanup);

describe('Markdown', () => {
  it('renders semantic content without HTML or remote images', () => {
    const { container } = render(
      <MarkdownContent
        value={
          '# Heading\n\n**Strong**\n\n- Item\n\n<script>alert(1)</script>\n\n![Description](https://example.com/a.png)'
        }
      />,
    );
    expect(screen.getByRole('heading', { level: 2 }).textContent).toBe(
      'Heading',
    );
    expect(container.querySelector('strong')?.textContent).toBe('Strong');
    expect(screen.getByRole('listitem').textContent).toBe('Item');
    expect(container.querySelector('script, img')).toBeNull();
    expect(screen.getByText('Description')).toBeTruthy();
  });

  it('rejects unsafe URLs and preserves safe links', () => {
    for (const url of [
      'javascript:alert(1)',
      'JaVaScRiPt:alert(1)',
      'data:text/html,test',
      '//evil.example',
      'java\nscript:alert(1)',
      '\\evil.example',
    ]) {
      expect(markdownUrlTransform(url)).toBe('');
    }
    for (const url of [
      'https://example.com',
      'http://example.com',
      '/entities',
      '#section',
    ]) {
      expect(markdownUrlTransform(url)).toBe(url);
    }
    render(
      <MarkdownContent
        value={'[bad](javascript:alert) [good](https://example.com)'}
      />,
    );
    expect(screen.getByText('bad').getAttribute('href')).toBe('');
    expect(
      screen.getByRole('link', { name: 'good' }).getAttribute('href'),
    ).toBe('https://example.com');
  });

  it('previews the source and keeps the disabled input unchanged', () => {
    let changed = false;
    render(
      <MarkdownEditor
        label="Description"
        value="**Hello**"
        disabled
        onChange={() => {
          changed = true;
        }}
      />,
    );
    const input = screen.getByRole('textbox') as HTMLTextAreaElement;
    expect(input.disabled).toBe(true);
    fireEvent.change(input, { target: { value: 'Changed' } });
    expect(changed).toBe(false);
    fireEvent.click(screen.getByRole('tab', { name: 'Preview' }));
    expect(screen.getByText('Hello').tagName).toBe('STRONG');
    fireEvent.click(screen.getByRole('tab', { name: 'Write' }));
    expect(input.value).toBe('**Hello**');
  });
});
