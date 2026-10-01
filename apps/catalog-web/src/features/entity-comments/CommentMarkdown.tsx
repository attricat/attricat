import { Box, Link } from '@mui/material';
import ReactMarkdown from 'react-markdown';

import { safeCommentUrl } from './markdown';

// Do not load remote images (tracking pixels), HTML, or executable URL schemes.

export const CommentMarkdown = ({ body }: { body: string }) => (
  <Box
    sx={{
      overflowWrap: 'anywhere',
      '& > :first-of-type': { mt: 0 },
      '& > :last-child': { mb: 0 },
      '& pre': { bgcolor: 'action.hover', p: 1, overflow: 'auto' },
      '& code': { fontFamily: 'monospace' },
      '& blockquote': { borderLeft: 1, borderColor: 'divider', pl: 2, ml: 0 },
      '& h1, & h2, & h3, & h4, & h5, & h6': {
        typography: 'subtitle1',
        fontWeight: 600,
      },
    }}
  >
    <ReactMarkdown
      skipHtml
      urlTransform={safeCommentUrl}
      components={{
        a: ({ href, children }) =>
          href ? (
            <Link href={href} rel="nofollow noreferrer noopener">
              {children}
            </Link>
          ) : (
            <>{children}</>
          ),
        img: ({ alt }) => <span>{alt}</span>,
      }}
    >
      {body}
    </ReactMarkdown>
  </Box>
);
