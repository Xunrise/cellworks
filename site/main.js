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

// Whether a player runs: an explicit pause wins; Play overrides a hover or
// focus hold until the pointer or focus leaves; off screen it rests.
const runs = (s) => !s.paused && s.visible && (s.forced || !(s.hover || s.focus));

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

// Plays `urls` on `img` at `fps`; resolves when done or superseded. With
// `loop` it starts over after the last frame until superseded: a steady
// state's loop is filmed to close on its first frame. While `img.paused` is
// set it waits on the frame it shows.
const play = (img, urls, fps, loop = false) => {
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
      if (img.paused) return setTimeout(tick, 200);
      img.src = urls[i];
      if (++i < urls.length) setTimeout(tick, 1000 / fps);
      else if (loop) {
        i = 0;
        setTimeout(tick, 1000 / fps);
      } else done();
    };
    tick();
  });
};

// ---------------------------------------------------------------- the hero
// The overview city, looping. The poster is frame 0; the rest load after the
// page and the loop starts once they are decoded.

const hero = document.querySelector(".hero-shot img[data-film]");
const heroBtn = document.querySelector(".hero-pause");
if (hero && !reduce) {
  const urls = frameUrls(hero);
  // Hover does not hold the hero; only Pause and leaving the screen do.
  const hs = { paused: false, hover: false, focus: false, visible: true, forced: true };
  let i = 0, ready = false;
  addEventListener("load", () => {
    preload(urls);
    Promise.all(urls.map((u) => cache.get(u).decode().catch(() => {}))).then(() => (ready = true));
  });
  setInterval(() => {
    if (!ready || document.hidden || !runs(hs)) return;
    i = (i + 1) % urls.length;
    hero.src = urls[i];
  }, 1000 / (Number(hero.dataset.fps) || 10));
  new IntersectionObserver(([e]) => (hs.visible = e.isIntersecting)).observe(hero);
  heroBtn.addEventListener("click", () => {
    hs.paused = !hs.paused;
    heroBtn.setAttribute("aria-pressed", String(hs.paused));
    heroBtn.textContent = hs.paused ? "Play" : "Pause";
  });
} else if (heroBtn) {
  heroBtn.remove();
}

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
    // The change once, then its steady state (lanes with their traffic)
    // for as long as it is picked.
    await play(img, frameUrls(key), fps);
    if (key.dataset.steady && mine === seq) {
      const loop = { dataset: { film: key.dataset.steady, n: key.dataset.steadyN } };
      play(img, frameUrls(loop), 10, true);
    }
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
    if (!e.isIntersecting) return;
    keys.forEach((k) => {
      preload(frameUrls(k));
      if (k.dataset.steady) preload(frameUrls({ dataset: { film: k.dataset.steady, n: k.dataset.steadyN } }));
    });
  }), { rootMargin: "300px" }).observe(row);
  // A film rests only while its picture is wholly off screen.
  new IntersectionObserver(([e]) => (img.paused = !e.isIntersecting)).observe(img);
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
  let at = 0, timer = null, left = DWELL, since = 0;
  const st = { paused: false, hover: false, focus: false, visible: false, forced: false };
  const pauseBtn = controls.querySelector(".pause");
  const count = controls.querySelector(".count");

  // Runs the step timer and the progress bar while `runs(st)` holds, keeping
  // what is left of the step across a pause.
  const sync = () => {
    clearTimeout(timer);
    const go = runs(st);
    // Pause holds the step's film too; scrolling never does.
    steps.forEach((s) => (s.querySelector("img").paused = st.paused || !st.visible));
    bar.classList.toggle("held", !go);
    if (go) {
      since = Date.now();
      timer = setTimeout(() => show((at + 1) % steps.length), left);
    } else if (since) {
      left -= Date.now() - since;
      since = 0;
    }
  };

  function show(i) {
    at = i;
    // The film of the step left stops; a step shown again plays from the start.
    steps.forEach((s, k) => {
      if (k !== i) s.querySelector("img").playing = {};
    });
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
    if (step.dataset.film && !st.paused) {
      play(img, frameUrls(step), Number(step.dataset.fps) || 10).then(() => {
        if (steps[at] === step) img.src = still;
      });
    }
    // Warm the next step.
    const next = steps[(i + 1) % steps.length];
    preload([next.querySelector("img").dataset.still || next.querySelector("img").getAttribute("src"), ...(next.dataset.film ? frameUrls(next) : [])]);
    // A new step starts its bar from empty.
    left = DWELL;
    since = 0;
    bar.style.setProperty("--dwell", DWELL + "ms");
    bar.classList.remove("run");
    void bar.offsetWidth;
    bar.classList.add("run");
    sync();
  }

  controls.addEventListener("click", (e) => {
    const go = e.target.closest("[data-go]");
    if (go) show((at + Number(go.dataset.go) + steps.length) % steps.length);
    if (e.target.closest(".pause")) {
      st.paused = !st.paused;
      // Play overrides the hover or focus hold, or it would look inert.
      st.forced = !st.paused;
      pauseBtn.setAttribute("aria-pressed", String(st.paused));
      pauseBtn.textContent = st.paused ? "Play" : "Pause";
      sync();
    }
  });
  // Hovering the stage strip or the controls holds the step; the rest of the
  // section passes under the pointer while scrolling, so it does not.
  [strip, controls].forEach((el) => {
    el.addEventListener("mouseenter", () => { st.hover = true; sync(); });
    el.addEventListener("mouseleave", () => { st.hover = st.forced = false; sync(); });
  });
  loop.addEventListener("focusin", () => { st.focus = true; sync(); });
  loop.addEventListener("focusout", (e) => {
    if (!loop.contains(e.relatedTarget)) { st.focus = st.forced = false; sync(); }
  });
  // Only run while the section is on screen.
  // It rests only once (almost) all of it has scrolled away, and picks up
  // the same step and progress when it is back.
  new IntersectionObserver(([e]) => {
    const visible = e.intersectionRatio >= 0.1;
    if (visible === st.visible) return;
    st.visible = visible;
    if (visible && steps[0].dataset.film) preload(frameUrls(steps[0]));
    sync();
  }, { threshold: [0, 0.1] }).observe(loop);

  show(0);
}
