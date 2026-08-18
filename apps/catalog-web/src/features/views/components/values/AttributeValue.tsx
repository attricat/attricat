import { Chip, Stack, Typography } from '@mui/material';
import { Link } from '@tanstack/react-router';
import type { Attribute } from '../../../entities/api';

type RelationshipValue = { items?: { id: string; display?: string }[]; truncated?: boolean };

const isRelationshipValue = (value: unknown): value is RelationshipValue =>
  typeof value === 'object' && value !== null && 'items' in value;

export const formatAttributeValue = (attribute: Attribute, value: unknown) => {
  if (value === null || value === undefined) return 'Not set';
  if (attribute.value_type === 'boolean') return value ? 'Yes' : 'No';
  if (attribute.value_type === 'date' && typeof value === 'string') {
    return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeZone: 'UTC' }).format(new Date(`${value}T00:00:00Z`));
  }
  if (attribute.value_type === 'datetime' && typeof value === 'string') {
    return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value));
  }
  if (attribute.value_type === 'time' && typeof value === 'object' && value !== null) {
    const time = (value as { time?: unknown; time_zone?: unknown }).time;
    const zone = (value as { time_zone?: unknown }).time_zone;
    return typeof time === 'string' && typeof zone === 'string' ? `${time} ${zone}` : 'Invalid time';
  }
  return String(value);
};

export const AttributeValue = ({ attribute, value, compact = false }: { attribute: Attribute; value: unknown; compact?: boolean }) => {
  if (attribute.value_type === 'relationship' && isRelationshipValue(value)) {
    const items = value.items ?? [];
    if (compact) return <Typography variant="body2">{items.length ? `${items.length} linked` : 'Not set'}</Typography>;
    return (
      <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }}>
        {items.map((item) => <Link key={item.id} params={{ entityId: item.id }} to="/entities/$entityId"><Chip label={item.display ?? item.id} clickable /></Link>)}
        {value.truncated && <Chip label="More linked entities" variant="outlined" />}
        {!items.length && <Typography color="text.secondary">Not set</Typography>}
      </Stack>
    );
  }
  if (attribute.value_type === 'boolean' && value !== null && value !== undefined && !compact) {
    return <Chip color={value ? 'success' : 'default'} label={formatAttributeValue(attribute, value)} size="small" />;
  }
  return <Typography color={value === null || value === undefined ? 'text.secondary' : undefined} variant="body2">{formatAttributeValue(attribute, value)}</Typography>;
};
