interface QueryEntry {
  data?: unknown;
  request?: Promise<unknown>;
  updatedAt?: number;
}

const entries = new Map<string, QueryEntry>();
const DEFAULT_FRESHNESS_MS = 30_000;
let generation = 0;

export function cachedQuery<T>(key: string): T | undefined {
  return entries.get(key)?.data as T | undefined;
}

export function loadQuery<T>(
  key: string,
  loader: () => Promise<T>,
  { freshForMs = DEFAULT_FRESHNESS_MS }: { freshForMs?: number } = {}
): Promise<T> {
  const existing = entries.get(key);
  if (existing?.request) return existing.request as Promise<T>;
  if (
    existing?.data !== undefined &&
    existing.updatedAt !== undefined &&
    Date.now() - existing.updatedAt < freshForMs
  ) {
    return Promise.resolve(existing.data as T);
  }

  const requestGeneration = generation;
  const request = loader()
    .then((data) => {
      if (generation === requestGeneration) entries.set(key, { data, updatedAt: Date.now() });
      return data;
    })
    .catch((error) => {
      const current = entries.get(key);
      if (current?.request === request) {
        if (current.data === undefined) entries.delete(key);
        else entries.set(key, { data: current.data, updatedAt: current.updatedAt });
      }
      throw error;
    });
  entries.set(key, { data: existing?.data, request, updatedAt: existing?.updatedAt });
  return request;
}

export function clearQueryCache(): void {
  generation += 1;
  entries.clear();
}
