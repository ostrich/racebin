type CachedValue = { data: unknown; updatedAt: number };
type QueryEntry =
  | { phase: "ready"; value: CachedValue }
  | { phase: "loading"; request: Promise<unknown>; previous?: CachedValue };

function cachedValue(entry?: QueryEntry): CachedValue | undefined {
  return entry?.phase === "ready" ? entry.value : entry?.previous;
}

const entries = new Map<string, QueryEntry>();
const DEFAULT_FRESHNESS_MS = 30_000;
let generation = 0;

export function cachedQuery<T>(key: string): T | undefined {
  return cachedValue(entries.get(key))?.data as T | undefined;
}

export function loadQuery<T>(
  key: string,
  loader: () => Promise<T>,
  { freshForMs = DEFAULT_FRESHNESS_MS }: { freshForMs?: number } = {}
): Promise<T> {
  const existing = entries.get(key);
  if (existing?.phase === "loading") return existing.request as Promise<T>;
  const previous = cachedValue(existing);
  if (previous !== undefined && Date.now() - previous.updatedAt < freshForMs) {
    return Promise.resolve(previous.data as T);
  }

  const requestGeneration = generation;
  const request = loader()
    .then((data) => {
      if (generation === requestGeneration)
        entries.set(key, { phase: "ready", value: { data, updatedAt: Date.now() } });
      return data;
    })
    .catch((error) => {
      const current = entries.get(key);
      if (current?.phase === "loading" && current.request === request) {
        if (!current.previous) entries.delete(key);
        else entries.set(key, { phase: "ready", value: current.previous });
      }
      throw error;
    });
  entries.set(key, { phase: "loading", previous, request });
  return request;
}

export function clearQueryCache(): void {
  generation += 1;
  entries.clear();
}
