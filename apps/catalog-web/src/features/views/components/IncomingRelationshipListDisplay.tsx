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
import { useTranslation } from 'react-i18next';
import { LoadMoreButton } from '../../../components/LoadMoreButton';
import { getIncomingRelationships } from '../../entities/api';
import { displayLabel } from '../../entities/entityDisplay';
import { entityQueryKeys } from '../../entities/queryKeys';
import type { ViewComponentDefinition } from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';

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
  const { t } = useTranslation();
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
            <Typography>{t('views.loadingLinkedEntities')}</Typography>
          )}
          {results.isError && (
            <Typography color="error">{results.error.message}</Typography>
          )}
          {!results.isPending && !results.isError && !items.length && (
            <Typography color="text.secondary">
              {t('views.noLinkedEntities')}
            </Typography>
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
            <LoadMoreButton
              isLoading={results.isFetchingNextPage}
              onLoadMore={() => void results.fetchNextPage()}
            />
          )}
          <Button onClick={() => setOpen(false)}>{t('views.close')}</Button>
        </DialogActions>
      </Dialog>
    </>
  );
};

export const incomingRelationshipListDisplayComponent = {
  id: VIEW_COMPONENT_IDS.incomingRelationshipListDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['incoming_relationship_list'],
  value_types: [],
  allowed_props: [],
  incomingRelationshipRenderer: IncomingRelationshipListDisplay,
} satisfies ViewComponentDefinition;
