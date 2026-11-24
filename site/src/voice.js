import * as THREE from "three";

/**
 * The hero "voice field": particles on a sphere, displaced by noise that swells
 * with a voice envelope. `morph` (0→1, driven by scroll) flattens the sphere
 * into a single waveform line — a voice becoming a line of text.
 */

const NOISE = /* glsl */ `
vec3 mod289(vec3 x){return x-floor(x*(1./289.))*289.;}
vec4 mod289(vec4 x){return x-floor(x*(1./289.))*289.;}
vec4 permute(vec4 x){return mod289(((x*34.)+1.)*x);}
vec4 taylorInvSqrt(vec4 r){return 1.79284291400159-0.85373472095314*r;}
float snoise(vec3 v){
  const vec2 C=vec2(1./6.,1./3.);const vec4 D=vec4(0.,.5,1.,2.);
  vec3 i=floor(v+dot(v,C.yyy));vec3 x0=v-i+dot(i,C.xxx);
  vec3 g=step(x0.yzx,x0.xyz);vec3 l=1.-g;vec3 i1=min(g.xyz,l.zxy);vec3 i2=max(g.xyz,l.zxy);
  vec3 x1=x0-i1+C.xxx;vec3 x2=x0-i2+C.yyy;vec3 x3=x0-D.yyy;i=mod289(i);
  vec4 p=permute(permute(permute(i.z+vec4(0.,i1.z,i2.z,1.))+i.y+vec4(0.,i1.y,i2.y,1.))+i.x+vec4(0.,i1.x,i2.x,1.));
  float n_=.142857142857;vec3 ns=n_*D.wyz-D.xzx;vec4 j=p-49.*floor(p*ns.z*ns.z);
  vec4 x_=floor(j*ns.z);vec4 y_=floor(j-7.*x_);vec4 x=x_*ns.x+ns.yyyy;vec4 y=y_*ns.x+ns.yyyy;
  vec4 h=1.-abs(x)-abs(y);vec4 b0=vec4(x.xy,y.xy);vec4 b1=vec4(x.zw,y.zw);
  vec4 s0=floor(b0)*2.+1.;vec4 s1=floor(b1)*2.+1.;vec4 sh=-step(h,vec4(0.));
  vec4 a0=b0.xzyw+s0.xzyw*sh.xxyy;vec4 a1=b1.xzyw+s1.xzyw*sh.zzww;
  vec3 p0=vec3(a0.xy,h.x);vec3 p1=vec3(a0.zw,h.y);vec3 p2=vec3(a1.xy,h.z);vec3 p3=vec3(a1.zw,h.w);
  vec4 norm=taylorInvSqrt(vec4(dot(p0,p0),dot(p1,p1),dot(p2,p2),dot(p3,p3)));
  p0*=norm.x;p1*=norm.y;p2*=norm.z;p3*=norm.w;
  vec4 m=max(.6-vec4(dot(x0,x0),dot(x1,x1),dot(x2,x2),dot(x3,x3)),0.);m=m*m;
  return 42.*dot(m*m,vec4(dot(p0,x0),dot(p1,x1),dot(p2,x2),dot(p3,x3)));
}`;

const vertex = /* glsl */ `
uniform float uTime;
uniform float uVoice;
uniform float uMorph;
uniform float uPixel;
uniform vec2 uMouse;
attribute vec3 aLine;
attribute float aSeed;
varying float vHeat;
varying float vAlpha;
${NOISE}
void main(){
  // Sphere state: push each point out along its normal by noise × voice.
  vec3 n = normalize(position);
  float slow = snoise(n * 1.4 + vec3(uTime * .12));
  float fast = snoise(n * 3.2 + vec3(uTime * .9, 0., uTime * .4));
  float d = slow * .12 + fast * (.04 + uVoice * .55);
  vec3 sphere = n * (1.6 + d);

  // Line state: a waveform across the screen whose height follows the voice.
  float x = aLine.x;
  float env = exp(-pow(x * .32, 2.));
  float wave = sin(x * 3.1 + uTime * 2.4) * .35 + sin(x * 7.3 - uTime * 3.7) * .18 + snoise(vec3(x * 1.6, uTime * .8, aSeed)) * .3;
  vec3 line = vec3(x, wave * env * (.25 + uVoice * 1.6) + aLine.y, aLine.z);

  float m = smoothstep(0., 1., clamp(uMorph * 1.25 - aSeed * .25, 0., 1.));
  vec3 p = mix(sphere, line, m);

  // A little parallax toward the pointer.
  p.xy += uMouse * .12 * (1. - m);

  vec4 mv = modelViewMatrix * vec4(p, 1.);
  gl_Position = projectionMatrix * mv;
  gl_PointSize = uPixel * (1.4 + aSeed * 1.6 + uVoice * 1.2) * (5.5 / -mv.z);

  vHeat = clamp(mix(d * 2.6 + uVoice * .5, abs(wave) * env * (.4 + uVoice), m) + .1, 0., 1.);
  vAlpha = .35 + .65 * aSeed;
}`;

