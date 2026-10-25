import { Box, Stack, Typography } from '@mui/material';
import {
  AsteriskIcon,
  CornerDownRightIcon,
  ListIcon,
  type LucideIcon,
} from 'lucide-react';
import { createElement } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { RecordIcon, RelationshipIcon } from '../../components/systemIcons';
import { attributeLabel } from '../records/recordDisplay';
import { valueTypeIcons, valueTypeLabelKey } from '../records/valueTypeIcons';
import type { QuerySuggestion } from './querySuggestions';

const suggestionIcon = (suggestion: QuerySuggestion): LucideIcon => {
  switch (suggestion.kind) {
    case 'attribute':
      return valueTypeIcons[suggestion.attribute.value_type];
    case 'relationship':
      return RelationshipIcon;
    case 'allFields':
      return ListIcon;
    case 'global':
      return AsteriskIcon;
    case 'ids':
      return RecordIcon;
    case 'value':
      return CornerDownRightIcon;
  }
};

/** One completion in the query input's suggestion list. */
export const QuerySuggestionOption = ({
  active,
  id,
  onHover,
  onSelect,
  suggestion,
}: {
  active: boolean;
  id: string;
  onHover: () => void;
  onSelect: () => void;
  suggestion: QuerySuggestion;
}) => {
  const { t } = useTranslation();
  const describe = (suggestion: QuerySuggestion) => {
    switch (suggestion.kind) {
      case 'attribute': {
        const valueSchema = suggestion.attribute.value_schema;
        const schemaDescription =
          typeof valueSchema === 'object'
            ? valueSchema?.description
            : undefined;
        return typeof schemaDescription === 'string' && schemaDescription.trim()
          ? schemaDescription
          : t('explorer.querySuggestions.attribute', {
              name: attributeLabel(suggestion.attribute),
            });
      }
      case 'relationship':
        return t('explorer.querySuggestions.relationship', {
          target: suggestion.target,
        });
      case 'allFields':
        return t('explorer.querySuggestions.allFields', {
          target: suggestion.target,
        });
      case 'global':
        return t('explorer.querySuggestions.global');
      case 'ids':
        return t('explorer.querySuggestions.ids');
      case 'value':
        return undefined;
    }
  };
  const typeLabel = (suggestion: QuerySuggestion) =>
    suggestion.kind === 'attribute'
      ? t(valueTypeLabelKey(suggestion.attribute.value_type))
      : suggestion.kind === 'relationship'
        ? t('explorer.querySuggestions.linksTo', { target: suggestion.target })
        : undefined;
  const description = describe(suggestion);
  const type = typeLabel(suggestion);
  return (
    <Box
      aria-selected={active}
      component="li"
      id={id}
      onClick={onSelect}
      onMouseMove={onHover}
      role="option"
      sx={{
        alignItems: 'flex-start',
        borderRadius: 1,
        cursor: 'pointer',
        display: 'flex',
        gap: 2,
        px: 2,
        py: 1.5,
        ...(active && {
          bgcolor: 'action.selected',
        }),
      }}
    >
      <Box
        sx={{
          color:
            suggestion.kind === 'global' ? 'secondary.main' : 'primary.main',
          display: 'flex',
          pt: 0.5,
        }}
      >
        {createElement(suggestionIcon(suggestion), {
          size: compactIconSize,
        })}
      </Box>
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Stack
          direction="row"
          spacing={2}
          sx={{
            alignItems: 'baseline',
            justifyContent: 'space-between',
          }}
        >
          <Typography
            component="span"
            sx={{ fontWeight: 600, overflowWrap: 'anywhere' }}
            variant="body2"
          >
            {suggestion.kind === 'allFields'
              ? t('explorer.querySuggestions.allFieldsTitle', {
                  field: suggestion.code,
                })
              : suggestion.code}
          </Typography>
          {type && (
            <Typography
              color="text.secondary"
              component="span"
              sx={{ flexShrink: 0 }}
              variant="caption"
            >
              {type}
            </Typography>
          )}
        </Stack>
        {description && (
          <Typography
            color="text.secondary"
            component="span"
            sx={{ display: 'block' }}
            variant="caption"
          >
            {description}
          </Typography>
        )}
      </Box>
    </Box>
  );
};
