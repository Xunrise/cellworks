// The install tabs, copy buttons, the ticket walkthrough and the key demos.
// No tracking, no network beyond this site's own pictures.
const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;

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

// ---------------------------------------------------------------- films
// A film is a run of frames the test harness rendered from the app itself:
// assets/anim/<name>/00.jpg ... Several names play one after the other.

const frameUrls = (el) => {
  if (el.dataset.src) return ["assets/" + el.dataset.src];
  const names = (el.dataset.film || "").split(" ").filter(Boolean);
  const counts = (el.dataset.n || "").split(" ").map(Number);
  return names.flatMap((name, k) =>
    Array.from({ length: counts[k] || 1 }, (_, i) => `assets/anim/${name}/${String(i).padStart(2, "0")}.jpg`));
};

const cache = new Map();
const preload = (urls) => urls.forEach((u) => {
  if (cache.has(u)) return;
  const img = new Image();
  img.decoding = "async";
  img.src = u;
  cache.set(u, img);
});

// Plays `urls` on `img` at `fps`; resolves when done or superseded.
const play = (img, urls, fps) => {
  const token = (img.playing = {});
  preload(urls);
  if (reduce || urls.length < 2) {
    img.src = urls[urls.length - 1];
    return Promise.resolve();
  }
  return new Promise((done) => {
    let i = 0;
    const tick = () => {
      if (img.playing !== token) return done();
      img.src = urls[i];
      if (++i < urls.length) setTimeout(tick, 1000 / fps);
      else done();
    };
    tick();
  });
};

// ---------------------------------------------------------------- key demos

const rows = [...document.querySelectorAll(".demo-row")];

rows.forEach((row) => {
  const img = row.querySelector(".demo img");
  const cap = row.querySelector(".cap");
  const base = img.getAttribute("src");
  const baseCap = cap.textContent;
  const keys = [...row.querySelectorAll(".key")];
  let current = null;

  let seq = 0;
  const press = async (key) => {
    const mine = ++seq;
    keys.forEach((k) => k.setAttribute("aria-pressed", String(k === key)));
    cap.textContent = key.dataset.caption;
    const fps = Number(key.dataset.fps) || 12;
    // Pressing what is shown again, or a reset key, runs the film back.
    if (key.hasAttribute("data-reset") || key === current) {
      if (current && !current.dataset.src) {
        await play(img, frameUrls(current).reverse(), (Number(current.dataset.fps) || 12) * 1.5);
        if (mine !== seq) return;
      }
      img.src = base;
      if (key === current) {
        key.setAttribute("aria-pressed", "false");
        cap.textContent = baseCap;
      }
      current = null;
      return;
    }
    current = key;
    await play(img, frameUrls(key), fps);
  };

  keys.forEach((k) => {
    k.setAttribute("aria-pressed", "false");
    k.addEventListener("click", () => press(k));
  });
  row.press = (name) => {
    const matching = keys.filter((k) => k.dataset.key === name);
    if (!matching.length) return false;
    // A key with several buttons (L) steps through them.
    const at = matching.indexOf(current);
    press(matching[(at + 1) % matching.length]);
    return true;
  };

  new IntersectionObserver((entries) => entries.forEach((e) => {
    if (e.isIntersecting) keys.forEach((k) => preload(frameUrls(k)));
  }), { rootMargin: "300px" }).observe(row);
});

// The row most in view takes the keyboard.
const inView = (el) => {
  const r = el.getBoundingClientRect();
  return Math.max(0, Math.min(r.bottom, innerHeight) - Math.max(r.top, 0)) / Math.min(r.height, innerHeight);
};
const activeRow = () => {
  const best = rows.map((r) => [r, inView(r)]).sort((a, b) => b[1] - a[1])[0];
  return best && best[1] > 0.5 ? best[0] : null;
};

document.addEventListener("keydown", (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  if (e.target.closest?.("input, textarea, [role=tab], .strip")) return;
  const name = e.key.toLowerCase();
  // Enter on a focused button clicks it; only take Enter from the page.
  if (name === "enter" && e.target.closest?.("button, a")) return;
  const row = e.target.closest?.(".demo-row") || activeRow();
  if (row?.press(name)) e.preventDefault();
});

// ---------------------------------------------------------------- the loop
// Without JavaScript, or with reduced motion, the steps stay a plain list.

