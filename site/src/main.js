import "./style.css";
import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import Lenis from "lenis";
import { createVoiceField, microphoneLevel } from "./voice.js";

gsap.registerPlugin(ScrollTrigger);

const REPO = "https://github.com/sunderbharath85/tiny-whisper";
const RELEASES = `${REPO}/releases/latest`;
const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];

/* ---------- links & platform ---------- */
$$("[data-repo]").forEach((a) => (a.href = REPO));
$$("[data-releases], [data-release]").forEach((a) => {
  a.href = RELEASES;
  a.target = "_blank";
  a.rel = "noopener";
});
$$("[data-changelog]").forEach((a) => (a.href = `${REPO}/blob/main/CHANGELOG.md`));

const ua = (navigator.userAgentData?.platform || navigator.platform || navigator.userAgent).toLowerCase();
const os = ua.includes("mac") ? "mac" : ua.includes("win") ? "win" : ua.includes("linux") ? "linux" : null;
const OS_NAME = { mac: "macOS", win: "Windows", linux: "Linux" };
if (os) {
  $("[data-download]").textContent = `Download for ${OS_NAME[os]}`;
  $(`.platform[data-os="${os}"]`)?.classList.add("is-yours");
}

/* ---------- text splitting ---------- */
/** Wrap each word of `el` (keeping <em> etc.) in span.w. */
function splitWords(el) {
  const walk = (node, into) => {
    for (const child of [...node.childNodes]) {
      if (child.nodeType === Node.TEXT_NODE) {
        child.textContent.split(/(\s+)/).forEach((part) => {
          if (!part) return;
          if (/^\s+$/.test(part)) into.appendChild(document.createTextNode(" "));
          else {
            const w = document.createElement("span");
            w.className = "w";
            w.textContent = part;
            into.appendChild(w);
          }
        });
        child.remove();
      } else if (child.nodeType === Node.ELEMENT_NODE) {
        const clone = child.cloneNode(false);
        walk(child, clone);
        child.replaceWith(clone);
        into.appendChild(clone);
      }
    }
  };
  const frag = document.createElement("div");
  walk(el, frag);
  el.append(...frag.childNodes);
  return $$(".w", el);
}
/** For headline lines: each word (or <em>) becomes one sliding block. */
function splitLines(el) {
  return $$(".line", el).flatMap((line) => {
    const words = [];
    for (const child of [...line.childNodes]) {
      if (child.nodeType === Node.TEXT_NODE) {
        child.textContent.split(/(\s+)/).forEach((part) => {
          if (!part) return;
          if (/^\s+$/.test(part)) line.insertBefore(document.createTextNode(" "), child);
          else {
            const w = document.createElement("span");
            w.className = "w";
            w.textContent = part;
            line.insertBefore(w, child);
            words.push(w);
          }
        });
        child.remove();
      } else {
        const w = document.createElement("span");
        w.className = "w";
        line.insertBefore(w, child);
        w.appendChild(child);
        words.push(w);
      }
    }
    return words;
  });
}

/* ---------- smooth scroll ---------- */
let lenis = null;
if (!reduce) {
  lenis = new Lenis({ lerp: 0.09, smoothWheel: true });
  lenis.on("scroll", ScrollTrigger.update);
  gsap.ticker.add((t) => lenis.raf(t * 1000));
  gsap.ticker.lagSmoothing(0);
}
$$('a[href^="#"]').forEach((a) =>
  a.addEventListener("click", (e) => {
    const id = a.getAttribute("href");
    if (id.length < 2) return;
    e.preventDefault();
    lenis ? lenis.scrollTo(id, { duration: 1.6 }) : $(id)?.scrollIntoView();
  })
);

/* ---------- voice field ---------- */
const field = createVoiceField($(".field"));

/* ---------- intro ---------- */
const heroWords = splitLines($(".hero__title"));
const downloadWords = splitLines($(".download__title"));
gsap.set(heroWords, { yPercent: 110 });
document.body.classList.add("loading");
lenis?.stop();

const intro = gsap.timeline({ defaults: { ease: "expo.out" } });
const counter = { v: 0 };
intro
  .to(counter, {
    v: 100,
    duration: reduce ? 0.1 : 1.5,
    ease: "power2.inOut",
    onUpdate: () => ($("[data-count]").textContent = Math.round(counter.v)),
  })
  .to(".loader", { yPercent: -100, duration: reduce ? 0.1 : 1.1, ease: "expo.inOut" })
  .from(".field", { opacity: 0, scale: 1.15, duration: 2.2 }, "<0.3")
  .to(heroWords, { yPercent: 0, duration: 1.4, stagger: 0.08 }, "<0.2")
  .from(".reveal-up", { y: 30, opacity: 0, duration: 1.2, stagger: 0.1 }, "<0.4")
  .from(".nav", { y: -20, opacity: 0, duration: 1 }, "<")
  .add(() => {
    document.body.classList.remove("loading");
    $(".loader").remove();
    lenis?.start();
  }, "<");

