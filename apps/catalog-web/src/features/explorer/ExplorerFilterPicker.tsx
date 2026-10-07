import { Button, Chip, Stack, Typography } from '@mui/material';
import { PlusIcon } from 'lucide-react';
import { useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactOutlinedActionButtonSx } from '../../components/CompactOutlinedActionButton';
import type { Attribute } from '../entities/api';
import { isHiddenByDefault } from '../entities/attributeVisibility';
import { AttributeFilterDialog } from './AttributeFilterDialog';
import { principalConfiguration } from '../principals/principal';
import { usePrincipalDirectory } from '../principals/usePrincipalDirectory';
import {
  attributeFilterKey,
  attributeFilterLabel,
  isFilterableAttribute,
} from './attributeFilters';
import {
  attributeFilterInputValue,
  emptyAttributeFilterDraft,
  type AttributeFilterDraft,
  type AttributeFilterRequest,
} from './attributeFilterValues';
import { explorerVisibilityScope, maximumAttributeFilters } from './constants';
import type { RelationshipFilterAttribute } from './relationshipFilterTypes';
import type { AttributeFilter } from './search';
import { useRelationshipFilterPaths } from './useRelationshipFilterPaths';
import { smallIconSize } from '../../components/iconSizes';
import { useTimeZone } from '../../time/useInstantFormat';

type Props = {
  filters: AttributeFilter[];
  filterRequest?: AttributeFilterRequest;
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
  filterRequest,
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
  const timeZone = useTimeZone();
  const [open, setOpen] = useState(false);
  const [editor, setEditor] = useState<EditorState>({
    session: 0,
    editingIndex: null,
    draft: emptyAttributeFilterDraft,
  });
  // Open for each new external request while rendering, so the dialog starts
  // from the requested draft without an extra effect pass.
  const chipRefs = useRef<(HTMLDivElement | null)[]>([]);
  const addButtonRef = useRef<HTMLButtonElement>(null);
  const removedIndex = useRef<number | null>(null);
  // Deleting a chip removes the focused element. Move focus to the chip that
  // took its place, else the previous one, else Add filter, so keyboard focus
  // stays in the list (and inside a surrounding drawer or dialog).
  useLayoutEffect(() => {
    const index = removedIndex.current;
    if (index === null) return;
    removedIndex.current = null;
    const next = chipRefs.current[Math.min(index, filters.length - 1)];
    (next ?? addButtonRef.current)?.focus();
  }, [filters]);
  const remove = (index: number) => {
    removedIndex.current = index;
    onRemove(index);
  };
  const [handledRequest, setHandledRequest] = useState(filterRequest);
  if (filterRequest !== handledRequest) {
    setHandledRequest(filterRequest);
    if (filterRequest) {
      setEditor((current) => ({
        session: current.session + 1,
        editingIndex: null,
        draft: filterRequest.draft,
      }));
      setOpen(true);
    }
  }
  const relationshipPaths = useRelationshipFilterPaths(
    relationshipAttributes,
    open && editor.editingIndex === null,
  );
  const localAttributes = attributes.filter(
    (attribute) =>
      isFilterableAttribute(attribute) &&
      !isHiddenByDefault(attribute, explorerVisibilityScope),
  );
  const filterableAttributes: Attribute[] = [
    ...new Map(
      [
        ...localAttributes,
        // Table path entries carry only a code and type; never let one replace
        // a local attribute's name and schema (status and assignee pickers).
        ...pathAttributes
          .filter(
            (path) =>
              !localAttributes.some(
                (attribute) => attribute.code === path.code,
              ),
          )
          .map(({ code, value_type }) => ({ code, value_type })),
        ...relationshipAttributes,
        ...relationshipPaths.relationshipPaths,
      ].map((attribute) => [attribute.code, attribute]),
    ).values(),
  ];
  const maximumReached = filters.length >= maximumAttributeFilters;
  const directory = usePrincipalDirectory(
    attributes.some((attribute) => principalConfiguration(attribute)),
  );

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
        timeZone,
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
        spacing={2}
        sx={{ alignItems: 'center', flexWrap: 'wrap' }}
        useFlexGap
      >
        {filters.map((filter, index) => (
          <Chip
            key={attributeFilterKey(filter, index)}
            label={attributeFilterLabel(
              t,
              filter,
              findAttribute(filter.field),
              directory.data,
            )}
            onClick={() => openFilter(filter, index)}
            onDelete={() => remove(index)}
            ref={(element: HTMLDivElement | null) => {
              chipRefs.current[index] = element;
            }}
            size="small"
          />
        ))}
        <Button
          color="primary"
          onClick={() => openEditor(null, emptyAttributeFilterDraft)}
          ref={addButtonRef}
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
