// The install tabs and copy buttons. No tracking, no network.
const tabs = [...document.querySelectorAll("[role=tab]")];
const select = (tab) => {
  tabs.forEach((t) => {
    const on = t === tab;
    t.setAttribute("aria-selected", String(on));
    t.tabIndex = on ? 0 : -1;
    document.getElementById(t.getAttribute("aria-controls")).hidden = !on;
  });
  tab.focus();
};
tabs.forEach((t, i) => {
  t.addEventListener("click", () => select(t));
  t.addEventListener("keydown", (e) => {
    const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
    if (step) select(tabs[(i + step + tabs.length) % tabs.length]);
  });
});

document.querySelectorAll(".copy").forEach((b) => b.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(b.previousElementSibling.textContent);
    b.textContent = "Copied";
  } catch {
    b.textContent = "Select and copy";
  }
  setTimeout(() => (b.textContent = "Copy"), 1600);
}));
