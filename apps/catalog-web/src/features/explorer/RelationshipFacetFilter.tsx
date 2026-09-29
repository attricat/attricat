import { Button, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { compactOutlinedActionButtonSx } from '../../components/CompactOutlinedActionButton';
import { RelationshipSelectorDialog } from '../../components/RelationshipSelectorDialog';
import {
  RelationshipIcon,
  RelationshipPickerIcon,
} from '../../components/systemIcons';
import { RelationshipSelectionPills } from '../entities/components/RelationshipSelectionPills';
import { useRelationshipSelectionLabels } from '../entities/components/useRelationshipSelectionLabels';
import { facetHeadingIconSize } from './constants';
import type {
  ExplorerRelationshipFacet,
  RelationshipFacetUpdate,
} from './relationshipFilterTypes';
import { RelationshipTargetPicker } from './RelationshipTargetPicker';

type Props = {
  facet: ExplorerRelationshipFacet;
  label: string;
  onClose: () => void;
  onOpen: () => void;
  onUpdate: (field: string, updates: RelationshipFacetUpdate) => void;
  open: boolean;
};

export const RelationshipFacetFilter = ({
  facet,
  label,
  onClose,
  onOpen,
  onUpdate,
  open,
}: Props) => {
  const { t } = useTranslation();
  const { code, target_blueprint_code: targetBlueprint } =
    facet.sourceRelationship;
  const selectionLabels = useRelationshipSelectionLabels(
    targetBlueprint,
    facet.selectedIds,
  );
  return (
    <>
      {facet.selectedIds.length > 0 && (
        <Stack spacing={0.5}>
          <Stack
            direction="row"
            spacing={0.5}
            sx={{ alignItems: 'center', color: 'primary.main' }}
          >
            <RelationshipIcon sx={{ fontSize: facetHeadingIconSize }} />
            <Typography
              color="inherit"
              component="h3"
              sx={{ fontWeight: 700, letterSpacing: '0.01em', lineHeight: 1.4 }}
              variant="subtitle2"
            >
              {label}
            </Typography>
          </Stack>
          <RelationshipSelectionPills
            action={
              <Button
                aria-label={label}
                color="primary"
                onClick={onOpen}
                size="small"
                startIcon={<RelationshipPickerIcon fontSize="small" />}
                sx={compactOutlinedActionButtonSx}
                variant="outlined"
              >
                {t('entities.openRelationshipSelector')}
              </Button>
            }
            ids={facet.selectedIds}
            labels={selectionLabels}
            onRemove={(id) =>
              onUpdate(code, {
                selectedIds: facet.selectedIds.filter(
                  (selectedId) => selectedId !== id,
                ),
              })
            }
          />
        </Stack>
      )}
      <RelationshipSelectorDialog
        closeLabel={t('entities.closeRelationshipSelector')}
        onClose={onClose}
        open={open}
        selectedLabel={t('entities.relationshipSelected', {
          count: facet.selectedIds.length,
        })}
        title={t('entities.selectRelationships', { blueprint: label })}
        topAction={{ label: t('explorer.done'), onClick: onClose }}
      >
        <RelationshipTargetPicker
          onSelectedIdsChange={(ids) =>
            onUpdate(code, { selectedIds: ids, targetBlueprint })
          }
          selectedIds={facet.selectedIds}
          targetBlueprint={targetBlueprint}
        />
      </RelationshipSelectorDialog>
    </>
  );
};
