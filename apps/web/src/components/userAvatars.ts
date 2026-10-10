/** Shared avatar sizes so a person looks the same across surfaces. */
export const userAvatarSizes = {
  /** Beside a name inside running text or a table cell. */
  inline: 24,
  /** Leading a list row, matching MUI's `ListItemAvatar`. */
  list: 40,
} as const;

/** Initials scale with the avatar so small and large avatars stay balanced. */
export const initialsFontScale = 0.4;

/** Up to two initials from a display name or, failing that, an email. */
export const userInitials = (name: string) =>
  name
    .split(/[\s@._-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join('');
