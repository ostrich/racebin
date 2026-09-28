export type LanguageOption = {
  id: string;
  label: string;
  aliases?: readonly string[];
};

export const languageOptions: readonly LanguageOption[] = [
  { id: "plaintext", label: "Plain text", aliases: ["text", "txt"] },
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

export function availableLanguageOptions(
  discovered: readonly LanguageOption[]
): readonly LanguageOption[] {
  if (!discovered.length) return languageOptions;
  const byId = new Map(languageOptions.map((language) => [language.id, language]));
  for (const language of discovered) {
    byId.set(language.id, { ...byId.get(language.id), ...language });
  }
  return [...byId.values()].sort((left, right) => {
    if (left.id === "plaintext") return -1;
    if (right.id === "plaintext") return 1;
    if (left.id === "auto") return -1;
    if (right.id === "auto") return 1;
    return left.label.localeCompare(right.label);
  });
}

const aliases = new Map<string, string>();
for (const language of languageOptions) {
  aliases.set(language.id, language.id);
  for (const alias of language.aliases ?? []) aliases.set(alias, language.id);
}
aliases.set("html", "xml");

export function normalizeLanguage(value: string): string | undefined {
  return aliases.get(value.trim().toLowerCase());
}
