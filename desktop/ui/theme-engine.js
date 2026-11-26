// MultiParity themes: turns a theme from themes.json into CSS custom properties on the page,
// effect switches (data-fx) and the web fonts it needs. Shared by the desktop app (plain
// script) and the web UI (imported for its side effect). Exposes globalThis.MPTheme.
(function () {
  "use strict";

  // Web fonts a theme may use, and the Google Fonts axis spec for each. Bricolage Grotesque
  // (the default) ships with both apps, so it is not fetched.
  const GOOGLE = {
    "Geist": "wght@400;600;700", "Manrope": "wght@400;600;700", "DM Sans": "wght@400;600;700",
    "Space Grotesk": "wght@400;600;700", "Archivo": "wght@400;600;800", "Archivo Black": "",
    "IBM Plex Sans": "wght@400;600", "IBM Plex Mono": "wght@400;600", "VT323": "", "Share Tech Mono": "",
    "Orbitron": "wght@500;800", "Rajdhani": "wght@500;700", "Chakra Petch": "wght@400;600;700", "Michroma": "",
    "Oxanium": "wght@400;600;700", "Martian Mono": "wght@400;700", "Unbounded": "wght@400;700;800",
    "Syne": "wght@400;700;800", "Big Shoulders Display": "wght@500;900", "Sora": "wght@400;600;700",
    "Instrument Sans": "wght@400;600;700", "JetBrains Mono": "wght@400;600", "Roboto": "wght@400;500;700",
    "Roboto Mono": "wght@400;500", "Open Sans": "wght@400;600;700", "Press Start 2P": "", "Caveat": "wght@500;700",
    "Patrick Hand": "", "Permanent Marker": "", "Bangers": "", "Fredoka": "wght@400;600", "Jost": "wght@400;600;800",
    "Rubik": "wght@400;600;800", "Creepster": "", "Special Elite": "", "Black Ops One": "",
    "Fraunces": "wght@400;700", "Nunito": "wght@400;700;800", "Gochi Hand": "",
  };

  const families = (stack) => String(stack || "").split(",").map((f) => f.trim().replace(/^['"]|['"]$/g, "")).filter(Boolean);

  function fontsUrl(t) {
    const want = new Set();
    for (const s of [t.font, t.display, t.mono, t.fileFont]) for (const f of families(s)) if (f in GOOGLE) want.add(f);
    if (!want.size) return null;
    const parts = [...want].map((f) => "family=" + f.replace(/ /g, "+") + (GOOGLE[f] ? ":" + GOOGLE[f] : ""));
    return "https://fonts.googleapis.com/css2?" + parts.join("&") + "&display=swap";
  }

  // Rough lightness of a colour (#hex or rgb/rgba), 0..1. Gradients and unknowns count as light.
  function lightness(c) {
    c = String(c || "").trim();
    let r, g, b;
    const hex = c.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/i);
    const rgb = c.match(/^rgba?\(([^)]+)\)/i);
    if (hex) {
      const h = hex[1].length === 3 ? hex[1].replace(/./g, "$&$&") : hex[1];
      [r, g, b] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16));
    } else if (rgb) {
      [r, g, b] = rgb[1].split(",").map((x) => parseFloat(x));
    } else return 1;
    return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  }

  let applied = [];

  function vars(t) {
    const [side, sideInk, sideMute, paper, card, ink, mute, line, acc, accInk, hold, nw, warn] = t.c;
    const sideIsGradient = String(side).includes("gradient");
    const v = {
      "--side": side, "--sidetext": sideInk, "--sidemute": sideMute,
      "--side2": `color-mix(in srgb, ${sideIsGradient ? "#ffffff" : sideInk} 10%, ${sideIsGradient ? "transparent" : side})`,
      "--paper": paper, "--card": card, "--ink": ink, "--mute": mute, "--line": line,
      "--line2": `color-mix(in srgb, ${line} 55%, transparent)`,
      "--acc": acc, "--accink": accInk, "--accdark": `color-mix(in srgb, ${acc} 78%, ${ink})`,
      "--hold": hold, "--holdbg": t.holdBg || `color-mix(in srgb, ${hold} 12%, transparent)`,
      "--new": nw, "--newbg": `color-mix(in srgb, ${nw} 12%, transparent)`,
      "--bad": warn, "--warn": warn, "--badbg": `color-mix(in srgb, ${warn} 12%, transparent)`,
      "--p-off": line, "--p-on": ink, "--p-now": acc,
      "--font": t.font, "--font-display": t.display || t.font, "--font-mono": t.mono || "monospace", "--file-font": t.fileFont || "inherit",
      "--r": `${t.r}px`, "--r-btn": `${t.br ?? t.r}px`, "--r-small": `${Math.min(t.r, 8)}px`, "--r-ck": t.ckr || `${Math.min(t.r, 4)}px`,
      "--bw": t.bw, "--pw": t.pw || t.bw, "--lw": t.lw || "1px",
      "--btn-line": t.btnLine || ink, "--panel-line": t.panelLine || ink,
      "--sh-panel": t.sh || "none", "--sh-btn": t.bsh || "none",
      "--bg-image": t.bgimg || "none", "--bg-size": t.bgsize || "auto",
      "--blur": t.blurAmt || "18px", "--sat": t.sat || "1.5", "--field": t.field || "transparent",
      "--side-border": t.sideBorder || "0", "--side-sel": t.sideSel || "transparent", "--side-sel-bar": t.sideSelBar || "none",
      "--side-sel-ink": t.sideSelColor || (t.sideSelInk ? ink : sideInk),
      "--glow-text": t.glowText || "none", "--h1-case": t.h1Case || "none", "--h1-track": t.h1Track || "-0.035em",
      "--h1-weight": String(t.h1Weight || 700), "--h1-ink": t.h1Ink || ink, "--h2-case": t.h2Case || "none",
      "--label-case": t.labelCase || "none", "--label-track": t.labelTrack || "0",
      "--logo-case": t.logoCase || "none", "--logo-track": t.logoTrack || "-0.03em", "--logo-weight": String(t.logoWeight || 700),
      "--chamfer": t.chamfer || "none", "--chamfer-big": t.chamferBig || "none",
      "--toast-bg": t.toastBg || ink, "--toast-ink": t.toastInk || (lightness(ink) > 0.5 ? "#111111" : "#ffffff"),
      "--on-paper": t.onPaper || ink,
    };
    // Toasts: a hard offset shadow in hard-shadow themes, a soft one otherwise.
    const hard = /^\s*\d+px \d+px 0 /.test(t.sh || "");
    v["--sh-toast"] = hard ? `5px 5px 0 ${acc}` : "0 12px 32px rgba(0,0,0,.28)";
    v["--sh-toast-bad"] = hard ? `5px 5px 0 ${warn}` : `0 12px 32px rgba(0,0,0,.28), inset 4px 0 0 ${warn}`;
    return v;
  }

  function apply(t, root) {
    root = root || document.documentElement;
    for (const k of applied) root.style.removeProperty(k);
    applied = [];
    const v = vars(t);
    for (const [k, val] of Object.entries(v)) {
      root.style.setProperty(k, val);
      applied.push(k);
    }
    const fx = (t.fx || []).map((f) => f.replace(/^fx-/, ""));
    if (t.fx && t.fx.includes("fx-glass")) fx.push("frost");
    root.dataset.fx = fx.join(" ");
    root.dataset.themeId = t.id;
    const dark = lightness(t.c[5]) > 0.5; // light ink means a dark theme
    root.dataset.dark = String(dark);
    root.style.colorScheme = dark ? "dark" : "light";
    const href = fontsUrl(t);
    let link = document.getElementById("mp-theme-fonts");
    if (href) {
      if (!link) {
        link = document.createElement("link");
        link.id = "mp-theme-fonts";
        link.rel = "stylesheet";
        document.head.appendChild(link);
      }
      if (link.href !== href) link.href = href;
    } else if (link) link.remove();
  }

  const KEY = "mp.theme";
  function saved() {
    try { return localStorage.getItem(KEY) || "ember"; } catch (_) { return "ember"; }
  }
  function save(id) {
    try { localStorage.setItem(KEY, id); } catch (_) {}
  }

  globalThis.MPTheme = { apply, vars, fontsUrl, saved, save, KEY };
})();
