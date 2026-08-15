import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const require = createRequire(path.resolve("web/package.json"));
const postcss = require("postcss");

const stylesDirectory = "web/src/styles";
const files = fs.readdirSync(stylesDirectory)
  .filter((name) => name.endsWith(".css"))
  .sort();
const declarations = new Map();
const errors = [];

function contextFor(node) {
  const context = [];
  for (let parent = node.parent; parent; parent = parent.parent) {
    if (parent.type === "atrule") {
      context.unshift(`@${parent.name} ${parent.params}`.trim());
    }
  }
  return context.join(" > ");
}

for (const file of files) {
  const source = path.join(stylesDirectory, file);
  const root = postcss.parse(fs.readFileSync(source, "utf8"), { from: source });

  root.walkRules((rule) => {
    const selectors = rule.selectors ?? [rule.selector];
    const properties = new Map();

    rule.each((node) => {
      if (node.type !== "decl") return;
      const location = `${source}:${node.source.start.line}`;
      if (properties.has(node.prop)) {
        errors.push(`${location}: ${rule.selector} repeats ${node.prop} in one rule`);
      }
      properties.set(node.prop, node.value);

      for (const selector of selectors) {
        const key = [contextFor(rule), selector.trim(), node.prop].join("\u0000");
        const previous = declarations.get(key);
        if (previous) {
          const change = previous.value === node.value
            ? `repeats ${node.prop}: ${node.value}`
            : `overrides ${node.prop}: ${previous.value} -> ${node.value}`;
          errors.push(`${location}: ${selector.trim()} ${change} (${previous.location})`);
        } else if (!previous) {
          declarations.set(key, { value: node.value, location });
        }
      }
    });
  });
}

if (errors.length > 0) {
  console.error("Conflicting CSS ownership detected:\n" + errors.join("\n"));
  process.exit(1);
}
