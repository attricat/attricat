/** Characters of a UUID shown where a short, recognisable ID is enough. */
export const shortIdLength = 8;

/** The leading characters of an ID, for titles and labels that lack a name. */
export const shortId = (id: string) => id.slice(0, shortIdLength);
