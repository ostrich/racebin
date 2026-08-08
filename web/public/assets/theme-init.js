(() => {
  let theme = "auto";
  try {
    const stored = localStorage.getItem("racebin.colorTheme");
    if (stored === "dark" || stored === "light") theme = stored;
  } catch {
    // The system preference remains available when storage is unavailable.
  }
  const colorScheme = theme === "auto"
    ? matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
    : theme;
  const root = document.documentElement;
  root.dataset.theme = theme;
  root.dataset.colorScheme = colorScheme;
  root.style.colorScheme = colorScheme;
})();
