import { Box, Chip, IconButton } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { EllipsisVerticalIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { EntityItem, EntityPublicationStatus } from '../entities/api';
import { contextDisplayLabel } from '../entities/entityDisplay';
import {
  emptyValuePlaceholder,
  entityPanelOpenerAttribute,
  publicationStatuses,
} from './constants';
import { compactIconSize } from '../../components/iconSizes';

export type ActionMenuPosition = { left: number; top: number };

export type OpenEntityPanel = (entityId: string, opener: HTMLElement) => void;

/**
 * Links to the entity page. With `onOpenPanel`, a plain click opens the
 * entity in a panel over the results instead; modified clicks still open the page.
 */
export const EntityDisplayCell = ({
  contextCodes,
  entity,
  onOpenPanel,
}: {
  contextCodes: readonly string[];
  entity: EntityItem;
  onOpenPanel?: OpenEntityPanel;
}) => {
  const { t } = useTranslation();
  return (
    <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
      <Link
        {...(onOpenPanel && { [entityPanelOpenerAttribute]: true })}
        onClick={(event) => {
          if (
            !onOpenPanel ||
            event.button !== 0 ||
            event.altKey ||
            event.ctrlKey ||
            event.metaKey ||
            event.shiftKey
          )
            return;
          event.preventDefault();
          onOpenPanel(entity.id, event.currentTarget);
        }}
        params={{ entityId: entity.id }}
        to="/entities/$entityId"
      >
        {contextDisplayLabel(entity.display, entity.id, contextCodes)}
      </Link>
      {entity.is_sample && (
        <Chip color="info" label={t('entities.sample')} size="small" />
      )}
    </Box>
  );
};

export const PublicationStatusCell = ({
  publication,
}: {
  publication: EntityPublicationStatus | undefined;
}) => {
  const { t } = useTranslation();
  if (!publication) return emptyValuePlaceholder;
  return (
    <Chip
      color={
        publication.status === publicationStatuses.published
          ? 'success'
          : 'default'
      }
      label={t(`entities.publication.${publication.status}`)}
      size="small"
    />
  );
};

export const SchemaVersionCell = ({ entity }: { entity: EntityItem }) => {
  const { t } = useTranslation();
  return (
    <Chip
      color={entity.schema_outdated ? 'warning' : 'success'}
      label={t('explorer.schemaVersionStatus', {
        status: entity.schema_outdated
          ? t('explorer.outdated')
          : t('explorer.current'),
        version: entity.blueprint_version,
      })}
      size="small"
    />
  );
};

export const EntityActionsCell = ({
  entityId,
  onOpenActions,
}: {
  entityId: string;
  onOpenActions: (entityId: string, position: ActionMenuPosition) => void;
}) => {
  const { t } = useTranslation();
  return (
    <IconButton
      aria-label={t('explorer.entityActionsFor', { entityId })}
      onClick={(event) => {
        const { left, top } = event.currentTarget.getBoundingClientRect();
        onOpenActions(entityId, { left, top });
      }}
      size="small"
    >
      <EllipsisVerticalIcon size={compactIconSize} />
    </IconButton>
  );
};