/* ---------- hero: parallax out ---------- */
gsap.to(".hero__title", {
  yPercent: -18,
  opacity: 0.2,
  ease: "none",
  scrollTrigger: { trigger: ".hero", start: "top top", end: "bottom top", scrub: true },
});

/* ---------- voice → line ---------- */
const TYPED = "Hey Priya — I'm moving standup to 10:30 so the Berlin team can join. I'll send the revised estimate before Friday.";
const typed = $("[data-typed]");
const docState = $("[data-docstate]");
const lines = $$("[data-morph-line]");
const morph = gsap.timeline({
  scrollTrigger: {
    trigger: ".morph",
    start: "top top",
    end: "bottom bottom",
    scrub: 1,
    pin: ".morph__pin",
    pinSpacing: false,
  },
});
morph
  .to(field.state, { morph: 1, duration: 0.35, ease: "power2.inOut" }, 0)
  .to(".field", { yPercent: 30, duration: 0.35, ease: "power2.inOut" }, 0)
  .to(".doc", { y: 0, rotate: 0, opacity: 1, duration: 0.25, ease: "power3.out" }, 0.12)
  .to({ p: 0 }, {
    p: 1,
    duration: 0.55,
    ease: "none",
    onUpdate() {
      const p = this.targets()[0].p;
      typed.textContent = TYPED.slice(0, Math.round(TYPED.length * p));
      docState.textContent = p >= 1 ? "Pasted ✓" : p > 0 ? "Transcribing…" : "Listening…";
    },
  }, 0.3)
  .to({}, { duration: 0.15 });
// Light the headline one line at a time as the morph progresses.
ScrollTrigger.create({
  trigger: ".morph",
  start: "top top",
  end: "bottom bottom",
  onUpdate: (s) => lines.forEach((l, i) => l.classList.toggle("is-lit", s.progress > 0.02 + i * 0.14)),
});
// Fade the field out as the story moves on; stop drawing once invisible.
gsap.to(".field", {
  opacity: 0,
  ease: "none",
  scrollTrigger: {
    trigger: ".manifesto",
    start: "top 80%",
    end: "top 10%",
    scrub: true,
    onUpdate: (s) => (field.state.visible = s.progress < 1),
  },
});

/* ---------- manifesto: words light up ---------- */
const mWords = splitWords($("[data-words]"));
gsap.to(mWords, {
  opacity: 1,
  stagger: 0.1,
  ease: "none",
  scrollTrigger: { trigger: ".manifesto", start: "top 65%", end: "bottom 70%", scrub: true },
});

/* ---------- section heads ---------- */
$$(".section-title").forEach((t) => {
  const ws = splitWords(t);
  gsap.from(ws, {
    yPercent: 60,
    opacity: 0,
    rotate: 3,
    duration: 1.1,
    stagger: 0.06,
    ease: "expo.out",
    scrollTrigger: { trigger: t, start: "top 85%" },
  });
});

/* ---------- features ---------- */
gsap.from(".card", {
  y: 80,
  opacity: 0,
  duration: 1.2,
  stagger: 0.1,
  ease: "expo.out",
  scrollTrigger: { trigger: ".bento", start: "top 80%" },
});
$$(".card").forEach((c) =>
  c.addEventListener("pointermove", (e) => {
    const r = c.getBoundingClientRect();
    c.style.setProperty("--mx", `${e.clientX - r.left}px`);
    c.style.setProperty("--my", `${e.clientY - r.top}px`);
  })
);
// Keys press, then the phrase lands in the next app.
const chips = $$(".apps span");
const bigkeys = $$(".bigkeys kbd");
let hot = 0;
gsap.timeline({ repeat: -1, repeatDelay: 0.6, scrollTrigger: { trigger: ".card--dictate", start: "top bottom", toggleActions: "play pause resume pause" } })
  .add(() => bigkeys.forEach((k) => k.classList.add("is-down")))
  .add(() => bigkeys.forEach((k) => k.classList.remove("is-down")), 0.35)
  .add(() => {
    chips.forEach((c) => c.classList.remove("is-hot"));
    chips[hot++ % chips.length].classList.add("is-hot");
  }, 0.5)
  .to({}, { duration: 1.2 });
gsap.from(".speakers__bar i", {
  scaleX: 0,
  duration: 1.4,
  stagger: 0.15,
  ease: "expo.out",
  scrollTrigger: { trigger: ".card--sessions", start: "top 75%" },
});
gsap.from(".history li", {
  x: -20,
  opacity: 0,
  stagger: 0.12,
  duration: 0.9,
  ease: "expo.out",
  scrollTrigger: { trigger: ".card--history", start: "top 75%" },
});
const zero = { v: 4096 };
gsap.to(zero, {
  v: 0,
  duration: 2.4,
  ease: "expo.out",
  onUpdate: () => ($("[data-zero]").textContent = Math.round(zero.v).toLocaleString()),
  scrollTrigger: { trigger: ".card--zero", start: "top 75%" },
});

