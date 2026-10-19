import { Box, Tab, tabClasses, Tabs, tabsClasses } from '@mui/material';
import { CircleAlertIcon } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { ViewNode } from '../../records/api';
import { lexiconText } from '../../lexicon/lexicon';
import { nodesPlacedFields } from '../viewFieldComponents';
import { compactIconSize } from '../../../components/iconSizes';
import { VIEW_TAB_PANEL_SPACING } from '../constants';
import { useFieldRevealRequest } from '../fieldReveal';

export const ViewTabs = ({
  invalidFields,
  panelSpacing = VIEW_TAB_PANEL_SPACING,
  tabs,
  render,
}: {
  /** Fields with errors; tabs holding one are marked, as only one renders. */
  invalidFields?: ReadonlySet<string>;
  /** Space above the panel, larger when it opens on an editable field. */
  panelSpacing?: number;
  tabs: { label: string; children: ViewNode[] }[];
  render: (nodes: ViewNode[]) => ReactNode;
}) => {
  const { t } = useTranslation();
  const [value, setValue] = useState(0);
  const tabId = useId();
  useFieldRevealRequest((code) => {
    const index = tabs.findIndex((tab) =>
      nodesPlacedFields(tab.children).has(code),
    );
    if (index >= 0) setValue(index);
  });
  const tabHasErrors = (children: readonly ViewNode[]) =>
    Boolean(invalidFields?.size) &&
    [...nodesPlacedFields(children)].some((code) => invalidFields!.has(code));
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
        value={value}
        variant="scrollable"
      >
        {tabs.map((tab, index) => {
          const invalid = tabHasErrors(tab.children);
          return (
            <Tab
              aria-controls={`${tabId}-tabpanel-${index}`}
              aria-description={invalid ? t('views.tabHasErrors') : undefined}
              icon={
                invalid ? <CircleAlertIcon size={compactIconSize} /> : undefined
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
                invalid && { color: 'error.main', minHeight: 48 },
              ]}
            />
          );
        })}
      </Tabs>
      {tabs[value] && (
        <Box
          aria-labelledby={`${tabId}-tab-${value}`}
          id={`${tabId}-tabpanel-${value}`}
          role="tabpanel"
          sx={{ pt: panelSpacing }}
        >
          {render(tabs[value].children)}
        </Box>
      )}
    </>
  );
};
