// Applies the theme before first paint. An external classic script (no inline script, CSP
// script-src 'self'), loaded synchronously from <head>. Keep in sync with src/lib/theme.ts
// (resolveTheme, schemeFor, THEME_STORAGE_KEY, THEME_COLOR); theme.test.ts runs this file and
// compares.
(function () {
  var theme = "dark";
  try {
    var stored = window.localStorage.getItem("guardedsleep.theme");
    if (stored === "light") theme = "light";
    else if (stored === "system") {
      theme = window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
    }
  } catch (_error) {
    // Storage can be blocked (private window, site data off): the dark default stays.
  }
  var light = theme === "light";
  document.documentElement.dataset.theme = theme;
  document.documentElement.dataset.scheme = light ? "day" : "dusk";
  var meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute("content", light ? "#faf7f4" : "#141218");
})();
