import { Menu, MenuItem } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { EntityItem, EntityPublicationStatus } from '../entities/api';
import { ExtensionPopoverOutlet } from '../extensions/ExtensionOutlet';

type Props = {
  blueprintId: string;
  canPublish: boolean;
  canDelete: boolean;
  entity: EntityItem;
  onClose: () => void;
  onDelete: () => void;
  onSearchInfo: (entity: EntityItem) => void;
  position: { left: number; top: number } | null;
  publication: EntityPublicationStatus | undefined;
  publicationContextId: string | undefined;
  publish: () => void;
  publishing: boolean;
  duplicate: () => void;
  duplicating: boolean;
  unpublish: () => void;
  unpublishing: boolean;
};

export const EntityActionsMenu = ({
  blueprintId,
  canPublish,
  canDelete,
  entity,
  onClose,
  onDelete,
  onSearchInfo,
  position,
  publication,
  publicationContextId,
  publish,
  publishing,
  duplicate,
  duplicating,
  unpublish,
  unpublishing,
}: Props) => {
  const { t } = useTranslation();

  return (
    <Menu
      anchorPosition={position ?? undefined}
      anchorReference="anchorPosition"
      onClose={onClose}
      open={Boolean(position)}
      transformOrigin={{ horizontal: 'right', vertical: 'top' }}
    >
      <MenuItem
        onClick={() => {
          onClose();
          onSearchInfo(entity);
        }}
      >
        {t('explorer.searchInfo')}
      </MenuItem>
      <MenuItem
        disabled={duplicating}
        onClick={() => {
          onClose();
          duplicate();
        }}
      >
        {t('entities.duplicateEntity')}
      </MenuItem>
      {canDelete && (
        <MenuItem
          onClick={() => {
            onClose();
            onDelete();
          }}
        >
          {t('entities.deleteEntity')}
        </MenuItem>
      )}
      {canPublish && publicationContextId && publication && (
        <>
          <MenuItem
            disabled={publishing}
            onClick={() => {
              onClose();
              publish();
            }}
          >
            {t('entities.publish')}
          </MenuItem>
          {publication.status !== 'not_published' && (
            <MenuItem
              disabled={unpublishing}
              onClick={() => {
                onClose();
                unpublish();
              }}
            >
              {t('entities.unpublish')}
            </MenuItem>
          )}
        </>
      )}
      <ExtensionPopoverOutlet
        context={{
          context_version: 1,
          blueprint_id: blueprintId,
          blueprint_version: entity.blueprint_version,
          entity_id: entity.id,
        }}
        label={t('explorer.extensionActions')}
        outlet="explorer_row_action"
      />
    </Menu>
  );
};
