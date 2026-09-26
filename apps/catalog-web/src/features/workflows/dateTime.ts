export const formatWorkflowDateTime = (
  value: string | null,
  locale: string,
  fallback: string,
) =>
  value
    ? new Intl.DateTimeFormat(locale, {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(new Date(value))
    : fallback;
