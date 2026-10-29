import { Box, Paper, Tab, Tabs } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import { useId, type ReactNode, type Ref } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../../components/iconSizes';
import type { RecordPreviewTabKey } from '../constants';

type RecordPreviewTab = {
  key: RecordPreviewTabKey;
  /** The system icon of the section's concept. */
  icon: LucideIcon;
  label: string;
  content: ReactNode;
};

/**
 * Sections of a record, its values first, shown one at a time. Hidden panels
 * stay mounted, so a panel keeps following its data, and unsaved field edits
 * and a comment being written survive switching tabs.
 */
export const RecordPreviewTabs = ({
  tabs,
  value,
  onChange,
  panelRef,
}: {
  tabs: readonly RecordPreviewTab[];
  /** The selected tab; the first one is shown when it is not available. */
  value: RecordPreviewTabKey;
  onChange: (key: RecordPreviewTabKey) => void;
  /** The visible panel, focused when another control opens a tab. */
  panelRef?: Ref<HTMLDivElement>;
}) => {
  const { t } = useTranslation();
  const id = useId();
  if (tabs.length === 0) return null;
  const selected = tabs.some((tab) => tab.key === value) ? value : tabs[0].key;
  return (
    <Paper
      component="section"
      aria-label={t('records.recordTabs')}
      sx={{ mt: 3 }}
    >
      <Tabs
        onChange={(_, next: RecordPreviewTabKey) => onChange(next)}
        sx={{ px: { xs: 1, md: 2 } }}
        value={selected}
        variant="scrollable"
      >
        {tabs.map((tab) => (
          <Tab
            aria-controls={`${id}-tabpanel-${tab.key}`}
            icon={<tab.icon aria-hidden size={compactIconSize} />}
            id={`${id}-tab-${tab.key}`}
            key={tab.key}
            label={tab.label}
            value={tab.key}
          />
        ))}
      </Tabs>
      {tabs.map((tab) => (
        <Box
          aria-labelledby={`${id}-tab-${tab.key}`}
          hidden={tab.key !== selected}
          id={`${id}-tabpanel-${tab.key}`}
          key={tab.key}
          ref={tab.key === selected ? panelRef : undefined}
          role="tabpanel"
          sx={{ p: { xs: 2, md: 3 } }}
          tabIndex={-1}
        >
          {tab.content}
        </Box>
      ))}
    </Paper>
  );
};
