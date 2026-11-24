import { useEffect, useRef } from "react";
import type { Mood } from "@/lib/catalog";

/// A ring of radial strokes that breathes when idle, stirs when listening,
/// swells with noise while you speak and sweeps while the model writes.
/// Pure canvas; reads colours from CSS variables so it follows the theme.
export function VoiceField({ mood, size = 340 }: { mood: Mood; size?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const moodRef = useRef(mood);
  moodRef.current = mood;

  useEffect(() => {
    const canvas = ref.current!;
    const ctx = canvas.getContext("2d")!;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = size * dpr;
    canvas.height = size * dpr;
    ctx.scale(dpr, dpr);

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const N = 144;
    const noise = Array.from({ length: N }, () => Math.random() * Math.PI * 2);
    // Smoothed parameters, eased toward the current mood's targets.
    const cur = { amp: 0.05, energy: 0, sweep: 0, warm: 0, alpha: 0.5 };
    let t = 0;
    let raf = 0;

    const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

    function targets(m: Mood) {
      switch (m) {
        case "listening": return { amp: 0.12, energy: 0.15, sweep: 0, warm: 1, alpha: 0.85 };
        case "speaking": return { amp: 0.34, energy: 1, sweep: 0, warm: 1, alpha: 1 };
        case "thinking": return { amp: 0.1, energy: 0.05, sweep: 1, warm: 0.4, alpha: 0.8 };
        case "recording": return { amp: 0.2, energy: 0.5, sweep: 0, warm: 1, alpha: 1 };
        case "error": return { amp: 0.06, energy: 0, sweep: 0, warm: 0, alpha: 0.6 };
        default: return { amp: 0.05, energy: 0, sweep: 0, warm: 0, alpha: 0.55 };
      }
    }

    function frame() {
      const m = moodRef.current;
      const tg = targets(m);
      for (const k of Object.keys(cur) as (keyof typeof cur)[]) cur[k] += (tg[k] - cur[k]) * 0.06;
      t += reduce ? 0 : 1 / 60;

      const ink = css("--ink");
      const ember = css(m === "error" ? "--danger" : "--ember");
      const c = size / 2;
      const inner = size * 0.27;
      const span = size * 0.17;

      ctx.clearRect(0, 0, size, size);

      // Soft glow behind the ring when warm.
      if (cur.warm > 0.02) {
        const g = ctx.createRadialGradient(c, c, inner * 0.4, c, c, size * 0.5);
        g.addColorStop(0, "transparent");
        g.addColorStop(0.55, withAlpha(ember, 0.08 * cur.warm * (0.7 + cur.energy * 0.6)));
        g.addColorStop(1, "transparent");
        ctx.fillStyle = g;
        ctx.fillRect(0, 0, size, size);
      }

      ctx.lineCap = "round";
      for (let i = 0; i < N; i++) {
        const a = (i / N) * Math.PI * 2 - Math.PI / 2;
        const n1 = Math.sin(t * 1.3 + noise[i]) * 0.5 + 0.5;
        const n2 = Math.sin(t * 7.1 + noise[i] * 3 + i * 0.4) * 0.5 + 0.5;
        const wave = Math.sin(a * 3 + t * 0.8) * 0.5 + 0.5;
        let len = 0.08 + cur.amp * (0.35 + wave * 0.65) + cur.energy * n2 * 0.55 * (0.4 + n1);
        // Thinking: a bright comet sweeps around.
        let hi = 0;
        if (cur.sweep > 0.01) {
          const head = (t * 1.6) % (Math.PI * 2);
          let d = (a + Math.PI / 2 - head) % (Math.PI * 2);
          if (d < 0) d += Math.PI * 2;
          hi = Math.max(0, 1 - d / 1.6) * cur.sweep;
          len += hi * 0.35;
        }
        const r0 = inner;
        const r1 = inner + span * len;
        ctx.strokeStyle = mix(ink, ember, Math.min(1, cur.warm * (0.35 + n1 * 0.65) + hi));
        ctx.globalAlpha = cur.alpha * (0.35 + 0.65 * Math.max(n1 * 0.6, hi, cur.energy * n2));
        ctx.lineWidth = 1.4;
        ctx.beginPath();
        ctx.moveTo(c + Math.cos(a) * r0, c + Math.sin(a) * r0);
        ctx.lineTo(c + Math.cos(a) * r1, c + Math.sin(a) * r1);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;

      // Fine outer orbit.
      ctx.strokeStyle = withAlpha(ink, 0.08);
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.arc(c, c, inner + span * 1.15, 0, Math.PI * 2);
      ctx.stroke();

      raf = requestAnimationFrame(frame);
    }
    raf = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(raf);
  }, [size]);

  return <canvas ref={ref} style={{ width: size, height: size }} aria-hidden="true" />;
}

type RGB = [number, number, number];
const probe = typeof document !== "undefined" ? document.createElement("canvas").getContext("2d", { willReadFrequently: true }) : null;
const cache = new Map<string, RGB>();

/// Resolve any CSS colour (oklch included) to RGB by painting one pixel.
function rgb(color: string): RGB {
  const hit = cache.get(color);
  if (hit) return hit;
  let out: RGB = [128, 128, 128];
  if (probe) {
    probe.clearRect(0, 0, 1, 1);
    probe.fillStyle = "#808080";
    probe.fillStyle = color;
    probe.fillRect(0, 0, 1, 1);
    const d = probe.getImageData(0, 0, 1, 1).data;
    out = [d[0], d[1], d[2]];
  }
  cache.set(color, out);
  return out;
}
function mix(a: string, b: string, t: number) {
  const x = rgb(a), y = rgb(b);
  const c = x.map((v, i) => Math.round(v + (y[i] - v) * t));
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}
function withAlpha(c: string, a: number) {
  const [r, g, b] = rgb(c);
  return `rgba(${r},${g},${b},${a})`;
}
