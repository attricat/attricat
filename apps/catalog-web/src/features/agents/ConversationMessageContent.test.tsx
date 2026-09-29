import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ConversationMessageContent } from './ConversationMessageContent';

describe('ConversationMessageContent', () => {
  it('renders assistant responses as Markdown', () => {
    const markup = renderToStaticMarkup(
      <ConversationMessageContent
        content="Hello **catalog**"
        messageRole="assistant"
      />,
    );

    expect(markup).toContain('<strong>catalog</strong>');
  });

  it('opens entity preview links in a new tab in assistant replies and tool results', () => {
    const id = '123e4567-e89b-12d3-a456-426614174000';
    const url = `/entities/${id}`;
    const reply = renderToStaticMarkup(
      <ConversationMessageContent
        content={`See [this item](${url})`}
        messageRole="assistant"
      />,
    );
    expect(reply).toContain(`href="${url}"`);
    expect(reply).toContain('target="_blank"');
    expect(reply).toContain('rel="noopener noreferrer"');

    const tool = renderToStaticMarkup(
      <ConversationMessageContent
        content={{
          tool_call_id: 'call-1',
          name: 'get_entity_preview_link',
          result: { entity_id: id, url },
        }}
        messageRole="tool"
      />,
    );
    expect(tool).toContain(`href="${url}"`);
    expect(tool).toContain('target="_blank"');
  });

  it('links saved searches to Explorer in a new tab', () => {
    const id = '123e4567-e89b-12d3-a456-426614174000';
    const markup = renderToStaticMarkup(
      <ConversationMessageContent
        content={{
          tool_call_id: 'call-2',
          name: 'create_saved_search',
          result: { id, name: 'Spring products', url: `/?savedView=${id}` },
        }}
        messageRole="tool"
      />,
    );
    expect(markup).toContain(`href="/?savedView=${id}"`);
    expect(markup).toContain('Spring products');
    expect(markup).toContain('target="_blank"');
  });

  it('does not turn untrusted tool result URLs into links', () => {
    const markup = renderToStaticMarkup(
      <ConversationMessageContent
        content={{
          tool_call_id: 'call-1',
          name: 'get_entity_preview_link',
          result: { url: 'https://example.com' },
        }}
        messageRole="tool"
      />,
    );
    expect(markup).not.toContain('href="https://example.com"');
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
        messageRole="assistant"
      />,
    );

    expect(markup).toContain('Tool call');
    expect(markup).toContain('search_catalog');
    expect(markup).toContain('Show call JSON');
    expect(markup).toContain('aria-expanded="false"');
  });
});
