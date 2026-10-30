import {
  Box,
  ListItemIcon,
  Menu,
  MenuItem,
  Paper,
  Tab,
  Tabs,
} from '@mui/material';
import { EllipsisIcon, type LucideIcon } from 'lucide-react';
import { useId, useState, type ReactNode, type Ref } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../../components/iconSizes';
import { recordPreviewMoreTab, type RecordPreviewTabKey } from '../constants';

type RecordPreviewTab = {
  key: RecordPreviewTabKey;
  /** The system icon of the section's concept. */
  icon: LucideIcon;
  label: string;
  content: ReactNode;
  /** Listed in the More menu rather than the tab bar. */
  overflow?: boolean;
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
  const [moreAnchor, setMoreAnchor] = useState<HTMLElement | null>(null);
  if (tabs.length === 0) return null;
  const selected = tabs.some((tab) => tab.key === value) ? value : tabs[0].key;
  const overflow = tabs.filter((tab) => tab.overflow);
  // The More tab stands in for the overflow tab that is open.
  const openOverflow = overflow.find((tab) => tab.key === selected);
  const moreTabId = `${id}-tab-${recordPreviewMoreTab}`;
  const MoreIcon = openOverflow?.icon ?? EllipsisIcon;
  return (
    <Paper
      component="section"
      aria-label={t('records.recordTabs')}
      sx={{ mt: 3 }}
    >
      <Tabs
        onChange={(
          _,
          next: RecordPreviewTabKey | typeof recordPreviewMoreTab,
        ) => {
          if (next !== recordPreviewMoreTab) onChange(next);
        }}
        sx={{ px: { xs: 1, md: 2 } }}
        value={selected}
        variant="scrollable"
      >
        {tabs
          .filter((tab) => !tab.overflow)
          .map((tab) => (
            <Tab
              aria-controls={`${id}-tabpanel-${tab.key}`}
              icon={<tab.icon aria-hidden size={compactIconSize} />}
              id={`${id}-tab-${tab.key}`}
              key={tab.key}
              label={tab.label}
              value={tab.key}
            />
          ))}
        {overflow.length > 0 && (
          <Tab
            aria-controls={
              openOverflow ? `${id}-tabpanel-${openOverflow.key}` : undefined
            }
            aria-haspopup="menu"
            icon={<MoreIcon aria-hidden size={compactIconSize} />}
            id={moreTabId}
            label={openOverflow?.label ?? t('records.moreTabs')}
            onClick={(event) => setMoreAnchor(event.currentTarget)}
            value={openOverflow?.key ?? recordPreviewMoreTab}
          />
        )}
      </Tabs>
      <Menu
        anchorEl={moreAnchor}
        onClose={() => setMoreAnchor(null)}
        open={Boolean(moreAnchor)}
        slotProps={{ list: { 'aria-labelledby': moreTabId } }}
      >
        {overflow.map((tab) => (
          <MenuItem
            key={tab.key}
            onClick={() => {
              setMoreAnchor(null);
              onChange(tab.key);
            }}
            selected={tab.key === selected}
          >
            <ListItemIcon>
              <tab.icon aria-hidden size={compactIconSize} />
            </ListItemIcon>
            {tab.label}
          </MenuItem>
        ))}
      </Menu>
      {tabs.map((tab) => (
        <Box
          aria-labelledby={tab.overflow ? moreTabId : `${id}-tab-${tab.key}`}
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
