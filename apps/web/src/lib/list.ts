/** Insert or replace an item by id (new items are prepended by default). */
export function upsertById<T extends { id: string }>(list: T[] | undefined, item: T, prepend = true): T[] {
  const l = list ?? [];
  const i = l.findIndex((x) => x.id === item.id);
  if (i === -1) return prepend ? [item, ...l] : [...l, item];
  const copy = l.slice();
  copy[i] = item;
  return copy;
}
