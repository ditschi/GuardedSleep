// Applies the theme and night mode before first paint. An external classic script (no inline
// script, CSP script-src 'self'), loaded synchronously from <head>. Keep in sync with
// src/lib/theme.ts (resolveTheme, schemeFor, THEME_STORAGE_KEY, THEME_COLOR, NIGHT_THEME_COLOR) and
// src/lib/night-mode.ts (nightActive, NIGHT_STORAGE_KEY); theme.test.ts runs this file and compares.
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

  // Night mode: Off, On, or Automatic by the daily window with a manual override until the next
  // boundary. Anything unreadable means off.
  var night = false;
  try {
    var raw = window.localStorage.getItem("guardedsleep.night");
    var n = raw === null ? null : JSON.parse(raw);
    if (n && n.mode === "on") night = true;
    else if (n && n.mode === "auto") {
      var DAY = 86400000;
      var now = Date.now();
      var o = n.override;
      var from = Number.isInteger(n.from) && n.from >= 0 && n.from < 1440 ? n.from : 1260;
      var to = Number.isInteger(n.to) && n.to >= 0 && n.to < 1440 ? n.to : 420;
      if (
        o &&
        typeof o.active === "boolean" &&
        typeof o.until === "number" &&
        now < o.until &&
        o.until - now <= DAY
      ) {
        night = o.active;
      } else {
        var local = now - new Date(now).getTimezoneOffset() * 60000;
        var minute = Math.floor((((local % DAY) + DAY) % DAY) / 60000);
        if (from === to) night = false;
        else if (from < to) night = minute >= from && minute < to;
        else night = minute >= from || minute < to;
      }
    }
  } catch (_error) {
    // No stored choice or storage blocked: night mode stays off.
  }

  var light = theme === "light";
  document.documentElement.dataset.theme = theme;
  document.documentElement.dataset.scheme = night ? "ember" : light ? "day" : "dusk";
  var meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute("content", night ? "#000000" : light ? "#faf7f4" : "#141218");
})();
