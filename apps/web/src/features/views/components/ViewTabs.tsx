import { Box, Tab, tabClasses, Tabs, tabsClasses } from '@mui/material';
import { CircleAlertIcon } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { ViewNode } from '../../records/api';
import { lexiconText } from '../../lexicon/lexicon';
import { nodesPlacedFields } from '../viewFieldComponents';
import { useFieldDiagnostics } from '../../records/fieldDiagnostics';
import { layoutMarker, layoutMarkerDescriptions } from '../layoutMarker';
import { compactIconSize } from '../../../components/iconSizes';
import { VIEW_TAB_MIN_HEIGHT, VIEW_TAB_PANEL_SPACING } from '../constants';
import { useFieldRevealRequest } from '../fieldReveal';

export const ViewTabs = ({
  invalidFields,
  panelSpacing = VIEW_TAB_PANEL_SPACING,
  tabs,
  render,
}: {
  /**
   * Fields with errors; tabs holding one are marked, as only one renders.
   * Tabs holding a field with data quality findings are marked too.
   */
  invalidFields?: ReadonlySet<string>;
  /** Space above the panel, larger when it opens on an editable field. */
  panelSpacing?: number;
  tabs: { label: string; children: ViewNode[] }[];
  render: (nodes: ViewNode[]) => ReactNode;
}) => {
  const { t } = useTranslation();
  const [value, setValue] = useState(0);
  // The tab list can shrink under a mounted strip, as in a live preview.
  const selected = Math.max(0, Math.min(value, tabs.length - 1));
  const tabId = useId();
  useFieldRevealRequest((code) => {
    const index = tabs.findIndex((tab) =>
      nodesPlacedFields(tab.children).has(code),
    );
    if (index >= 0) setValue(index);
  });
  const diagnostics = useFieldDiagnostics();
  return (
    <>
      <Tabs
        onChange={(_, next) => setValue(next)}
        // View tabs are bordered rather than underlined. The line under the
        // strip is drawn by the unselected tabs and a filler after them, so
        // the selected tab opens into the panel on any background.
        slotProps={{ indicator: { sx: { display: 'none' } } }}
        sx={{
          borderBottom: 0,
          [`& .${tabsClasses.list}::after`]: {
            borderBottom: 1,
            borderColor: 'divider',
            content: '""',
            flexGrow: 1,
          },
        }}
        value={tabs.length > 0 ? selected : false}
        variant="scrollable"
      >
        {tabs.map((tab, index) => {
          const marker = layoutMarker(tab.children, invalidFields, diagnostics);
          return (
            <Tab
              aria-controls={`${tabId}-tabpanel-${index}`}
              aria-description={
                marker ? t(layoutMarkerDescriptions[marker.reason]) : undefined
              }
              icon={
                marker ? <CircleAlertIcon size={compactIconSize} /> : undefined
              }
              iconPosition="end"
              id={`${tabId}-tab-${index}`}
              key={index}
              label={lexiconText(tab.label)}
              sx={[
                {
                  bgcolor: 'action.hover',
                  border: 1,
                  borderColor: 'divider',
                  borderTopLeftRadius: (theme) => theme.shape.borderRadius,
                  borderTopRightRadius: (theme) => theme.shape.borderRadius,
                  '& + &': { borderLeft: 0 },
                  [`&.${tabClasses.selected}`]: {
                    bgcolor: 'transparent',
                    borderBottomColor: 'transparent',
                  },
                },
                marker
                  ? { color: marker.color, minHeight: VIEW_TAB_MIN_HEIGHT }
                  : false,
              ]}
            />
          );
        })}
      </Tabs>
      {tabs[selected] && (
        <Box
          aria-labelledby={`${tabId}-tab-${selected}`}
          id={`${tabId}-tabpanel-${selected}`}
          role="tabpanel"
          sx={{ pt: panelSpacing }}
        >
          {render(tabs[selected].children)}
        </Box>
      )}
    </>
  );
};
