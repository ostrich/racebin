import { readFile, writeFile } from "node:fs/promises";

const [input, output] = process.argv.slice(2);
if (!input || !output) {
  throw new Error("usage: node normalize-openapi.mjs INPUT OUTPUT");
}

function normalize(value) {
  if (Array.isArray(value)) return value.map(normalize);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => [key, normalize(item)])
  );
}

const document = normalize(JSON.parse(await readFile(input, "utf8")));
await writeFile(output, `${JSON.stringify(document, null, 2)}\n`);
