import {
  Box,
  Chip,
  IconButton,
  Popover,
  Stack,
  Tooltip,
  Typography,
} from '@mui/material';
import { CircleAlertIcon, CopyIcon, InfoIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../../app/theme';
import { compactIconSize, smallIconSize } from '../../../components/iconSizes';
import { useCopyToClipboard } from '../../../components/useCopyToClipboard';
import { findingMessage } from '../../rules/findingMessage';
import { findingSeverityColor } from '../../rules/findingSeverity';
import type { Attribute } from '../api';
import {
  attributeCardinalities,
  attributeContextEditability,
  attributeContextFallbacks,
} from '../constants';
import {
  useFieldDiagnostics,
  useFieldFindings,
  worstSeverity,
} from '../fieldDiagnostics';
import { attributeLabel } from '../recordDisplay';
import { attributeValueTypes } from '../valueTypes';
import { useRevealField } from '../../views/fieldReveal';
import { FieldConnectionList } from './FieldConnectionList';
import { ValueTypeLabel } from './ValueTypeLabel';
import type { ResolvedValue } from '../../views/components/ValueField';

const isEmptyValue = (value: unknown) =>
  value === undefined ||
  value === null ||
  value === '' ||
  (Array.isArray(value) && value.length === 0);

const DiagnosticSection = ({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) => (
  <Box component="section">
    <Typography
      color="text.secondary"
      component="h3"
      sx={{ fontWeight: 700, mb: 0.5 }}
      variant="caption"
    >
      {title}
    </Typography>
    {children}
  </Box>
);

/**
 * A field's code, value type, value source and data quality findings, from
 * a small button beside the field. Renders nothing outside the record page.
 */
export const FieldDiagnosticsButton = ({
  attribute,
  resolved,
}: {
  attribute: Attribute;
  resolved?: ResolvedValue;
}) => {
  const { t } = useTranslation();
  const diagnostics = useFieldDiagnostics();
  const findings = useFieldFindings(attribute.code);
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  // Set when the popover closes to lead to another field, which keeps focus.
  const [revealing, setRevealing] = useState(false);
  const copyText = useCopyToClipboard();
  const revealField = useRevealField();
  if (!diagnostics) return null;
  const { contextCode } = diagnostics;
  const label = attributeLabel(attribute);
  const connections =
    diagnostics.connections?.filter((connection) =>
      connection.attributeCodes.includes(attribute.code),
    ) ?? [];
  const severity = findings && worstSeverity(findings);
  const Icon = severity ? CircleAlertIcon : InfoIcon;
  const sourceContext = resolved?.source_context;
  const fallsBack =
    attribute.context_fallback !== attributeContextFallbacks.none;
  const editableEverywhere =
    attribute.context_editable !== attributeContextEditability.default;
  return (
    <>
      <Tooltip title={t('records.fieldDiagnostics.tooltip')}>
        <IconButton
          aria-label={
            findings?.length
              ? t('records.fieldDiagnostics.openWithFindings', {
                  count: findings.length,
                  field: label,
                })
              : t('records.fieldDiagnostics.open', { field: label })
          }
          onClick={(event) => {
            setRevealing(false);
            setAnchor(event.currentTarget);
          }}
          size="small"
          sx={{
            color: severity
              ? `${findingSeverityColor(severity)}.main`
              : 'text.secondary',
          }}
        >
          <Icon size={compactIconSize} />
        </IconButton>
      </Tooltip>
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        disableRestoreFocus={revealing}
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
        slotProps={{ paper: { sx: { maxWidth: 360, p: 2 } } }}
      >
        <Stack spacing={2}>
          <Typography component="h2" sx={{ fontWeight: 700 }} variant="body2">
            {label}
          </Typography>
          <DiagnosticSection title={t('records.fieldDiagnostics.code')}>
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              <Typography
                component="code"
                sx={{ fontFamily: monoFontFamily, overflowWrap: 'anywhere' }}
                variant="body2"
              >
                {attribute.code}
              </Typography>
              <Tooltip title={t('records.fieldDiagnostics.copyCode')}>
                <IconButton
                  aria-label={t('records.fieldDiagnostics.copyCode')}
                  onClick={() => void copyText(attribute.code)}
                  size="small"
                >
                  <CopyIcon size={smallIconSize} />
                </IconButton>
              </Tooltip>
            </Stack>
          </DiagnosticSection>
          <DiagnosticSection title={t('records.fieldDiagnostics.valueType')}>
            <Typography component="div" variant="body2">
              <ValueTypeLabel valueType={attribute.value_type} />
            </Typography>
            {attribute.value_type === attributeValueTypes.relationship &&
              attribute.cardinality && (
                <Typography color="text.secondary" variant="body2">
                  {attribute.cardinality === attributeCardinalities.many
                    ? t('records.fieldDiagnostics.linksToMany')
                    : t('records.fieldDiagnostics.linksToOne')}
                </Typography>
              )}
          </DiagnosticSection>
          <DiagnosticSection title={t('records.fieldDiagnostics.valueSource')}>
            {diagnostics.showsValues && (
              <Typography variant="body2">
                {isEmptyValue(resolved?.value)
                  ? t('records.fieldDiagnostics.empty')
                  : !sourceContext
                    ? t('records.fieldDiagnostics.set')
                    : sourceContext.code === contextCode
                      ? t('records.fieldDiagnostics.setInContext', {
                          context: sourceContext.code,
                        })
                      : t('records.fieldDiagnostics.inheritedFromContext', {
                          context: sourceContext.code,
                        })}
              </Typography>
            )}
            <Typography color="text.secondary" variant="body2">
              {fallsBack
                ? t('records.fieldDiagnostics.fallsBack')
                : t('records.fieldDiagnostics.noFallback')}{' '}
              {editableEverywhere
                ? t('records.fieldDiagnostics.editableEverywhere')
                : t('records.fieldDiagnostics.editableInDefault')}
            </Typography>
          </DiagnosticSection>
          {connections.length > 0 && (
            <DiagnosticSection
              title={t('records.fieldDiagnostics.connections')}
            >
              <FieldConnectionList
                attributeCode={attribute.code}
                attributes={diagnostics.attributes ?? []}
                connections={connections}
                onReveal={
                  revealField
                    ? (code) => {
                        setRevealing(true);
                        setAnchor(null);
                        revealField(code);
                      }
                    : undefined
                }
              />
            </DiagnosticSection>
          )}
          {findings && (
            <DiagnosticSection title={t('records.fieldDiagnostics.findings')}>
              {findings.length === 0 ? (
                <Typography color="text.secondary" variant="body2">
                  {t('records.fieldDiagnostics.noFindings')}
                </Typography>
              ) : (
                <Stack component="ul" spacing={1} sx={{ m: 0, p: 0 }}>
                  {findings.map((finding) => (
                    <Stack
                      component="li"
                      direction="row"
                      key={finding.id}
                      spacing={1}
                      sx={{ alignItems: 'flex-start', listStyle: 'none' }}
                    >
                      <Chip
                        color={findingSeverityColor(finding.severity)}
                        label={t(`rules.severities.${finding.severity}`, {
                          defaultValue: finding.severity,
                        })}
                        size="small"
                      />
                      <Typography variant="body2">
                        {findingMessage(finding.message, attribute.code, [
                          attribute,
                        ])}
                      </Typography>
                    </Stack>
                  ))}
                </Stack>
              )}
            </DiagnosticSection>
          )}
        </Stack>
      </Popover>
    </>
  );
};