const loop = document.querySelector(".loop");
if (loop && !reduce) {
  const steps = [...loop.querySelectorAll(".step")];
  const strip = document.createElement("div");
  strip.className = "strip";
  strip.setAttribute("role", "group");
  strip.setAttribute("aria-label", "Steps of the ticket");
  const live = document.createElement("p");
  live.className = "sr";
  live.setAttribute("aria-live", "polite");
  const bar = document.createElement("div");
  bar.className = "progress";
  bar.innerHTML = "<span></span>";
  const controls = document.createElement("div");
  controls.className = "controls";
  controls.innerHTML =
    '<button type="button" class="ctl" data-go="-1" aria-label="Previous step">‹</button>' +
    '<button type="button" class="ctl pause" aria-pressed="false">Pause</button>' +
    '<button type="button" class="ctl" data-go="1" aria-label="Next step">›</button>' +
    '<span class="count mono" aria-hidden="true"></span>';

  const buttons = steps.map((s, i) => {
    if (i) {
      const lane = document.createElement("span");
      lane.className = "lane";
      lane.setAttribute("aria-hidden", "true");
      strip.append(lane);
    }
    const b = document.createElement("button");
    b.type = "button";
    b.className = "stage";
    b.textContent = s.dataset.stage;
    b.addEventListener("click", () => show(i));
    b.addEventListener("keydown", (e) => {
      const to = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: steps.length - 1 }[e.key];
      if (to === undefined) return;
      e.preventDefault();
      show((to + steps.length) % steps.length);
      buttons[at].focus();
    });
    strip.append(b);
    return b;
  });

  const list = loop.querySelector(".steps");
  list.before(strip, bar, controls, live);
  loop.classList.add("live");

  const DWELL = 7000;
  let at = 0, timer = null, held = false, userPaused = false;
  const pauseBtn = controls.querySelector(".pause");
  const count = controls.querySelector(".count");

  const schedule = () => {
    clearTimeout(timer);
    bar.classList.remove("run");
    if (held || userPaused) return;
    void bar.offsetWidth;
    bar.style.setProperty("--dwell", DWELL + "ms");
    bar.classList.add("run");
    timer = setTimeout(() => show((at + 1) % steps.length), DWELL);
  };

  function show(i) {
    at = i;
    steps.forEach((s, k) => s.classList.toggle("on", k === i));
    buttons.forEach((b, k) => {
      b.classList.toggle("done", k < i);
      if (k === i) b.setAttribute("aria-current", "step");
      else b.removeAttribute("aria-current");
      b.tabIndex = k === i ? 0 : -1;
    });
    count.textContent = `${String(i + 1).padStart(2, "0")} / ${steps.length}`;
    live.textContent = `Step ${i + 1} of ${steps.length}: ${steps[i].querySelector("h3").textContent}`;
    const step = steps[i];
    const img = step.querySelector("img");
    const still = img.dataset.still || img.getAttribute("src");
    img.dataset.still = still;
    if (step.dataset.film) {
      play(img, frameUrls(step), Number(step.dataset.fps) || 10).then(() => {
        if (steps[at] === step) img.src = still;
      });
    }
    // Warm the next step.
    const next = steps[(i + 1) % steps.length];
    preload([next.querySelector("img").dataset.still || next.querySelector("img").getAttribute("src"), ...(next.dataset.film ? frameUrls(next) : [])]);
    schedule();
  }

  controls.addEventListener("click", (e) => {
    const go = e.target.closest("[data-go]");
    if (go) show((at + Number(go.dataset.go) + steps.length) % steps.length);
    if (e.target.closest(".pause")) {
      userPaused = !userPaused;
      pauseBtn.setAttribute("aria-pressed", String(userPaused));
      pauseBtn.textContent = userPaused ? "Play" : "Pause";
      schedule();
    }
  });
  // Hover or focus holds the step; leaving lets it run on.
  const hold = (on) => { held = on; schedule(); };
  loop.addEventListener("mouseenter", () => hold(true));
  loop.addEventListener("mouseleave", () => hold(false));
  loop.addEventListener("focusin", () => hold(true));
  loop.addEventListener("focusout", (e) => { if (!loop.contains(e.relatedTarget)) hold(false); });
  // Only run while the section is on screen.
  let visible = false;
  new IntersectionObserver(([e]) => {
    if (e.isIntersecting === visible) return;
    visible = e.isIntersecting;
    if (visible && steps[0].dataset.film) preload(frameUrls(steps[0]));
    held = !visible || loop.matches(":hover") || loop.contains(document.activeElement);
    schedule();
  }, { threshold: 0.25 }).observe(loop);

  held = true;
  show(0);
}
