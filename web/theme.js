"use strict";

(() => {
  const supportedThemes = ["system", "light", "dark"];
  let theme = "system";
  try {
    const savedTheme = window.localStorage.getItem("lorehub_theme");
    if (supportedThemes.includes(savedTheme)) theme = savedTheme;
  } catch (_) {
    // Keep the system default when browser storage is unavailable.
  }

  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const effectiveTheme = theme === "system" ? (prefersDark ? "dark" : "light") : theme;
  document.documentElement.dataset.theme = theme;
  document.documentElement.dataset.colorScheme = effectiveTheme;
  document.querySelector('meta[name="theme-color"]').content = effectiveTheme === "dark" ? "#15131d" : "#17142f";
})();
