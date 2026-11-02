const wholePercent = 100;

/** How far along `done` is, for a progress bar; nothing to do counts as done. */
export const percentOf = (done: number, total: number) =>
  total === 0
    ? wholePercent
    : Math.min(wholePercent, (done / total) * wholePercent);
