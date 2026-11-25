import remarkGfm from 'remark-gfm';

/**
 * GitHub Flavored Markdown (tables, task lists, strikethrough, autolinks and
 * footnotes), so text written for GitHub renders the same here.
 */
export const githubMarkdownPlugins = [remarkGfm];

/** Styles for the GFM-only elements, using theme borders and spacing. */
export const githubMarkdownStyles = {
  '& table': {
    display: 'block',
    maxWidth: '100%',
    overflowX: 'auto',
    borderCollapse: 'collapse',
    my: 2,
  },
  '& th, & td': {
    border: 1,
    borderColor: 'divider',
    px: 1.5,
    py: 0.75,
    textAlign: 'left',
    verticalAlign: 'top',
  },
  '& th': { fontWeight: 'fontWeightBold', bgcolor: 'action.hover' },
  '& li.task-list-item': { listStyleType: 'none' },
  '& ul.contains-task-list': { pl: 2 },
  '& li.task-list-item input': { mr: 1 },
} as const;