const fragment = /* glsl */ `
uniform vec3 uInk;
uniform vec3 uEmber;
varying float vHeat;
varying float vAlpha;
void main(){
  vec2 c = gl_PointCoord - .5;
  float r = length(c);
  if (r > .5) discard;
  float soft = smoothstep(.5, .0, r);
  vec3 col = mix(uInk, uEmber, smoothstep(.15, .75, vHeat));
  gl_FragColor = vec4(col, soft * vAlpha);
}`;

export function createVoiceField(canvas) {
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const small = innerWidth < 760;
  const COUNT = small ? 9000 : 22000;

  const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, alpha: true, powerPreference: "high-performance" });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(35, 1, 0.1, 100);
  camera.position.set(0, 0, 7.2);

  // Fibonacci sphere for even spacing; a jittered line for the morph target.
  const pos = new Float32Array(COUNT * 3);
  const line = new Float32Array(COUNT * 3);
  const seed = new Float32Array(COUNT);
  const golden = Math.PI * (3 - Math.sqrt(5));
  for (let i = 0; i < COUNT; i++) {
    const y = 1 - (i / (COUNT - 1)) * 2;
    const r = Math.sqrt(1 - y * y);
    const th = golden * i;
    pos.set([Math.cos(th) * r, y, Math.sin(th) * r], i * 3);
    const t = Math.random();
    line.set([(t - 0.5) * 13, (Math.random() - 0.5) * 0.05, (Math.random() - 0.5) * 0.4], i * 3);
    seed[i] = Math.random();
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setAttribute("aLine", new THREE.BufferAttribute(line, 3));
  geo.setAttribute("aSeed", new THREE.BufferAttribute(seed, 1));

  const uniforms = {
    uTime: { value: 0 },
    uVoice: { value: 0 },
    uMorph: { value: 0 },
    uPixel: { value: renderer.getPixelRatio() },
    uMouse: { value: new THREE.Vector2() },
    uInk: { value: new THREE.Color("#efe9df") },
    uEmber: { value: new THREE.Color("#ff6a2b") },
  };
  const mat = new THREE.ShaderMaterial({
    vertexShader: vertex,
    fragmentShader: fragment,
    uniforms,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  });
  const points = new THREE.Points(geo, mat);
  scene.add(points);

  function resize() {
    const w = canvas.clientWidth, h = canvas.clientHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    // Keep the sphere a sensible size on narrow screens.
    camera.position.z = w / h < 0.9 ? 10.5 : 7.2;
    camera.updateProjectionMatrix();
  }
  resize();
  addEventListener("resize", resize);

  const mouse = new THREE.Vector2();
  addEventListener("pointermove", (e) => {
    mouse.set((e.clientX / innerWidth) * 2 - 1, -((e.clientY / innerHeight) * 2 - 1));
  });

  // Voice envelope: a simulated speaker (syllables in phrases with pauses),
  // replaced by the real microphone level when the visitor opts in.
  let mic = null;
  let target = 0;
  let nextChange = 0;
  let speaking = false;
  function simulated(t) {
    if (t > nextChange) {
      speaking = !speaking;
      nextChange = t + (speaking ? 1.6 + Math.random() * 2.2 : 0.9 + Math.random() * 1.4);
    }
    if (!speaking) return 0.02;
    const syll = Math.max(0, Math.sin(t * 9.5 + Math.sin(t * 2.3) * 2));
    return 0.25 + syll * 0.55;
  }

  const state = { morph: 0, visible: true };
  const clock = new THREE.Clock();
  function frame() {
    const t = clock.getElapsedTime();
    if (state.visible) {
      target = mic ? mic() : simulated(t);
      uniforms.uVoice.value += (target - uniforms.uVoice.value) * (target > uniforms.uVoice.value ? 0.25 : 0.06);
      uniforms.uTime.value = reduce ? 4 : t;
      uniforms.uMorph.value = state.morph;
      uniforms.uMouse.value.lerp(mouse, 0.05);
      points.rotation.y = reduce ? 0 : t * 0.06 * (1 - state.morph);
      points.rotation.x = Math.sin(t * 0.2) * 0.1 * (1 - state.morph);
      renderer.render(scene, camera);
    }
    requestAnimationFrame(frame);
  }
  frame();

  return {
    state,
    /** Use a live level source (0–1) instead of the simulated voice. */
    setLevelSource(fn) { mic = fn; },
    get voice() { return uniforms.uVoice.value; },
  };
}

/** Ask for the microphone and return a 0–1 level reader. Audio is only measured, never stored or sent. */
export async function microphoneLevel() {
  const stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } });
  const ctx = new AudioContext();
  const src = ctx.createMediaStreamSource(stream);
  const an = ctx.createAnalyser();
  an.fftSize = 1024;
  src.connect(an);
  const buf = new Float32Array(an.fftSize);
  return {
    level() {
      an.getFloatTimeDomainData(buf);
      let s = 0;
      for (let i = 0; i < buf.length; i++) s += buf[i] * buf[i];
      return Math.min(1, Math.sqrt(s / buf.length) * 9);
    },
    stop() {
      stream.getTracks().forEach((t) => t.stop());
      ctx.close();
    },
  };
}
