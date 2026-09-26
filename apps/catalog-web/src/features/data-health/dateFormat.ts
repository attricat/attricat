export const formatDataHealthDate = (
  value: string | null,
  language: string,
  never: string,
) =>
  value
    ? new Intl.DateTimeFormat(language, { dateStyle: 'medium' }).format(
        new Date(value),
      )
    : never;
