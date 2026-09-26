export const formatBlueprintDateTime = (
  value: string | null,
  unpublishedLabel: string,
) =>
  value
    ? new Intl.DateTimeFormat(undefined, {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(new Date(value))
    : unpublishedLabel;
