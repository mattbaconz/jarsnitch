// Page behavior that doesn't need the analyzer: tabs, the 1 / 2 shortcuts in the tool, copy
// buttons and the logo's read pass. Usable on any page of the site.

/** Selects a tab in its tablist and shows its panel. */
export function selectTab(tab) {
  for (const t of tab.closest('[role="tablist"]').querySelectorAll('[role="tab"]')) {
    const on = t === tab;
    t.setAttribute("aria-selected", String(on));
    t.tabIndex = on ? 0 : -1;
    const panel = document.getElementById(t.getAttribute("aria-controls"));
    if (panel) panel.hidden = !on;
  }
}

for (const list of document.querySelectorAll('[role="tablist"]')) {
  const tabs = [...list.querySelectorAll('[role="tab"]')];
  for (const tab of tabs) {
    tab.addEventListener("click", () => selectTab(tab));
    tab.addEventListener("keydown", (e) => {
      const i = tabs.indexOf(tab);
      const next = {
        ArrowRight: tabs[(i + 1) % tabs.length],
        ArrowLeft: tabs[(i - 1 + tabs.length) % tabs.length],
        Home: tabs[0],
        End: tabs.at(-1),
      }[e.key];
      if (!next) return;
      e.preventDefault();
      selectTab(next);
      next.focus();
    });
  }
}

// 1 and 2 pick the mode, like the [1] and [2] on the tabs, while focus is in the tool
// (so typing a number elsewhere on the page does nothing), and never in a text field.
document.getElementById("tool")?.addEventListener("keydown", (e) => {
  const tab = document.getElementById({ 1: "tab-scan", 2: "tab-compare" }[e.key] ?? "");
  if (!tab || e.altKey || e.ctrlKey || e.metaKey || e.repeat) return;
  const t = e.target;
  if (t.isContentEditable || t.closest?.("input:not([type=file]), textarea, select")) return; // a file picker takes no typing
  e.preventDefault();
  selectTab(tab);
  tab.focus();
});

// The read pass: a band sweeps the logo left to right and lights the pixels it
// covers, and the pupil follows it. Decoration only. It runs only while the logo
// is on screen, and not at all with reduced motion.

const SWEEP_MS = 1400;
const REST_MS = 7000;
const SVG = "http://www.w3.org/2000/svg";

function readPass(svg) {
  const ink = svg.querySelector(".i");
  const pupil = svg.querySelector(".p");
  if (!ink || !pupil) return;
  const { width, height } = svg.viewBox.baseVal;
  const clip = document.createElementNS(SVG, "clipPath");
  clip.id = "read-band";
  const band = document.createElementNS(SVG, "rect");
  for (const [k, v] of Object.entries({ x: -9, y: 0, width: 3, height })) band.setAttribute(k, v);
  clip.append(band);
  const lit = ink.cloneNode(true);
  lit.setAttribute("class", "lit");
  lit.setAttribute("clip-path", "url(#read-band)");
  svg.insertBefore(lit, pupil);
  svg.prepend(clip);

  const eye = 8; // the pupil's centre column in the jar
  const frame = (x) => {
    band.setAttribute("x", x);
    const shift = x < -3 ? 0 : Math.max(-2, Math.min(2, Math.round((x + 1.5 - eye) / 9)));
    pupil.setAttribute("transform", `translate(${shift} 0)`);
  };

  let raf = 0;
  let start = 0;
  const tick = (t) => {
    start ||= t;
    const p = (t - start) % (SWEEP_MS + REST_MS);
    frame(p < SWEEP_MS ? -4 + (p / SWEEP_MS) * (width + 5) : -9);
    raf = requestAnimationFrame(tick);
  };
  new IntersectionObserver(([e]) => {
    cancelAnimationFrame(raf);
    frame(-9);
    if (e.isIntersecting) {
      start = 0;
      raf = requestAnimationFrame(tick);
    }
  }).observe(svg);
}

if (!matchMedia("(prefers-reduced-motion: reduce)").matches) {
  document.querySelectorAll(".logo-lockup").forEach(readPass);
}
