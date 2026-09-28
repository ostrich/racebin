import hljs from "highlight.js/lib/core";
import type { LanguageFn } from "highlight.js";
import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import csharp from "highlight.js/lib/languages/csharp";
import css from "highlight.js/lib/languages/css";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import kotlin from "highlight.js/lib/languages/kotlin";
import lua from "highlight.js/lib/languages/lua";
import markdown from "highlight.js/lib/languages/markdown";
import php from "highlight.js/lib/languages/php";
import python from "highlight.js/lib/languages/python";
import r from "highlight.js/lib/languages/r";
import ruby from "highlight.js/lib/languages/ruby";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import swift from "highlight.js/lib/languages/swift";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import { normalizeLanguage } from "./languages";

type LanguageModule = { default: LanguageFn };
type LanguageLoader = () => Promise<LanguageModule>;

const commonLanguages: Array<[string, LanguageFn]> = [
  ["bash", bash],
  ["c", c],
  ["cpp", cpp],
  ["csharp", csharp],
  ["css", css],
  ["go", go],
  ["java", java],
  ["javascript", javascript],
  ["json", json],
  ["kotlin", kotlin],
  ["lua", lua],
  ["markdown", markdown],
  ["php", php],
  ["python", python],
  ["r", r],
  ["ruby", ruby],
  ["rust", rust],
  ["sql", sql],
  ["swift", swift],
  ["typescript", typescript],
  ["xml", xml],
  ["yaml", yaml]
];

for (const [name, definition] of commonLanguages) {
  hljs.registerLanguage(name, definition);
}

const lazyLanguages: Record<string, LanguageLoader> = {
  apache: () => import("highlight.js/lib/languages/apache"),
  armasm: () => import("highlight.js/lib/languages/armasm"),
  asciidoc: () => import("highlight.js/lib/languages/asciidoc"),
  awk: () => import("highlight.js/lib/languages/awk"),
  cmake: () => import("highlight.js/lib/languages/cmake"),
  coffeescript: () => import("highlight.js/lib/languages/coffeescript"),
  crystal: () => import("highlight.js/lib/languages/crystal"),
  dart: () => import("highlight.js/lib/languages/dart"),
  diff: () => import("highlight.js/lib/languages/diff"),
  dockerfile: () => import("highlight.js/lib/languages/dockerfile"),
  elixir: () => import("highlight.js/lib/languages/elixir"),
  elm: () => import("highlight.js/lib/languages/elm"),
  erlang: () => import("highlight.js/lib/languages/erlang"),
  fortran: () => import("highlight.js/lib/languages/fortran"),
  fsharp: () => import("highlight.js/lib/languages/fsharp"),
  glsl: () => import("highlight.js/lib/languages/glsl"),
  graphql: () => import("highlight.js/lib/languages/graphql"),
  groovy: () => import("highlight.js/lib/languages/groovy"),
  haskell: () => import("highlight.js/lib/languages/haskell"),
  http: () => import("highlight.js/lib/languages/http"),
  ini: () => import("highlight.js/lib/languages/ini"),
  julia: () => import("highlight.js/lib/languages/julia"),
  latex: () => import("highlight.js/lib/languages/latex"),
  lisp: () => import("highlight.js/lib/languages/lisp"),
  makefile: () => import("highlight.js/lib/languages/makefile"),
  matlab: () => import("highlight.js/lib/languages/matlab"),
  nginx: () => import("highlight.js/lib/languages/nginx"),
  nim: () => import("highlight.js/lib/languages/nim"),
  nix: () => import("highlight.js/lib/languages/nix"),
  objectivec: () => import("highlight.js/lib/languages/objectivec"),
  ocaml: () => import("highlight.js/lib/languages/ocaml"),
  perl: () => import("highlight.js/lib/languages/perl"),
  powershell: () => import("highlight.js/lib/languages/powershell"),
  protobuf: () => import("highlight.js/lib/languages/protobuf"),
  scala: () => import("highlight.js/lib/languages/scala"),
  scheme: () => import("highlight.js/lib/languages/scheme"),
  scss: () => import("highlight.js/lib/languages/scss"),
  smalltalk: () => import("highlight.js/lib/languages/smalltalk"),
  stata: () => import("highlight.js/lib/languages/stata"),
  vbnet: () => import("highlight.js/lib/languages/vbnet"),
  verilog: () => import("highlight.js/lib/languages/verilog"),
  vhdl: () => import("highlight.js/lib/languages/vhdl"),
  vim: () => import("highlight.js/lib/languages/vim"),
  wasm: () => import("highlight.js/lib/languages/wasm")
};

const commonIds = commonLanguages.map(([name]) => name);
const loading = new Map<string, Promise<void>>();
const MAX_HIGHLIGHT_LENGTH = 250_000;

async function ensureLanguage(language: string): Promise<boolean> {
  if (hljs.getLanguage(language)) return true;
  const loader = lazyLanguages[language];
  if (!loader) return false;
  let promise = loading.get(language);
  if (!promise) {
    promise = loader().then((module) => {
      hljs.registerLanguage(language, module.default);
    });
    loading.set(language, promise);
  }
  try {
    await promise;
    return true;
  } catch {
    loading.delete(language);
    return false;
  }
}

function escapeHtml(code: string): string {
  return code.replace(
    /[&<>]/g,
    (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[character]!
  );
}

export async function highlightedCode(
  code: string,
  language: string
): Promise<{ html: string; language?: string; relevance?: number }> {
  const canonicalLanguage = normalizeLanguage(language) ?? "plaintext";
  if (canonicalLanguage === "plaintext" || code.length > MAX_HIGHLIGHT_LENGTH) {
    return { html: escapeHtml(code) };
  }
  if (canonicalLanguage === "auto") {
    const result = hljs.highlightAuto(code, commonIds);
    return {
      html: result.value,
      language: result.language,
      relevance: result.relevance
    };
  }
  if (!(await ensureLanguage(canonicalLanguage))) {
    return { html: escapeHtml(code) };
  }
  return {
    html: hljs.highlight(code, { language: canonicalLanguage, ignoreIllegals: true }).value,
    language: canonicalLanguage
  };
}
