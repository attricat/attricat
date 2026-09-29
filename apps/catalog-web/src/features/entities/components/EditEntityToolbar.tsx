import ViewListOutlinedIcon from '@mui/icons-material/ViewListOutlined';
import VisibilityOutlinedIcon from '@mui/icons-material/VisibilityOutlined';
import { Button, Tooltip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RouterIconButton } from '../../../components/RouterLink';
import { EDIT_ENTITY_FORM_ID } from '../constants';
import { EntitySchemaStatus } from './EntitySchemaStatus';
import { EntityToolbar } from './EntityToolbar';

type Props = {
  blueprint?: { code: string; version: number };
  entityId: string;
  saving: boolean;
  schemaOutdated?: boolean;
};

/** Actions for the edit entity page, including submitting the edit form. */
export const EditEntityToolbar = ({
  blueprint,
  entityId,
  saving,
  schemaOutdated,
}: Props) => {
  const { t } = useTranslation();
  return (
    <EntityToolbar label={t('entities.editEntity')}>
      <Tooltip title={t('entities.viewPreview')}>
        <RouterIconButton
          aria-label={t('entities.viewPreview')}
          params={{ entityId }}
          to="/entities/$entityId"
        >
          <VisibilityOutlinedIcon />
        </RouterIconButton>
      </Tooltip>
      {blueprint && (
        <Tooltip title={t('entities.viewAll')}>
          <RouterIconButton
            aria-label={t('entities.viewAll')}
            search={{ blueprint: blueprint.code, version: blueprint.version }}
            to="/"
          >
            <ViewListOutlinedIcon />
          </RouterIconButton>
        </Tooltip>
      )}
      {schemaOutdated !== undefined && (
        <EntitySchemaStatus
          entityId={entityId}
          schemaOutdated={schemaOutdated}
        />
      )}
      {blueprint && (
        <Button
          disabled={saving}
          form={EDIT_ENTITY_FORM_ID}
          sx={{ ml: 'auto' }}
          type="submit"
          variant="contained"
        >
          {t('entities.saveChanges')}
        </Button>
      )}
    </EntityToolbar>
  );
};
