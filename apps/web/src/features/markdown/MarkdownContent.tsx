import { Box, Link } from '@mui/material';
import ReactMarkdown from 'react-markdown';
import { monoFontFamily } from '../../app/theme';
import { UserProfileLink } from '../../components/UserProfileLink';
import { markdownUrlTransform } from './markdownUrls';
import { mentionedUserId } from './mentions';

/** Keeps mentions' user references and passes other URLs through the filter. */
const urlTransform = (url: string) =>
  mentionedUserId(url) ? url : markdownUrlTransform(url);

/**
 * Attricat-authored Markdown (field values and comments). Raw HTML is skipped,
 * images show their alt text instead of loading remote resources, links
 * pass through `markdownUrlTransform`, and mentions (`[@Ada](user:<uuid>)`)
 * link to the user's profile.
 */
export const MarkdownContent = ({ value }: { value: string }) => (
  <Box
    sx={{
      minWidth: 0,
      overflowWrap: 'anywhere',
      typography: 'body2',
      '& > :first-of-type': { mt: 0 },
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
      urlTransform={urlTransform}
      components={{
        a: ({ href, children }) => {
          const userId = mentionedUserId(href);
          if (userId)
            return (
              <Box
                component="span"
                sx={{ color: 'primary.main', fontWeight: 600 }}
              >
                <UserProfileLink userId={userId}>{children}</UserProfileLink>
              </Box>
            );
          return href ? (
            <Link href={href} rel="nofollow noreferrer noopener">
              {children}
            </Link>
          ) : (
            <>{children}</>
          );
        },
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
