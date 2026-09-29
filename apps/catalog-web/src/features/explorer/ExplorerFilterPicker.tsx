import { Button, Chip, Stack, Typography } from '@mui/material';
import { PlusIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactOutlinedActionButtonSx } from '../../components/CompactOutlinedActionButton';
import type { Attribute } from '../entities/api';
import { isHiddenByDefault } from '../entities/attributeVisibility';
import { attributeLabel } from '../entities/entityDisplay';
import { AttributeFilterDialog } from './AttributeFilterDialog';
import {
  attributeFilterKey,
  attributeFilterLabel,
  isFilterableAttribute,
} from './attributeFilters';
import {
  attributeFilterInputValue,
  emptyAttributeFilterDraft,
  type AttributeFilterDraft,
} from './attributeFilterValues';
import { explorerVisibilityScope, maximumAttributeFilters } from './constants';
import type { RelationshipFilterAttribute } from './relationshipFilterTypes';
import type { AttributeFilter } from './search';
import { useRelationshipFilterPaths } from './useRelationshipFilterPaths';
import { smallIconSize } from '../../components/iconSizes';

type Props = {
  filters: AttributeFilter[];
  attributes: Attribute[];
  blueprintName: string;
  pathAttributes?: { code: string; value_type: Attribute['value_type'] }[];
  relationshipAttributes?: RelationshipFilterAttribute[];
  onAdd: (filter: AttributeFilter) => void;
  onAddRelationship: (attribute: RelationshipFilterAttribute) => void;
  onRemove: (index: number) => void;
  onUpdate: (index: number, filter: AttributeFilter) => void;
};

type EditorState = {
  // Remounts the dialog form for every opening so it starts from its draft.
  session: number;
  editingIndex: number | null;
  draft: AttributeFilterDraft;
};

export const ExplorerFilterPicker = ({
  filters,
  attributes,
  blueprintName,
  pathAttributes = [],
  relationshipAttributes = [],
  onAdd,
  onAddRelationship,
  onRemove,
  onUpdate,
}: Props) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [editor, setEditor] = useState<EditorState>({
    session: 0,
    editingIndex: null,
    draft: emptyAttributeFilterDraft,
  });
  const relationshipPaths = useRelationshipFilterPaths(
    relationshipAttributes,
    open && editor.editingIndex === null,
  );
  const filterableAttributes: Attribute[] = [
    ...new Map(
      [
        ...attributes.filter(
          (attribute) =>
            isFilterableAttribute(attribute) &&
            !isHiddenByDefault(attribute, explorerVisibilityScope),
        ),
        ...pathAttributes.map(({ code, value_type }) => ({ code, value_type })),
        ...relationshipAttributes,
        ...relationshipPaths.relationshipPaths,
      ].map((attribute) => [attribute.code, attribute]),
    ).values(),
  ];
  const maximumReached = filters.length >= maximumAttributeFilters;

  if (!filterableAttributes.length) {
    return (
      <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
        {t('explorer.noFilterableAttributes')}
      </Typography>
    );
  }

  const findAttribute = (code: string) =>
    filterableAttributes.find((item) => item.code === code);
  const openEditor = (
    editingIndex: number | null,
    draft: AttributeFilterDraft,
  ) => {
    setEditor((current) => ({
      session: current.session + 1,
      editingIndex,
      draft,
    }));
    setOpen(true);
  };
  const openFilter = (filter: AttributeFilter, index: number) =>
    openEditor(index, {
      field: filter.field,
      operator: filter.operator,
      value: attributeFilterInputValue(
        filter,
        findAttribute(filter.field)?.value_type,
      ),
    });
  const close = () => setOpen(false);
  const submit = (filter: AttributeFilter) => {
    if (editor.editingIndex === null) onAdd(filter);
    else onUpdate(editor.editingIndex, filter);
    close();
  };
  const selectRelationship = (attribute: RelationshipFilterAttribute) => {
    onAddRelationship(attribute);
    close();
  };

  return (
    <Stack spacing={1} sx={{ mt: 1 }}>
      <Stack
        direction="row"
        spacing={0.5}
        sx={{ alignItems: 'center', flexWrap: 'wrap' }}
        useFlexGap
      >
        {filters.map((filter, index) => (
          <Chip
            key={attributeFilterKey(filter, index)}
            label={attributeFilterLabel(
              t,
              filter,
              attributeLabel(
                findAttribute(filter.field) ?? { code: filter.field },
              ),
            )}
            onClick={() => openFilter(filter, index)}
            onDelete={() => onRemove(index)}
            size="small"
          />
        ))}
        <Button
          color="primary"
          onClick={() => openEditor(null, emptyAttributeFilterDraft)}
          size="small"
          startIcon={<PlusIcon size={smallIconSize} />}
          sx={compactOutlinedActionButtonSx}
          variant="outlined"
        >
          {t('explorer.addAttributeFilter')}
        </Button>
      </Stack>
      {maximumReached && (
        <Typography color="text.secondary" variant="body2">
          {t('explorer.maximumAttributeFilters', {
            count: maximumAttributeFilters,
          })}
        </Typography>
      )}
      <AttributeFilterDialog
        attributes={filterableAttributes}
        blueprintName={blueprintName}
        editing={editor.editingIndex !== null}
        initialDraft={editor.draft}
        key={editor.session}
        maximumReached={maximumReached}
        onClose={close}
        onSelectRelationship={selectRelationship}
        onSubmit={submit}
        open={open}
        relationshipPathsLoading={relationshipPaths.loading}
      />
    </Stack>
  );
};
