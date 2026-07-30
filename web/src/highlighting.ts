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

type LanguageModule = { default: LanguageFn };
type LanguageLoader = () => Promise<LanguageModule>;

export type LanguageOption = {
  id: string;
  label: string;
  aliases?: readonly string[];
};

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

export const languageOptions: readonly LanguageOption[] = [
  { id: "none", label: "Plain text", aliases: ["text", "txt", "plaintext"] },
  { id: "auto", label: "Auto detect" },
  { id: "apache", label: "Apache configuration" },
  { id: "armasm", label: "ARM assembly", aliases: ["arm"] },
  { id: "asciidoc", label: "AsciiDoc", aliases: ["adoc"] },
  { id: "awk", label: "Awk" },
  { id: "bash", label: "Bash / Shell", aliases: ["sh", "shell", "zsh"] },
  { id: "c", label: "C" },
  { id: "cmake", label: "CMake" },
  { id: "coffeescript", label: "CoffeeScript", aliases: ["coffee"] },
  { id: "cpp", label: "C++", aliases: ["c++"] },
  { id: "crystal", label: "Crystal" },
  { id: "csharp", label: "C#", aliases: ["cs", "c#"] },
  { id: "css", label: "CSS" },
  { id: "dart", label: "Dart" },
  { id: "diff", label: "Diff / Patch", aliases: ["patch"] },
  { id: "dockerfile", label: "Dockerfile", aliases: ["docker"] },
  { id: "elixir", label: "Elixir", aliases: ["ex"] },
  { id: "elm", label: "Elm" },
  { id: "erlang", label: "Erlang", aliases: ["erl"] },
  { id: "fortran", label: "Fortran" },
  { id: "fsharp", label: "F#", aliases: ["fs", "f#"] },
  { id: "glsl", label: "GLSL" },
  { id: "go", label: "Go", aliases: ["golang"] },
  { id: "graphql", label: "GraphQL" },
  { id: "groovy", label: "Groovy" },
  { id: "haskell", label: "Haskell", aliases: ["hs"] },
  { id: "html", label: "HTML", aliases: ["htm"] },
  { id: "http", label: "HTTP" },
  { id: "ini", label: "INI / TOML", aliases: ["toml"] },
  { id: "java", label: "Java" },
  { id: "javascript", label: "JavaScript", aliases: ["js", "jsx"] },
  { id: "json", label: "JSON" },
  { id: "julia", label: "Julia", aliases: ["jl"] },
  { id: "kotlin", label: "Kotlin", aliases: ["kt", "kts"] },
  { id: "latex", label: "LaTeX", aliases: ["tex"] },
  { id: "lisp", label: "Lisp" },
  { id: "lua", label: "Lua" },
  { id: "makefile", label: "Makefile", aliases: ["make"] },
  { id: "markdown", label: "Markdown", aliases: ["md"] },
  { id: "matlab", label: "MATLAB" },
  { id: "nginx", label: "Nginx configuration" },
  { id: "nim", label: "Nim" },
  { id: "nix", label: "Nix" },
  { id: "objectivec", label: "Objective-C", aliases: ["objc"] },
  { id: "ocaml", label: "OCaml", aliases: ["ml"] },
  { id: "perl", label: "Perl", aliases: ["pl"] },
  { id: "php", label: "PHP" },
  { id: "powershell", label: "PowerShell", aliases: ["ps1"] },
  { id: "protobuf", label: "Protocol Buffers", aliases: ["proto"] },
  { id: "python", label: "Python", aliases: ["py"] },
  { id: "r", label: "R" },
  { id: "ruby", label: "Ruby", aliases: ["rb"] },
  { id: "rust", label: "Rust", aliases: ["rs"] },
  { id: "scala", label: "Scala" },
  { id: "scheme", label: "Scheme" },
  { id: "scss", label: "SCSS" },
  { id: "smalltalk", label: "Smalltalk" },
  { id: "sql", label: "SQL" },
  { id: "stata", label: "Stata" },
  { id: "swift", label: "Swift" },
  { id: "typescript", label: "TypeScript", aliases: ["ts", "tsx"] },
  { id: "vbnet", label: "Visual Basic .NET", aliases: ["vb"] },
  { id: "verilog", label: "Verilog" },
  { id: "vhdl", label: "VHDL" },
  { id: "vim", label: "Vim script" },
  { id: "wasm", label: "WebAssembly", aliases: ["wat"] },
  { id: "xml", label: "XML", aliases: ["svg"] },
  { id: "yaml", label: "YAML", aliases: ["yml"] }
];

const aliases = new Map<string, string>();
for (const language of languageOptions) {
  aliases.set(language.id, language.id);
  for (const alias of language.aliases ?? []) {
    aliases.set(alias, language.id);
  }
}
aliases.set("html", "xml");

