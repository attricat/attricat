import { useInfiniteQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  List,
  ListItem,
  ListItemText,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { getIncomingRelationships } from '../../entities/api';
import { displayLabel } from '../../entities/entity-display';
import { entityQueryKeys } from '../../entities/query-keys';
import type { ViewComponentDefinition } from './component-types';

export const IncomingRelationshipListDisplay = ({
  entityId,
  node,
}: {
  entityId: string;
  node: {
    type: 'incoming_relationship_list';
    label: string;
    relationships: { source_blueprint: string; field: string }[];
    page_size: number;
  };
}) => {
  const [open, setOpen] = useState(false);
  const results = useInfiniteQuery({
    queryKey: entityQueryKeys.incomingRelationships(
      entityId,
      node.relationships,
      node.page_size,
    ),
    queryFn: ({ pageParam }) =>
      getIncomingRelationships(
        entityId,
        node.relationships,
        node.page_size,
        pageParam,
      ),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
    enabled: open,
  });
  const items = results.data?.pages.flatMap((page) => page.items) ?? [];

  return (
    <>
      <Button onClick={() => setOpen(true)} variant="outlined">
        {node.label}
      </Button>
      <Dialog
        fullWidth
        maxWidth="sm"
        onClose={() => setOpen(false)}
        open={open}
      >
        <DialogTitle>{node.label}</DialogTitle>
        <DialogContent dividers>
          {results.isPending && (
            <Typography>Loading linked entities...</Typography>
          )}
          {results.isError && (
            <Typography color="error">{results.error.message}</Typography>
          )}
          {!results.isPending && !results.isError && !items.length && (
            <Typography color="text.secondary">No linked entities.</Typography>
          )}
          {items.length > 0 && (
            <List disablePadding>
              {items.map((item) => (
                <ListItem alignItems="flex-start" disableGutters key={item.id}>
                  <ListItemText
                    primary={
                      <Link
                        params={{ entityId: item.id }}
                        to="/entities/$entityId"
                      >
                        {displayLabel(item.display, item.id)}
                      </Link>
                    }
                    secondary={
                      <Box
                        sx={{
                          display: 'flex',
                          flexWrap: 'wrap',
                          gap: 0.5,
                          mt: 0.5,
                        }}
                      >
                        <Chip label={item.blueprint_code} size="small" />
                      </Box>
                    }
                  />
                </ListItem>
              ))}
            </List>
          )}
        </DialogContent>
        <DialogActions>
          {results.hasNextPage && (
            <Button
              disabled={results.isFetchingNextPage}
              onClick={() => void results.fetchNextPage()}
            >
              {results.isFetchingNextPage ? 'Loading...' : 'Load more'}
            </Button>
          )}
          <Button onClick={() => setOpen(false)}>Close</Button>
        </DialogActions>
      </Dialog>
    </>
  );
};

export const incomingRelationshipListDisplayComponent = {
  id: 'catalog.incoming_relationship_list_display',
  version: 1,
  capabilities: ['display'],
  placements: ['incoming_relationship_list'],
  value_types: [],
  allowed_props: [],
  incomingRelationshipRenderer: IncomingRelationshipListDisplay,
} satisfies ViewComponentDefinition;
