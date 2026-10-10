import { Box, Button, Typography } from '@mui/material';
import { PlusIcon } from 'lucide-react';
import { useLayoutEffect, useRef, useState, type RefObject } from 'react';
import { useTranslation } from 'react-i18next';
import { compactOutlinedActionButtonSx } from '../../components/CompactOutlinedActionButton';
import type { Attribute, Blueprint } from '../records/api';
import { isHiddenByDefault } from '../records/attributeVisibility';
import { attributeLabel } from '../records/recordDisplay';
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
import { ExplorerFilterChip } from './ExplorerFilterChip';
import { RelationshipFacetChip } from './RelationshipFacetChip';
import type {
  ExplorerRelationshipFacet,
  RelationshipFacetUpdate,
  RelationshipFilterAttribute,
} from './relationshipFilterTypes';
import type { AttributeFilter } from './search';
import { useRelationshipFilterPaths } from './useRelationshipFilterPaths';
import { smallIconSize } from '../../components/iconSizes';
import { useTimeZone } from '../../time/useInstantFormat';
import { lexiconText } from '../lexicon/lexicon';

type Props = {
  filters: AttributeFilter[];
  filterRequest?: AttributeFilterRequest;
  attributes: Attribute[];
  blueprint: string;
  blueprints?: Blueprint[];
  /** Receives focus when the last chip is removed and nothing can be added. */
  emptyFocusTarget?: RefObject<HTMLElement | null>;
  facets?: ExplorerRelationshipFacet[];
  pathAttributes?: { code: string; value_type: Attribute['value_type'] }[];
  relationshipAttributes?: RelationshipFilterAttribute[];
  onAdd: (filter: AttributeFilter) => void;
  onRemove: (index: number) => void;
  onUpdate: (index: number, filter: AttributeFilter) => void;
  onUpdateFacet?: (field: string, updates: RelationshipFacetUpdate) => void;
};

type EditorState = {
  // Remounts the dialog form for every opening so it starts from its draft.
  session: number;
  editingIndex: number | null;
  draft: AttributeFilterDraft;
};

/** Keeps the facet being picked mounted even before it has selections. */
const withOpenFacet = (
  facets: ExplorerRelationshipFacet[],
  relationshipToOpen: RelationshipFilterAttribute | undefined,
) =>
  relationshipToOpen &&
  !facets.some(
    (facet) => facet.sourceRelationship.code === relationshipToOpen.code,
  )
    ? [...facets, { selectedIds: [], sourceRelationship: relationshipToOpen }]
    : facets;

/**
 * Explorer filters as one row of chips: each chip opens its editor and the
 * trailing button adds another, so the results keep the full page width.
 */
export const ExplorerFilterBar = ({
  filters,
  filterRequest,
  attributes,
  blueprint,
  blueprints = [],
  emptyFocusTarget,
  facets = [],
  pathAttributes = [],
  relationshipAttributes = [],
  onAdd,
  onRemove,
  onUpdate,
  onUpdateFacet = () => undefined,
}: Props) => {
  const { t } = useTranslation();
  const timeZone = useTimeZone();
  const [open, setOpen] = useState(false);
  const [relationshipToOpen, setRelationshipToOpen] =
    useState<RelationshipFilterAttribute>();
  const [editor, setEditor] = useState<EditorState>({
    session: 0,
    editingIndex: null,
    draft: emptyAttributeFilterDraft,
  });
  const selectedFacets = facets.filter((facet) => facet.selectedIds.length);
  const chipKeys = [
    ...filters.map((filter, index) => attributeFilterKey(filter, index)),
    ...selectedFacets.map((facet) => facet.sourceRelationship.code),
  ];
  const chipRefs = useRef(new Map<string, HTMLDivElement>());
  const chipRef = (key: string) => (element: HTMLDivElement | null) => {
    if (element) chipRefs.current.set(key, element);
    else chipRefs.current.delete(key);
  };
  const addButtonRef = useRef<HTMLButtonElement>(null);
  const removedPosition = useRef<number | null>(null);
  // Deleting a chip removes the focused element. Move focus to the chip that
  // took its place, else the previous one, else Add filter, so keyboard focus
  // stays in the row.
  useLayoutEffect(() => {
    const position = removedPosition.current;
    if (position === null) return;
    removedPosition.current = null;
    const key = chipKeys[Math.min(position, chipKeys.length - 1)];
    const next = key === undefined ? undefined : chipRefs.current.get(key);
    (next ?? addButtonRef.current ?? emptyFocusTarget?.current)?.focus();
  });
  const removeChip = (position: number, remove: () => void) => {
    removedPosition.current = position;
    remove();
  };
  // Open for each new external request while rendering, so the dialog starts
  // from the requested draft without an extra effect pass.
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
  const blueprintName = (code: string) => {
    const name = blueprints.find((item) => item.code === code)?.name;
    return name === undefined ? code : lexiconText(name);
  };

  const findAttribute = (code: string) =>
    filterableAttributes.find((item) => item.code === code) ??
    attributes.find((item) => item.code === code);
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
    setRelationshipToOpen(attribute);
    close();
  };

  return (
    <Box
      aria-label={t('explorer.filters')}
      role="group"
      sx={{
        alignItems: 'center',
        display: 'flex',
        flexWrap: 'wrap',
        gap: 1,
        minWidth: 0,
        mt: 2.5,
      }}
    >
      {filters.map((filter, index) => {
        const key = attributeFilterKey(filter, index);
        const label = attributeFilterLabel(
          t,
          filter,
          findAttribute(filter.field),
          directory.data,
        );
        return (
          <ExplorerFilterChip
            chipRef={chipRef(key)}
            key={key}
            label={label}
            onClick={() => openFilter(filter, index)}
            onDelete={() => removeChip(index, () => onRemove(index))}
            removeLabel={t('explorer.removeAttributeFilter', { filter: label })}
          />
        );
      })}
      {withOpenFacet(facets, relationshipToOpen).map((facet) => {
        const { code } = facet.sourceRelationship;
        const fieldLabel = attributeLabel(facet.sourceRelationship);
        return (
          <RelationshipFacetChip
            chipRef={chipRef(code)}
            facet={facet}
            fieldLabel={fieldLabel}
            key={code}
            label={t('explorer.relationshipFacetLabel', {
              attribute: fieldLabel,
              blueprint: blueprintName(
                facet.sourceRelationship.target_blueprint_code,
              ),
            })}
            onClose={() => setRelationshipToOpen(undefined)}
            onOpen={() => setRelationshipToOpen(facet.sourceRelationship)}
            onRemove={() =>
              removeChip(chipKeys.indexOf(code), () =>
                onUpdateFacet(code, { selectedIds: [] }),
              )
            }
            onUpdate={onUpdateFacet}
            open={relationshipToOpen?.code === code}
          />
        );
      })}
      {filterableAttributes.length > 0 && (
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
      )}
      {maximumReached && (
        <Typography color="text.secondary" variant="body2">
          {t('explorer.maximumAttributeFilters', {
            count: maximumAttributeFilters,
          })}
        </Typography>
      )}
      <AttributeFilterDialog
        attributes={filterableAttributes}
        blueprintName={blueprintName(blueprint)}
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
    </Box>
  );
};