const commonIds = commonLanguages.map(([name]) => name);
const loading = new Map<string, Promise<void>>();
const MAX_HIGHLIGHT_LENGTH = 250_000;

export function normalizeSyntax(value: string): string | undefined {
  return aliases.get(value.trim().toLowerCase());
}

export function languageMenu(): string {
  return languageOptions
    .map(language => {
      const search = [language.id, language.label, ...(language.aliases ?? [])]
        .join(" ")
        .toLowerCase();
      return `<button type="button" role="option" data-language="${language.id}" data-search="${search}">${language.label}<small>${language.id}</small></button>`;
    })
    .join("");
}

export function connectLanguagePicker(
  input: HTMLInputElement,
  menu: HTMLElement
): void {
  const options = [...menu.querySelectorAll<HTMLButtonElement>("[data-language]")];
  let active = -1;

  const visible = () => options.filter(option => !option.hidden);
  const setActive = (index: number) => {
    const choices = visible();
    choices.forEach(option => option.classList.remove("active"));
    if (!choices.length) {
      active = -1;
      return;
    }
    active = (index + choices.length) % choices.length;
    choices[active]!.classList.add("active");
    choices[active]!.scrollIntoView({ block: "nearest" });
  };
  const open = (query = "") => {
    const term = query.trim().toLowerCase();
    options.forEach(option => {
      option.hidden = Boolean(term && !option.dataset.search!.includes(term));
      option.classList.toggle(
        "selected",
        option.dataset.language === normalizeSyntax(input.value)
      );
    });
    menu.hidden = false;
    input.setAttribute("aria-expanded", "true");
    active = -1;
  };
  const close = () => {
    menu.hidden = true;
    input.setAttribute("aria-expanded", "false");
    active = -1;
  };
  const choose = (option: HTMLButtonElement) => {
    input.value = option.dataset.language!;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    close();
  };

  input.addEventListener("focus", () => {
    input.select();
    open();
  });
  input.addEventListener("input", () => open(input.value));
  input.addEventListener("blur", () => window.setTimeout(close, 100));
  input.addEventListener("keydown", event => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (menu.hidden) open();
      setActive(active + (event.key === "ArrowDown" ? 1 : -1));
    } else if (event.key === "Enter" && active >= 0) {
      event.preventDefault();
      choose(visible()[active]!);
    } else if (event.key === "Escape") {
      close();
    }
  });
  menu.addEventListener("mousedown", event => event.preventDefault());
  menu.addEventListener("click", event => {
    const option = (event.target as HTMLElement).closest<HTMLButtonElement>("[data-language]");
    if (option) choose(option);
  });
}

async function ensureLanguage(language: string): Promise<boolean> {
  if (hljs.getLanguage(language)) return true;
  const loader = lazyLanguages[language];
  if (!loader) return false;
  let promise = loading.get(language);
  if (!promise) {
    promise = loader().then(module => {
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
    character => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[character]!
  );
}

export async function highlightedCode(
  code: string,
  syntax: string
): Promise<{ html: string; language?: string }> {
  const language = normalizeSyntax(syntax) ?? "none";
  if (language === "none" || code.length > MAX_HIGHLIGHT_LENGTH) {
    return { html: escapeHtml(code) };
  }
  if (language === "auto") {
    const result = hljs.highlightAuto(code, commonIds);
    return { html: result.value, language: result.language };
  }
  if (!(await ensureLanguage(language))) {
    return { html: escapeHtml(code) };
  }
  return {
    html: hljs.highlight(code, { language, ignoreIllegals: true }).value,
    language
  };
}

export async function highlightElement(
  element: HTMLElement,
  code: string,
  syntax: string
): Promise<void> {
  const result = await highlightedCode(code, syntax);
  element.innerHTML = result.html;
  element.classList.add("hljs");
  if (result.language) {
    element.dataset.highlightLanguage = result.language;
  } else {
    delete element.dataset.highlightLanguage;
  }
}

export function connectHighlightedEditor(
  textarea: HTMLTextAreaElement,
  output: HTMLElement,
  language: HTMLInputElement
): void {
  let revision = 0;
  const render = async () => {
    const current = ++revision;
    const result = await highlightedCode(textarea.value, language.value);
    if (current !== revision) return;
    output.innerHTML = `${result.html}\n`;
  };
  const syncScroll = () => {
    output.parentElement!.scrollTop = textarea.scrollTop;
    output.parentElement!.scrollLeft = textarea.scrollLeft;
  };
  textarea.addEventListener("input", () => void render());
  textarea.addEventListener("scroll", syncScroll);
  language.addEventListener("input", () => void render());
  void render();
}
