import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ConversationMessageContent } from './ConversationMessageContent';

describe('ConversationMessageContent', () => {
  it('renders assistant responses as Markdown', () => {
    const markup = renderToStaticMarkup(
      <ConversationMessageContent
        content="Hello **catalog**"
        role="assistant"
      />,
    );

    expect(markup).toContain('<strong>catalog</strong>');
  });

  it('renders tool calls as compact summaries with collapsed JSON details', () => {
    const markup = renderToStaticMarkup(
      <ConversationMessageContent
        content={{
          tool_calls: [
            {
              id: 'call-1',
              function: {
                name: 'search_catalog',
                arguments: '{"query":"chair"}',
              },
            },
          ],
        }}
        role="assistant"
      />,
    );

    expect(markup).toContain('Tool call');
    expect(markup).toContain('search_catalog');
    expect(markup).toContain('Show call JSON');
    expect(markup).toContain('aria-expanded="false"');
  });
});
