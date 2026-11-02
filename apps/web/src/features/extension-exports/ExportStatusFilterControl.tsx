import { workspaceExportStatusOptions } from './queryOptions';
import { Chip, ListSubheader, MenuItem } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { CircleXIcon, FilterIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useCan } from '../auth/useCan';
import { exportStatuses, type ExportStatus } from './constants';
import { capabilities } from '../auth/capabilities';
import { FilterMenuButton } from '../../components/FilterMenuButton';

export type ExportStatusFilterValue = {
  extension: string;
  connection?: string;
  status: ExportStatus;
};

/**
 * Filters Explorer results by export status at one destination: a chip
 * while the filter applies, otherwise a menu of the export destinations.
 */
export const ExportStatusFilterControl = ({
  value,
  onChange,
}: {
  value: ExportStatusFilterValue | undefined;
  onChange: (value: ExportStatusFilterValue | undefined) => void;
}) => {
  const { t } = useTranslation();
  const canRead = useCan(capabilities.dataHealthRead);
  const overview = useQuery({
    ...workspaceExportStatusOptions(),
    enabled: canRead,
  });
  const destinations = overview.data?.destinations ?? [];
  const destinationTitle = (extensionId: string) =>
    destinations.find((item) => item.extension_id === extensionId)?.title ??
    extensionId;
  if (value) {
    const label = t('extensionExports.filterChip', {
      // The connection tells apart filters on one extension's accounts.
      destination: value.connection
        ? `${destinationTitle(value.extension)} · ${value.connection}`
        : destinationTitle(value.extension),
      status: t(`extensionExports.status.${value.status}`),
    });
    return (
      <Chip
        deleteIcon={
          <CircleXIcon
            aria-label={t('extensionExports.removeFilter', { filter: label })}
          />
        }
        label={label}
        size="small"
        title={label}
        onDelete={() => onChange(undefined)}
      />
    );
  }
  if (!destinations.length) return null;
  // One menu entry per destination and connection that has export state.
  const targets = [
    ...new Map(
      [
        // The default destination only for exports without connections;
        // one of a connection kind records every status per connection.
        ...destinations
          .filter((destination) => !destination.connection_kind)
          .map((destination) => ({
            extension: destination.extension_id,
            connection: undefined as string | undefined,
          })),
        ...(overview.data?.counts ?? []).map((count) => ({
          extension: count.extension_id,
          connection: count.connection ?? undefined,
        })),
      ].map((target) => [
        `${target.extension}:${target.connection ?? ''}`,
        target,
      ]),
    ).values(),
  ];
  return (
    <FilterMenuButton
      icon={FilterIcon}
      label={t('extensionExports.filterByExport')}
    >
      {(close) =>
        targets.flatMap((target) => [
          <ListSubheader key={`${target.extension}:${target.connection ?? ''}`}>
            {destinationTitle(target.extension)}
            {target.connection ? ` · ${target.connection}` : ''}
          </ListSubheader>,
          ...exportStatuses.map((status) => (
            <MenuItem
              key={`${target.extension}:${target.connection ?? ''}:${status}`}
              onClick={() => {
                close();
                onChange({ ...target, status });
              }}
            >
              {t(`extensionExports.status.${status}`)}
            </MenuItem>
          )),
        ])
      }
    </FilterMenuButton>
  );
};
