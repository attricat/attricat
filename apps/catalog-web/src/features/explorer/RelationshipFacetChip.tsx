import type { Ref } from 'react';
import { useTranslation } from 'react-i18next';
import { RelationshipSelectorDialog } from '../../components/RelationshipSelectorDialog';
import { useRelationshipSelectionLabels } from '../records/components/useRelationshipSelectionLabels';
import { ExplorerFilterChip } from './ExplorerFilterChip';
import type {
  ExplorerRelationshipFacet,
  RelationshipFacetUpdate,
} from './relationshipFilterTypes';
import { RelationshipTargetPicker } from './RelationshipTargetPicker';

type Props = {
  chipRef: Ref<HTMLDivElement>;
  facet: ExplorerRelationshipFacet;
  /** Short field label shown on the chip. */
  fieldLabel: string;
  /** Field and target blueprint, which title the picker. */
  label: string;
  onClose: () => void;
  onOpen: () => void;
  onRemove: () => void;
  onUpdate: (field: string, updates: RelationshipFacetUpdate) => void;
  open: boolean;
};

/** A relationship filter chip that edits its selection in the picker dialog. */
export const RelationshipFacetChip = ({
  chipRef,
  facet,
  fieldLabel,
  label,
  onClose,
  onOpen,
  onRemove,
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
  const [firstId] = facet.selectedIds;
  const chipLabel =
    firstId === undefined
      ? undefined
      : t('explorer.relationshipFilterChip', {
          field: fieldLabel,
          values:
            facet.selectedIds.length > 1
              ? t('explorer.relationshipFilterValues', {
                  rest: facet.selectedIds.length - 1,
                  first: selectionLabels.get(firstId) ?? firstId,
                })
              : (selectionLabels.get(firstId) ?? firstId),
        });
  return (
    <>
      {chipLabel && (
        <ExplorerFilterChip
          chipRef={chipRef}
          label={chipLabel}
          onClick={onOpen}
          onDelete={onRemove}
          removeLabel={t('explorer.removeRelationshipFilter', {
            filter: chipLabel,
          })}
        />
      )}
      <RelationshipSelectorDialog
        closeLabel={t('records.closeRelationshipSelector')}
        onClose={onClose}
        open={open}
        selectedLabel={t('records.relationshipSelected', {
          count: facet.selectedIds.length,
        })}
        title={t('records.selectRelationships', { blueprint: label })}
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