/* ---------- showcase ---------- */
gsap.fromTo(
  ".window",
  { rotateX: 32, scale: 0.82, y: 60 },
  { rotateX: 0, scale: 1, y: 0, ease: "none", scrollTrigger: { trigger: ".showcase", start: "top 85%", end: "center 55%", scrub: true } }
);
const track = $(".screens__track");
gsap.to(track, {
  x: () => -(track.scrollWidth - innerWidth),
  ease: "none",
  scrollTrigger: {
    trigger: ".screens",
    start: "center center",
    end: () => `+=${track.scrollWidth - innerWidth}`,
    scrub: 1,
    pin: true,
    invalidateOnRefresh: true,
  },
});

/* ---------- models ---------- */
const MODELS = [
  ["Tiny", "Instant, forgiving of slow machines.", 5, 2, "75 MB"],
  ["Base", "The everyday choice — fast on any laptop.", 4, 3, "142 MB", "Recommended"],
  ["Small", "Better with names and jargon.", 3, 4, "466 MB"],
  ["Medium", "High accuracy; best with a GPU.", 2, 4, "1.5 GB"],
  ["Large v3", "Most accurate Whisper, 99 languages.", 1, 5, "2.9 GB"],
  ["Parakeet CTC", "NVIDIA's sharp English model.", 4, 4, "2.4 GB"],
  ["Parakeet TDT", "25 languages, punctuation built in.", 4, 5, "2.5 GB"],
  ["Sortformer", "Tells up to four speakers apart.", 4, 4, "250 MB", "Sessions"],
];
const meter = (label, n) =>
  `<span class="meter"><span>${label}</span>${[1, 2, 3, 4, 5].map((i) => `<i class="${i <= n ? "on" : ""}"></i>`).join("")}</span>`;
$("[data-models]").innerHTML = MODELS.map(
  ([name, desc, speed, acc, size, tag]) => `
  <li class="mrow">
    <span class="mrow__name">${name}${tag ? `<span class="tag">${tag}</span>` : ""}</span>
    <span class="mrow__desc">${desc}</span>
    <span class="mrow__meters">${meter("Speed", speed)}${meter("Accuracy", acc)}</span>
    <span class="mrow__size">${size}</span>
  </li>`
).join("");
gsap.from(".mrow", {
  y: 40,
  opacity: 0,
  duration: 1,
  stagger: 0.07,
  ease: "expo.out",
  scrollTrigger: { trigger: ".mlist", start: "top 80%" },
});

/* ---------- download & footer ---------- */
gsap.set(downloadWords, { yPercent: 110 });
gsap.to(downloadWords, {
  yPercent: 0,
  duration: 1.4,
  stagger: 0.08,
  ease: "expo.out",
  scrollTrigger: { trigger: ".download", start: "top 70%" },
});
gsap.from(".platform", {
  y: 50,
  opacity: 0,
  duration: 1.1,
  stagger: 0.08,
  ease: "expo.out",
  scrollTrigger: { trigger: ".platforms", start: "top 88%" },
});
gsap.from(".footer__mark", {
  yPercent: 60,
  ease: "none",
  scrollTrigger: { trigger: ".footer", start: "top bottom", end: "bottom bottom", scrub: true },
});

/* ---------- cursor & magnetic buttons ---------- */
if (matchMedia("(hover: hover)").matches && !reduce) {
  const cur = $(".cursor");
  const xTo = gsap.quickTo(cur, "x", { duration: 0.35, ease: "power3" });
  const yTo = gsap.quickTo(cur, "y", { duration: 0.35, ease: "power3" });
  addEventListener("pointermove", (e) => { cur.classList.add("is-active"); xTo(e.clientX); yTo(e.clientY); });
  $$("a, button, .mrow").forEach((el) => {
    el.addEventListener("pointerenter", () => cur.classList.add("is-big"));
    el.addEventListener("pointerleave", () => cur.classList.remove("is-big"));
  });
  $$(".magnetic").forEach((el) => {
    el.addEventListener("pointermove", (e) => {
      const r = el.getBoundingClientRect();
      gsap.to(el, { x: (e.clientX - r.left - r.width / 2) * 0.3, y: (e.clientY - r.top - r.height / 2) * 0.35, duration: 0.6, ease: "power3" });
    });
    el.addEventListener("pointerleave", () => gsap.to(el, { x: 0, y: 0, duration: 0.9, ease: "elastic.out(1, .4)" }));
  });
}

/* ---------- talk to the page ---------- */
const micBtn = $("[data-mic]");
const micLabel = $("[data-mic-label]");
let mic = null;
micBtn.addEventListener("click", async () => {
  if (mic) {
    mic.stop();
    mic = null;
    field.setLevelSource(null);
    micBtn.classList.remove("is-on");
    micLabel.textContent = "Talk to this page";
    return;
  }
  try {
    mic = await microphoneLevel();
    field.setLevelSource(mic.level);
    micBtn.classList.add("is-on");
    micLabel.textContent = "Listening — say something";
    $("[data-mic-note]").hidden = false;
  } catch {
    micLabel.textContent = "Microphone unavailable";
  }
});

addEventListener("load", () => ScrollTrigger.refresh());
