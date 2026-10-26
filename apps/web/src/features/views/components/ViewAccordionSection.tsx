import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Typography,
} from '@mui/material';
import { ChevronDownIcon, CircleAlertIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { ViewNode } from '../../records/api';
import { lexiconText } from '../../lexicon/lexicon';
import { compactIconSize } from '../../../components/iconSizes';
import { useFieldRevealRequest } from '../fieldReveal';
import { nodesPlacedFields } from '../viewFieldComponents';
import { useFieldDiagnostics } from '../../records/fieldDiagnostics';
import { layoutMarker, layoutMarkerDescriptions } from '../layoutMarker';

/**
 * One accordion section of a view. It opens for a field the user asks to see
 * and, like a tab, is marked while it holds a field with an error or with
 * data quality findings.
 */
export const ViewAccordionSection = ({
  invalidFields,
  label,
  nodes,
  children,
}: {
  invalidFields?: ReadonlySet<string>;
  label: string;
  nodes: readonly ViewNode[];
  children: ReactNode;
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  useFieldRevealRequest((code) => {
    if (nodesPlacedFields(nodes).has(code)) setExpanded(true);
  });
  const marker = layoutMarker(nodes, invalidFields, useFieldDiagnostics());
  return (
    <Accordion expanded={expanded} onChange={(_, next) => setExpanded(next)}>
      <AccordionSummary
        aria-description={
          marker ? t(layoutMarkerDescriptions[marker.reason]) : undefined
        }
        expandIcon={<ChevronDownIcon />}
      >
        <Box
          sx={{
            alignItems: 'center',
            color: marker?.color,
            display: 'flex',
            gap: 1.5,
          }}
        >
          <Typography color="inherit">{lexiconText(label)}</Typography>
          {marker && <CircleAlertIcon aria-hidden size={compactIconSize} />}
        </Box>
      </AccordionSummary>
      <AccordionDetails>{children}</AccordionDetails>
    </Accordion>
  );
};
