interface QueryEntry {
  data?: unknown;
  request?: Promise<unknown>;
}

const entries = new Map<string, QueryEntry>();
let generation = 0;

export function cachedQuery<T>(key: string): T | undefined {
  return entries.get(key)?.data as T | undefined;
}

export function loadQuery<T>(key: string, loader: () => Promise<T>): Promise<T> {
  const existing = entries.get(key);
  if (existing?.request) return existing.request as Promise<T>;

  const requestGeneration = generation;
  const request = loader().then(data => {
    if (generation === requestGeneration) entries.set(key, { data });
    return data;
  }).catch(error => {
    const current = entries.get(key);
    if (current?.request === request) {
      if (current.data === undefined) entries.delete(key);
      else entries.set(key, { data: current.data });
    }
    throw error;
  });
  entries.set(key, { data: existing?.data, request });
  return request;
}

export function clearQueryCache(): void {
  generation += 1;
  entries.clear();
}
