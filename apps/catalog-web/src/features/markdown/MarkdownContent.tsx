import { Box } from '@mui/material';
import ReactMarkdown from 'react-markdown';
import { monoFontFamily } from '../../app/theme';

import { markdownUrlTransform } from './markdownUrls';

export const MarkdownContent = ({ value }: { value: string }) => (
  <Box
    sx={{
      minWidth: 0,
      overflowWrap: 'anywhere',
      typography: 'body2',
      '& > :first-child': { mt: 0 },
      '& > :last-child': { mb: 0 },
      '& a': { color: 'primary.main' },
      '& h2, & h3, & h4, & h5, & h6': {
        typography: 'subtitle1',
        fontWeight: 600,
      },
      '& pre': {
        overflowX: 'auto',
        p: 2,
        bgcolor: 'action.hover',
        borderRadius: 1,
      },
      '& code': { fontFamily: monoFontFamily },
      '& blockquote': {
        ml: 0,
        pl: 2,
        borderLeft: 1,
        borderColor: 'divider',
        color: 'text.secondary',
      },
    }}
  >
    <ReactMarkdown
      skipHtml
      urlTransform={markdownUrlTransform}
      components={{
        img: ({ alt }) => <span>{alt}</span>,
        h1: ({ children }) => <h2>{children}</h2>,
        h2: ({ children }) => <h3>{children}</h3>,
        h3: ({ children }) => <h4>{children}</h4>,
        h4: ({ children }) => <h5>{children}</h5>,
        h5: ({ children }) => <h6>{children}</h6>,
      }}
    >
      {value}
    </ReactMarkdown>
  </Box>
);
