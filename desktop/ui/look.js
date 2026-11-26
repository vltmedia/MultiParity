// MultiParity desktop: the Look view, where people choose a theme. Themes come from themes.js
// (generated from design/themes) and are applied by theme-engine.js.
"use strict";

(function () {
  const GROUPS = ["All", ...new Set(MP_THEMES.map((t) => t.grp))];
  let filter = "All";

  // A theme's card shows its own colours, corners and type, so the choice is visible at a glance.
  function card(t) {
    const [side, sideInk, , paper, cardBg, ink, mute, line, acc, accInk, hold, nw] = t.c;
    const cur = MPTheme.saved() === t.id;
    const b = document.createElement("button");
    b.type = "button";
    b.className = "look" + (cur ? " cur" : "");
    b.setAttribute("aria-pressed", String(cur));
    b.dataset.id = t.id;
    const r = `${t.r}px`;
    const br = `${t.br ?? t.r}px`;
    const solid = (c) => (String(c).includes("gradient") ? "#ffffff" : c);
    b.innerHTML = `
      <span class="look-art" style="background:${paper};background-image:${t.bgimg || "none"};background-size:${t.bgsize || "auto"}">
        <span class="look-side" style="background:${side}"><i style="background:${solid(sideInk)}"></i><i style="background:${solid(sideInk)};opacity:.5"></i><i style="background:${solid(sideInk)};opacity:.5"></i></span>
        <span class="look-panel" style="background:${cardBg};border:${t.pw || t.bw} solid ${t.panelLine || ink};border-radius:${r};box-shadow:${t.sh || "none"}">
          <span class="look-title" style="color:${ink};font-family:${t.display}">Aa</span>
          <span class="look-row"><i style="background:${nw}"></i><b style="background:${line}"></b></span>
          <span class="look-row"><i style="background:${hold}"></i><b style="background:${line}"></b></span>
          <span class="look-btn" style="background:${acc};color:${accInk};border-radius:${br}">Publish</span>
        </span>
      </span>
      <span class="look-name"><strong></strong><small></small></span>`;
    b.querySelector("strong").textContent = t.name;
    b.querySelector("small").textContent = cur ? "In use" : t.grp;
    b.title = t.desc;
    void mute;
    return b;
  }

  function render() {
    const bar = document.getElementById("look-filters");
    bar.replaceChildren(...GROUPS.map((g) => {
      const x = document.createElement("button");
      x.type = "button";
      x.textContent = g;
      x.setAttribute("aria-pressed", String(g === filter));
      x.addEventListener("click", () => { filter = g; render(); });
      return x;
    }));
    const list = MP_THEMES.filter((t) => filter === "All" || t.grp === filter);
    document.getElementById("looks").replaceChildren(...list.map(card));
  }

  document.getElementById("looks").addEventListener("click", (e) => {
    const b = e.target.closest(".look");
    if (!b) return;
    const t = MP_THEMES.find((x) => x.id === b.dataset.id);
    MPTheme.apply(t);
    MPTheme.save(t.id);
    render();
    if (window.toast) toast(`Now using ${t.name}.`);
  });

  render();
})();
