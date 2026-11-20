// Section 05, "Getting there". Three self-playing scenes:
//   fig-journey-flight  (2D) relativistic constant-acceleration voyage with ship-time vs Earth-time bars
//   fig-journey-cockpit (3D) the view ahead at relativistic speed: exact aberration + Doppler, interstellar headwind
//   fig-journey-map     (3D) the Milky Way and neighbours, morphing between true distance and ship travel time
// Physics in cgs unless a comment says otherwise.
(function () {
  'use strict';
  const C = P.C;
  const D = 1.74e3 * C.pc;                 // Earth to Pillars, cm (facts.md canonical 1.74 kpc = 5,675 ly)
  const TNT = 4.184e16;                    // erg per tonne of TNT
  const MP = 1.6726e-24;                   // proton mass, g
  const EV = 1.602e-12;                    // erg per eV
  const N_ISM = 1;                         // hydrogen atoms per cm^3, typical diffuse interstellar medium (order of magnitude)
  const MONO = '"JetBrains Mono", ui-monospace, Menlo, monospace';
  const SUP = { '-': '⁻', '0': '⁰', '1': '¹', '2': '²', '3': '³', '4': '⁴', '5': '⁵', '6': '⁶', '7': '⁷', '8': '⁸', '9': '⁹' };
  const sup = (n) => String(n).split('').map(ch => SUP[ch]).join('');
  const AMBER = '#f2a541', HA = '#ff5d52', TEAL = '#5fd0c4', DIM = 'rgba(220,224,232,0.55)', FG = '#f2efe8';

  // ---------- physics ----------
  const acosh1p = (x) => Math.log1p(x + Math.sqrt(x * (x + 2)));   // acosh(1 + x), accurate for small x

  // Constant proper acceleration a over distance d (hyperbolic motion; Rindler 2006 ch. 3; Gibbs & Baez "The Relativistic Rocket").
  // Brake at midpoint: tau = (2c/a) acosh(1 + a d/2c^2), t = 2 sqrt((d/2c)^2 + d/a), gamma_max = 1 + a d/2c^2.
  // Flyby (accelerate all the way): tau = (c/a) acosh(1 + a d/c^2), t = sqrt((d/c)^2 + 2d/a), gamma_max = 1 + a d/c^2.
  function trip(a, d, flyby) {
    const c = C.c;
    if (flyby) { const x = a * d / (c * c); const phi = acosh1p(x); return { tau: c / a * phi, t: Math.sqrt((d / c) ** 2 + 2 * d / a), gmax: 1 + x, phi, flyby }; }
    const x = a * d / (2 * c * c); const phi = acosh1p(x);
    return { tau: 2 * c / a * phi, t: 2 * Math.sqrt((d / (2 * c)) ** 2 + d / a), gmax: 1 + x, phi, flyby };
  }
  // State at ship proper time s. Accelerating leg: rapidity phi = a s/c, x = (2c^2/a) sinh^2(phi/2), t = (c/a) sinh phi.
  // The braking leg mirrors it. Phi_used is the rapidity change burned so far (for the rocket mass ratio).
  function stateAtTau(a, d, r, s) {
    const c = C.c, half = r.flyby ? r.tau : r.tau / 2;
    s = P.clamp(s, 0, r.tau);
    if (s <= half) { const p = a * s / c; return { x: 2 * c * c / a * Math.sinh(p / 2) ** 2, t: c / a * Math.sinh(p), phi: p, used: p, tau: s }; }
    const p = a * (r.tau - s) / c;
    return { x: d - 2 * c * c / a * Math.sinh(p / 2) ** 2, t: r.t - c / a * Math.sinh(p), phi: p, used: 2 * r.phi - p, tau: s };
  }
  // Same, parametrised by Earth time t: phi = asinh(a t / c).
  function stateAtT(a, d, r, t) {
    const c = C.c, th = r.flyby ? r.t : r.t / 2;
    t = P.clamp(t, 0, r.t);
    const s = t <= th ? c / a * Math.asinh(a * t / c) : r.tau - c / a * Math.asinh(a * (r.t - t) / c);
    return stateAtTau(a, d, r, s);
  }
  // Kinetic energy (gamma - 1) m c^2 from rapidity, written as 2 m c^2 sinh^2(phi/2) to stay accurate at low speed.
  const kinetic = (m, phi) => 2 * m * C.c * C.c * Math.sinh(phi / 2) ** 2;

  // ---------- formatting ----------
  function pow10Str(L) {
    if (L < 6) return P.fmt(10 ** L, 3);
    const e = Math.floor(L), m = 10 ** (L - e);
    return `${m.toFixed(1)} × 10${sup(e)}`;
  }
  const yrs = (sec) => { const y = sec / C.yr; return y < 100 ? `${y.toFixed(1)} yr` : `${P.fmt(y)} yr`; };
  // Speed from rapidity: km/s, per cent of c, or a string of nines when very close to c.
  function speedStr(phi) {
    const beta = Math.tanh(phi);
    if (beta < 0.01) return `${P.fmt(beta * C.c / 1e5, 3)} km/s`;
    if (beta < 0.999) return `${(beta * 100).toFixed(1)}% of c`;
    const eps = 1 / (Math.cosh(phi) ** 2 * (1 + beta)); // 1 - beta, without cancellation
    const n = Math.ceil(-Math.log10(eps));
    return `${(1 - eps).toFixed(Math.min(n + 1, 14))} c`;
  }
  const lyStr = (cm) => `${P.fmt(cm / C.ly, cm < 10 * C.ly ? 3 : 4)} ly`;
  function eVStr(e) {
    const u = [['eV', 1], ['keV', 1e3], ['MeV', 1e6], ['GeV', 1e9], ['TeV', 1e12]];
    let k = 0; while (k < 4 && e >= u[k + 1][1]) k++;
    return `${P.fmt(e / u[k][1], 3)} ${u[k][0]}`;
  }
  const pxFor = (w) => (w < 520 ? 10 : 12);

  // Deterministic background stars for the 2D scene.
  function starfield(n, seed) { const r = P.rng(seed); return Array.from({ length: n }, () => ({ x: r(), y: r(), m: r() ** 3, ph: r() * 6.28, T: 3000 + 9000 * r() ** 2 })); }

  // ---------- 1. relativistic flight (2D) ----------
  const accelSlider = (ctx) => ctx.slider({ label: 'Acceleration', min: -1, max: 1, step: 0.005, value: 0,
    format: (v) => `${P.fmt(10 ** v, 2)} g` });

  P.demo('fig-journey-flight', (ctx) => {
    const acc = accelSlider(ctx);
    const clock = ctx.choice({ label: 'Follow', options: [{ label: 'Ship clock', value: 'ship' }, { label: 'Earth clock', value: 'earth' }], value: 'ship' });
    const fly = ctx.toggle({ label: 'Flyby', value: false });
    const pause = ctx.toggle({ label: 'Pause', value: false });
    const RUN = 13, HOLD = 2.5;
    const stars = starfield(260, 11);
    let clockT = 0;
    return {
      animate: true,
      draw(t, dt) {
        if (!pause.value) clockT = (clockT + dt) % (RUN + HOLD);
        const f = Math.min(1, clockT / RUN);
        const a = 10 ** acc.value * C.g0, r = trip(a, D, fly.value);
        const st = clock.value === 'ship' ? stateAtTau(a, D, r, f * r.tau) : stateAtT(a, D, r, f * r.t);
        const g = ctx.g, w = ctx.w, h = ctx.h, px = pxFor(w);
        // own dark background: sits well in either page theme
        const bg = g.createLinearGradient(0, 0, 0, h); bg.addColorStop(0, '#070a12'); bg.addColorStop(1, '#03040a');
        g.fillStyle = bg; g.fillRect(0, 0, w, h);
        for (const s of stars) {
          const [cr, cg, cb] = P.bbColor(s.T);
          g.globalAlpha = (0.25 + 0.6 * s.m) * (0.75 + 0.25 * Math.sin(t * 1.3 + s.ph));
          g.fillStyle = `rgb(${cr},${cg},${cb})`; g.fillRect(s.x * w, s.y * h, 0.8 + 1.2 * s.m, 0.8 + 1.2 * s.m);
        }
        g.globalAlpha = 1;
        const x0 = 46, x1 = w - 46, yp = h * 0.66;
        const X = (cm) => x0 + cm / D * (x1 - x0);
        const kmax = Math.log(trip(10 * C.g0, D, true).gmax);      // widest tube over the slider range
        const hw = (gam) => 2 + Math.log(gam) / kmax * h * 0.21;
        const gAt = (x) => 1 + a * (fly.value ? x : Math.min(x, D - x)) / (C.c * C.c); // gamma at position x
        // distance axis
        g.strokeStyle = 'rgba(220,224,232,0.18)'; g.lineWidth = 1;
        g.beginPath(); g.moveTo(x0, yp); g.lineTo(x1, yp); g.stroke();
        g.font = `10px ${MONO}`; g.fillStyle = DIM; g.textAlign = 'center';
        for (let k = 0; k * 1000 * C.ly <= D; k++) {
          const x = X(k * 1000 * C.ly), yy = yp + h * 0.24;
          g.beginPath(); g.moveTo(x, yy); g.lineTo(x, yy + 5); g.stroke();
          if (w > 420 || k % 2 === 0) g.fillText(`${P.fmt(k * 1000)} ly`, x, yy + 17);
        }
        // the tube: width = log of the time-dilation factor gamma
        const xs = X(st.x), N = 180;
        const grd = g.createLinearGradient(0, yp - h * 0.22, 0, yp + h * 0.22);
        grd.addColorStop(0, 'rgba(242,165,65,0.02)'); grd.addColorStop(0.5, 'rgba(242,165,65,0.28)'); grd.addColorStop(1, 'rgba(242,165,65,0.02)');
        g.fillStyle = grd; g.beginPath();
        for (let i = 0; i <= N; i++) { const xx = x0 + (xs - x0) * i / N; g.lineTo(xx, yp - hw(gAt((xx - x0) / (x1 - x0) * D))); }
        for (let i = N; i >= 0; i--) { const xx = x0 + (xs - x0) * i / N; g.lineTo(xx, yp + hw(gAt((xx - x0) / (x1 - x0) * D))); }
        g.closePath(); g.fill();
        g.save(); g.shadowColor = AMBER; g.shadowBlur = 8; g.strokeStyle = 'rgba(242,165,65,0.75)'; g.lineWidth = 1;
        const ph = (t * 40) % 14;
        for (let xx = x0 + ph; xx < xs; xx += 14) {
          const rr = hw(gAt((xx - x0) / (x1 - x0) * D));
          g.beginPath(); g.ellipse(xx, yp, Math.max(1, rr * 0.17), rr, 0, 0, 2 * Math.PI); g.stroke();
        }
        g.restore();
        // Sun and Pillars
        g.save(); g.shadowColor = '#fff3d6'; g.shadowBlur = 14;
        g.fillStyle = '#fff3d6'; g.beginPath(); g.arc(x0, yp, 5, 0, 2 * Math.PI); g.fill(); g.restore();
        g.fillStyle = FG; g.font = `11px ${MONO}`; g.fillText('Sun', x0, yp - 14);
        [[-7, 9], [0, 13], [7, 6]].forEach(([dx, hh]) => {
          g.fillStyle = '#6b5038'; g.fillRect(x1 + dx - 2.5, yp + 4 - hh, 5, hh);
          g.save(); g.shadowColor = HA; g.shadowBlur = 8; g.fillStyle = HA; g.fillRect(x1 + dx - 2.5, yp + 4 - hh, 5, 1.6); g.restore();
        });
        g.fillStyle = HA; g.fillText('Pillars', x1, yp - 18);
        // ship: points along its thrust, flips at the midpoint when braking
        const dir = (!fly.value && st.x > D / 2) ? -1 : 1;
        if (f < 1) {
          g.save(); g.shadowColor = AMBER; g.shadowBlur = 12; g.fillStyle = AMBER; g.beginPath();
          g.moveTo(xs - 6 * dir, yp - 3); g.lineTo(xs - (15 + 4 * Math.sin(t * 31)) * dir, yp); g.lineTo(xs - 6 * dir, yp + 3); g.closePath(); g.fill(); g.restore();
        }
        g.fillStyle = FG; g.beginPath(); g.moveTo(xs + 8 * dir, yp); g.lineTo(xs - 6 * dir, yp - 5); g.lineTo(xs - 6 * dir, yp + 5); g.closePath(); g.fill();
        // HUD: speed, gamma, distance, and the photon-rocket mass ratio burned so far, m0/m = exp(rapidity used)
        g.textAlign = 'left';
        const yEnd = P.hud(g, 12, 12, [
          ['speed', speedStr(st.phi), AMBER],
          ['γ (time dilation)', P.fmt(Math.cosh(st.phi), 4)],
          ['from the Sun', lyStr(st.x)],
          ['photon-rocket m₀/m', pow10Str(st.used / Math.LN10)],
        ], { px });
        // ship vs Earth clocks, on one linear scale set by the full Earth-clock duration
        const bw = Math.min(w * 0.4, 320), bx = 12 + (px === 10 ? 44 : 54), by = yEnd + 8;
        [['ship', st.tau, r.tau], ['Earth', st.t, r.t]].forEach(([name, v, tot], i) => {
          const y = by + i * (px + 10);
          g.font = `${px}px ${MONO}`; g.textAlign = 'left';
          g.fillStyle = DIM; g.fillText(name, 14, y + px - 1);
          g.fillStyle = 'rgba(220,224,232,0.12)'; g.fillRect(bx, y, bw, px);
          g.save(); g.shadowColor = AMBER; g.shadowBlur = 6; g.fillStyle = AMBER; g.fillRect(bx, y, Math.max(1.5, bw * v / r.t), px); g.restore();
          g.fillStyle = FG; g.fillText(`${yrs(v)} / ${yrs(tot)}`, bx + bw + 8, y + px - 1);
        });
      },
    };
  });

  // ---------- shared GLSL: blackbody colour (Tanner Helland fit, same as P.bbColor) ----------
  const GLSL_BB = `
vec3 bb(float T){
  float t = clamp(T, 1000.0, 40000.0) / 100.0; float r, g, b;
  r = t <= 66.0 ? 1.0 : clamp(1.29293618606 * pow(t - 60.0, -0.1332047592), 0.0, 1.0);
  g = t <= 66.0 ? clamp(0.39008157876 * log(t) - 0.63184144378, 0.0, 1.0) : clamp(1.12989086089 * pow(t - 60.0, -0.0755148492), 0.0, 1.0);
  b = t >= 66.0 ? 1.0 : (t <= 19.0 ? 0.0 : clamp(0.54320678911 * log(t - 10.0) - 1.19625408914, 0.0, 1.0));
  return vec3(r, g, b);
}`;

  // ---------- 2. the cockpit view (3D) ----------
  P.demo3d('fig-journey-cockpit', (ctx) => {
    const PHI_MAX = acosh1p(trip(C.g0, D, false).gmax - 1);      // peak rapidity of the 1 g trip, ~8.68
    const sp = ctx.slider({ label: 'Speed', min: -2, max: Math.log10(Math.sinh(PHI_MAX)), step: 0.002, value: 0,
      format: (v) => speedStr(Math.asinh(10 ** v)), onInput: () => { auto.el.setAttribute('aria-pressed', 'false'); } });
    const auto = ctx.toggle({ label: 'Autopilot', value: true });
    const eye = ctx.choice({ label: 'Light', options: [{ label: 'Visible', value: 0 }, { label: 'All wavelengths', value: 1 }], value: 0 });

    const scene = new THREE.Scene();
    scene.background = new THREE.Color('#020308');
    const cam = new THREE.PerspectiveCamera(70, 1, 0.05, 2000);
    cam.position.set(0, 0.9, 1.5);

    // Stars: rest-frame polar angle from the direction of travel (-z), azimuth, temperature, brightness.
    const NS = 9000, rnd = P.rng(21);
    const aTh = new Float32Array(NS), aAz = new Float32Array(NS), aT = new Float32Array(NS), aB = new Float32Array(NS);
    for (let i = 0; i < NS; i++) {
      aTh[i] = Math.acos(2 * rnd() - 1); aAz[i] = 2 * Math.PI * rnd();
      aT[i] = 2800 * Math.pow(12, rnd() ** 1.8);                    // 2,800 to ~34,000 K, mostly cool
      aB[i] = Math.pow(10, -0.4 * 7 * Math.pow(rnd(), 0.55));        // magnitudes 0 to 7, mostly faint
    }
    const sg = new THREE.BufferGeometry();
    sg.setAttribute('position', new THREE.BufferAttribute(new Float32Array(NS * 3), 3));
    sg.setAttribute('aTh', new THREE.BufferAttribute(aTh, 1)); sg.setAttribute('aAz', new THREE.BufferAttribute(aAz, 1));
    sg.setAttribute('aT', new THREE.BufferAttribute(aT, 1)); sg.setAttribute('aB', new THREE.BufferAttribute(aB, 1));
    const starMat = new THREE.ShaderMaterial({
      uniforms: { uPhi: { value: 0 }, uBolo: { value: 0 }, uPx: { value: 1 }, uGain: { value: 1 }, uZoom: { value: 1 }, uMap: { value: P.dotTexture() } },
      vertexShader: `
attribute float aTh, aAz, aT, aB;
uniform float uPhi, uBolo, uPx, uGain, uZoom;
varying vec3 vCol; varying float vA;
${GLSL_BB}
void main(){
  // Aberration, exact and free of cancellation: tan(th'/2) = e^(-phi) tan(th/2)  (McKinley & Doherty 1979)
  float th2 = 2.0 * atan(tan(0.5 * aTh) * exp(-uPhi));
  vec3 dir = vec3(sin(th2) * cos(aAz), sin(th2) * sin(aAz), -cos(th2));
  // Doppler factor delta = gamma (1 + beta cos th) = e^phi cos^2(th/2) + e^-phi sin^2(th/2)
  float c2 = cos(0.5 * aTh), s2 = sin(0.5 * aTh);
  float delta = exp(uPhi) * c2 * c2 + exp(-uPhi) * s2 * s2;
  float Tobs = aT * delta;                       // a blackbody stays a blackbody at temperature delta*T
  float f;
  if (uBolo > 0.5) f = delta * delta;            // bolometric flux of a point source seen by a moving observer
  else {
    // flux at 550 nm: F' = F * B(550nm, delta T) / B(550nm, T) / delta^2, with x = hc / (550 nm k T)
    float x = 26160.0 / aT, xd = x / delta;
    f = xd > 80.0 ? 0.0 : (exp(x) - 1.0) / (xd < 1e-3 ? xd : exp(xd) - 1.0) / (delta * delta);
  }
  float b = aB * f;
  vCol = bb(Tobs);
  // display only: soft saturating curve so bunched stars read as sparkle, not a solid blob
  float s = 1.0 - exp(-2.5 * b);
  float bg = b * uGain; s = 1.0 - exp(-2.5 * bg);
  vA = (0.08 + 0.5 * s) * clamp(bg * 60.0, 0.0, 1.0);
  gl_PointSize = uPx * uZoom * (1.0 + 2.6 * s);
  vec4 mv = modelViewMatrix * vec4(dir * 900.0, 1.0);
  gl_Position = projectionMatrix * mv;
}`,
      fragmentShader: `
uniform sampler2D uMap; varying vec3 vCol; varying float vA;
void main(){ float m = texture2D(uMap, gl_PointCoord).r; gl_FragColor = vec4(vCol * vA * m, 1.0); }`,
      transparent: true, depthWrite: false, blending: THREE.AdditiveBlending,
    });
    const starPts = new THREE.Points(sg, starMat); starPts.frustumCulled = false; starPts.renderOrder = -1;
    scene.add(starPts);
    const scopeScene = new THREE.Scene(); scopeScene.background = new THREE.Color('#010205');
    const scopeMat = starMat.clone(); scopeMat.uniforms.uMap.value = P.dotTexture();
    const scopePts = new THREE.Points(sg, scopeMat); scopePts.frustumCulled = false; scopeScene.add(scopePts);
    const scopeCam = new THREE.PerspectiveCamera(60, 1, 1, 2000); scopeCam.lookAt(0, 0, -1);

    // Bow shield ahead and below the cockpit, heated by the headwind.
    const SH = { y: -2.9, z: -5.2, r: 2.0 };          // sphere centre and radius
    const shieldMat = new THREE.ShaderMaterial({
      uniforms: { uHeat: { value: 0 }, uTime: { value: 0 } },
      vertexShader: `varying vec3 vN, vV, vW; varying vec3 vP;
        void main(){ vec4 mv = modelViewMatrix * vec4(position, 1.0); vN = normalize(normalMatrix * normal); vV = -mv.xyz;
          vW = normalize(mat3(modelMatrix) * normal); vP = position; gl_Position = projectionMatrix * mv; }`,
      fragmentShader: `
varying vec3 vN, vV, vW, vP; uniform float uHeat, uTime;
${GLSL_BB}
float h(vec3 p){ return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453); }
float n(vec3 p){ vec3 i = floor(p), f = fract(p); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(h(i), h(i + vec3(1,0,0)), f.x), mix(h(i + vec3(0,1,0)), h(i + vec3(1,1,0)), f.x), f.y),
             mix(mix(h(i + vec3(0,0,1)), h(i + vec3(1,0,1)), f.x), mix(h(i + vec3(0,1,1)), h(i + vec3(1,1,1)), f.x), f.y), f.z); }
void main(){
  vec3 N = normalize(vN), V = normalize(vV);
  float fres = pow(1.0 - max(dot(N, V), 0.0), 2.4);
  float front = smoothstep(-0.3, 0.9, dot(vW, vec3(0.0, 0.0, -1.0)));   // face toward the headwind
  vec3 steel = vec3(0.05, 0.055, 0.07) + vec3(0.09, 0.1, 0.12) * max(dot(vW, normalize(vec3(0.2, 1.0, 0.3))), 0.0);
  steel += vec3(0.25, 0.28, 0.34) * fres * 0.5;
  float flick = 0.7 + 0.3 * n(vP * 3.0 + vec3(0.0, 0.0, uTime * 4.0));
  float T = 900.0 + 5600.0 * uHeat;                                      // amber at low speed, white-hot at peak
  float e = pow(uHeat, 0.7) * flick * (0.3 * front + 3.2 * fres * (0.5 + 0.5 * front) + 0.12);
  vec3 hot = bb(T) * e * (0.8 + 1.6 * uHeat);
  hot = hot / (1.0 + hot);
  gl_FragColor = vec4(steel + hot, 1.0);
}`,
    });
    const shield = new THREE.Mesh(new THREE.SphereGeometry(SH.r, 64, 40), shieldMat);
    shield.position.set(0, SH.y, SH.z);
    scene.add(shield);
    // Soft camera-facing halo behind the shield, additive.
    const halo = new THREE.Mesh(new THREE.PlaneGeometry(SH.r * 3.4, SH.r * 3.4), new THREE.ShaderMaterial({
      uniforms: { uHeat: { value: 0 } },
      vertexShader: `varying vec2 vUv; void main(){ vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
      fragmentShader: `varying vec2 vUv; uniform float uHeat;
        ${GLSL_BB}
        void main(){ float r = length(vUv - 0.5) * 2.0;
        float a = uHeat * uHeat * smoothstep(1.0, 0.58, r) * 0.32; gl_FragColor = vec4(bb(900.0 + 5600.0 * uHeat) * a, 1.0); }`,
      transparent: true, depthWrite: false, blending: THREE.AdditiveBlending,
    }));
    halo.position.copy(shield.position);
    scene.add(halo);

    // Interstellar hydrogen as streaks, dust as rare bright grains. Toy visual speed; physical numbers live in the HUD.
    const NH = 1400, ND = 24, ZF = -110, ZB = 3;
    const gas = Array.from({ length: NH }, () => ({ x: (rnd() - 0.5) * 22, y: (rnd() - 0.5) * 14, z: ZF + rnd() * (ZB - ZF) }));
    const dust = Array.from({ length: ND }, () => ({ x: (rnd() - 0.5) * 12, y: (rnd() - 0.5) * 8 - 1, z: ZF + rnd() * (ZB - ZF) }));
    const lg = new THREE.BufferGeometry();
    const lpos = new Float32Array(NH * 6), lcol = new Float32Array(NH * 6);
    lg.setAttribute('position', new THREE.BufferAttribute(lpos, 3)); lg.setAttribute('color', new THREE.BufferAttribute(lcol, 3));
    const lines = new THREE.LineSegments(lg, new THREE.LineBasicMaterial({ vertexColors: true, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending }));
    lines.frustumCulled = false; scene.add(lines);
    // Dust grains and impact sparks share one point shader.
    const NSP = 260, NP = ND + NSP;
    const ppos = new Float32Array(NP * 3), psz = new Float32Array(NP), pcol = new Float32Array(NP * 3);
    const life = new Float32Array(NSP);
    const pg = new THREE.BufferGeometry();
    pg.setAttribute('position', new THREE.BufferAttribute(ppos, 3)); pg.setAttribute('aSize', new THREE.BufferAttribute(psz, 1)); pg.setAttribute('aCol', new THREE.BufferAttribute(pcol, 3));
    const pts = new THREE.Points(pg, new THREE.ShaderMaterial({
      uniforms: { uMap: { value: P.dotTexture() }, uPx: { value: 1 } },
      vertexShader: `attribute float aSize; attribute vec3 aCol; varying vec3 vC; uniform float uPx;
        void main(){ vC = aCol; vec4 mv = modelViewMatrix * vec4(position, 1.0); gl_PointSize = uPx * aSize * 6.0 / max(0.3, -mv.z); gl_Position = projectionMatrix * mv; }`,
      fragmentShader: `uniform sampler2D uMap; varying vec3 vC; void main(){ gl_FragColor = vec4(vC * texture2D(uMap, gl_PointCoord).r, 1.0); }`,
      transparent: true, depthWrite: false, depthTest: false, blending: THREE.AdditiveBlending,   // impacts on the far face glow through
    }));
    pts.frustumCulled = false; pts.renderOrder = 2; scene.add(pts);
    // z where a particle moving along +z at (x, y) meets the shield sphere, or null if it misses
    const hitZ = (x, y) => { const dy = y - SH.y, q = SH.r * SH.r - x * x - dy * dy; return q > 0 ? SH.z - Math.sqrt(q) : null; };
    let nextSpark = 0;
    const spark = (x, y, z, s, hot) => { const k = nextSpark++ % NSP; life[k] = 1; const nx = x / SH.r, ny = (y - SH.y) / SH.r, nz = (z - SH.z) / SH.r; ppos.set([x + nx * 0.06, y + ny * 0.06, z + nz * 0.06], (ND + k) * 3); psz[ND + k] = s; pcol.set(hot, (ND + k) * 3); };

    // Look around by dragging; the view drifts gently when left alone.
    const look = { yaw: 0, pitch: 0, vy: 0, vp: 0, idle: 99 };
    P.drag(ctx.canvas, (dx, dy, x, y, phase) => { if (phase !== 'move') return; look.vy = dx * 0.005; look.vp = dy * 0.005; look.yaw += look.vy; look.pitch = P.clamp(look.pitch + look.vp, -1.2, 1.2); look.idle = 0; });

    let phi = 0, clockT = 0;
    const st = { phi: 0, fwd: new THREE.Vector3(), hits: 0, scopeFov: 60, scopeGain: 1 };
    const scopeBox = (pw, ph) => { const sz = Math.round(Math.min(pw, ph) * 0.3), m = Math.round(sz * 0.06); return { x: pw - sz - m, y: ph - sz - m, sz }; };
    return {
      camera: cam, scale: 1,
      renderTo(r, pw, ph) {
        r.render(scene, cam);
        const b = scopeBox(pw, ph);               // WebGL origin is bottom-left; put the scope at the top right
        r.setViewport(b.x, b.y, b.sz, b.sz); r.setScissor(b.x, b.y, b.sz, b.sz);
        scopeMat.uniforms.uZoom.value = 1.25;
        r.render(scopeScene, scopeCam);
        r.setViewport(0, 0, pw, ph); r.setScissor(0, 0, pw, ph);
      },
      update(t, dt, pw) {
        // Autopilot: a 1 g trip compressed to 40 s. Rapidity grows linearly with ship time at constant proper acceleration.
        if (auto.value) {
          clockT += dt;
          const u = (clockT % 40) / 40;
          phi = PHI_MAX * (u < 0.5 ? P.smooth(u * 2) : P.smooth(2 - u * 2));
          sp.value = Math.log10(Math.max(0.01, Math.sinh(phi)));
        } else phi += (Math.asinh(10 ** sp.value) - phi) * Math.min(1, dt * 4);
        st.phi = phi;
        const beta = Math.tanh(phi), gam = Math.cosh(phi);
        starMat.uniforms.uPhi.value = phi; starMat.uniforms.uBolo.value = eye.value;
        starMat.uniforms.uPx.value = pw / ctx.w; pts.material.uniforms.uPx.value = pw / ctx.w;
        // Shield heating from the gas: power per area in the ship frame = gamma n beta c (gamma-1) m_p c^2
        const Pw = gam * N_ISM * beta * C.c * kinetic(MP, phi) * 1e-3;      // W/m^2
        const heat = P.clamp((Math.log10(Math.max(Pw, 1)) - 2) / 9.7, 0, 1);
        shieldMat.uniforms.uHeat.value = heat; shieldMat.uniforms.uTime.value = t; halo.material.uniforms.uHeat.value = heat;
        halo.lookAt(cam.position); halo.position.copy(shield.position).addScaledVector(halo.getWorldDirection(new THREE.Vector3()), -0.6);
        // forward scope: field of view chosen so the aberrated front hemisphere fills it (display aid, not physics)
        const half = 2 * Math.atan(Math.exp(-phi));                     // where stars from 90 deg now sit, rad
        st.scopeFov = P.clamp(2.4 * half * 180 / Math.PI, 0.0005, 60);
        scopeCam.fov = st.scopeFov; scopeCam.aspect = 1; scopeCam.updateProjectionMatrix();
        scopeMat.uniforms.uPhi.value = phi; scopeMat.uniforms.uBolo.value = eye.value; scopeMat.uniforms.uPx.value = pw / ctx.w;
        // scope auto-exposure: in visible light the forward stars fade at extreme speed (energy moves to X-rays), so the
        // scope brightens by the factor a Sun-like star (x = hc/550nm kT ~ 4.5) has dimmed. Display aid; the main view is not rescaled.
        const dF = Math.exp(phi), xs = 4.5;
        const fSun = eye.value ? dF * dF : Math.expm1(xs) / Math.expm1(xs / dF) / (dF * dF);
        st.scopeGain = P.clamp(0.5 / fSun, 1, 1e6); scopeMat.uniforms.uGain.value = st.scopeGain;
        // camera
        if (look.idle > 2.5) { look.yaw *= 0.985; look.pitch *= 0.985; }
        look.idle += dt;
        const yaw = look.yaw + 0.1 * Math.sin(t * 0.07), pitch = look.pitch + 0.06 + 0.03 * Math.sin(t * 0.05);
        cam.rotation.set(-pitch - 0.02, -yaw, 0, 'YXZ');
        // particles: visual speed grows with beta, streak length with rapidity
        const v = 3 + 150 * beta, len = 0.05 + 7 * Math.tanh(phi / 3);
        const gasA = P.clamp(beta * 3, 0, 1);
        for (let i = 0; i < NH; i++) {
          const p = gas[i]; const z0 = p.z; p.z += v * dt;
          const zh = hitZ(p.x, p.y);
          if (zh !== null && z0 < zh && p.z >= zh) { if (rnd() < 0.45) spark(p.x, p.y, zh, 0.9 + 1.4 * heat, [0.55 + 0.45 * heat, 0.7 + 0.3 * heat, 1.0]); p.z = ZF; }
          if (p.z > ZB) { p.z = ZF + rnd() * 5; p.x = (rnd() - 0.5) * 22; p.y = (rnd() - 0.5) * 14; }
          const k = i * 6, fade = P.clamp((p.z - ZF) / 70, 0, 1) ** 2 * gasA;
          lpos[k] = p.x; lpos[k + 1] = p.y; lpos[k + 2] = p.z;
          lpos[k + 3] = p.x; lpos[k + 4] = p.y; lpos[k + 5] = p.z - len;
          lcol[k] = 0.28 * fade; lcol[k + 1] = 0.4 * fade; lcol[k + 2] = 0.65 * fade;
          lcol[k + 3] = 0; lcol[k + 4] = 0; lcol[k + 5] = 0;
        }
        lg.attributes.position.needsUpdate = true; lg.attributes.color.needsUpdate = true;
        for (let i = 0; i < ND; i++) {
          const p = dust[i]; const z0 = p.z; p.z += v * dt;
          const zh = hitZ(p.x, p.y);
          if (zh !== null && z0 < zh && p.z >= zh) { spark(p.x, p.y, zh, 3 + 5 * heat, [1.0, 0.8, 0.45]); p.z = ZF; }
          if (p.z > ZB) { p.z = ZF; p.x = (rnd() - 0.5) * 12; p.y = (rnd() - 0.5) * 8 - 1; }
          ppos.set([p.x, p.y, p.z], i * 3); psz[i] = 0.35 * gasA + 0.05; pcol.set([0.95 * gasA, 0.65 * gasA, 0.3 * gasA], i * 3);
        }
        for (let k = 0; k < NSP; k++) {
          life[k] = Math.max(0, life[k] - dt * 2.2);
          const L = life[k], j = ND + k;
          if (L <= 0) { psz[j] = 0; continue; }
          pcol[j * 3] *= 0.97; pcol[j * 3 + 1] *= 0.95; pcol[j * 3 + 2] *= 0.9;
          psz[j] *= 0.985;
        }
        pg.attributes.position.needsUpdate = true; pg.attributes.aSize.needsUpdate = true; pg.attributes.aCol.needsUpdate = true;
        st.fwd.set(0, 0, -900).project(cam);
      },
      overlay(g, w, h) {
        const phi = st.phi, beta = Math.tanh(phi), gam = Math.cosh(phi), px = pxFor(w);
        // The microwave background ahead: T' = T0 e^phi straight ahead (T0 = 2.725 K). Drawn as glare: it is a point at this field of view.
        const Tc = 2.725 * Math.exp(phi);
        if (Tc > 900 && st.fwd.z < 1) {
          const sx = (st.fwd.x * 0.5 + 0.5) * w, sy = (-st.fwd.y * 0.5 + 0.5) * h;
          const I = eye.value ? Math.pow(Tc / 5800, 4) : 89.8 / Math.expm1(26160 / Tc);   // relative to the Sun's surface
          const R = Math.min(w, h) * 0.05 * P.clamp((Math.log10(I) + 5) / 6.5, 0.1, 1);
          const [r, gg, b] = P.bbColor(Tc);
          const grd = g.createRadialGradient(sx, sy, 0, sx, sy, R);
          grd.addColorStop(0, `rgba(255,255,255,0.55)`); grd.addColorStop(0.2, `rgba(${r},${gg},${b},0.3)`); grd.addColorStop(1, `rgba(${r},${gg},${b},0)`);
          g.save(); g.globalCompositeOperation = 'lighter'; g.fillStyle = grd; g.beginPath(); g.arc(sx, sy, R, 0, 2 * Math.PI); g.fill(); g.restore();
        }
        { // forward scope frame (matches scopeBox in CSS px)
          const sz = Math.min(w, h) * 0.3, m = sz * 0.06, x = w - sz - m, y = m;
          g.save(); g.strokeStyle = 'rgba(220,224,232,0.45)'; g.lineWidth = 1; g.strokeRect(x + 0.5, y + 0.5, sz - 1, sz - 1);
          g.font = `${px - 1}px ${MONO}`; g.fillStyle = 'rgba(220,224,232,0.75)'; g.textAlign = 'right';
          g.fillText(`forward scope, ${st.scopeFov < 0.1 ? st.scopeFov.toPrecision(2) : P.fmt(st.scopeFov, 2)}° field`, x + sz - 6, y + sz + px + 2);
          if (st.scopeGain > 1.5) g.fillText(`exposure × ${P.fmt(st.scopeGain, 2)}`, x + sz - 6, y + sz + 2 * px + 5);
          g.restore();
        }
        const Ep = kinetic(MP, phi);                                    // erg per proton
        const flux = gam * N_ISM * beta * C.c;                          // protons per cm^2 per ship-second
        const Pw = flux * Ep * 1e-3;                                    // W/m^2
        const dust = kinetic(1e-6, phi);                                // 1 microgram grain, erg
        const half = 2 * Math.atan(Math.exp(-phi)) * 180 / Math.PI;     // where the stars of the front hemisphere now sit
        P.hud(g, 12, 12, [
          ['speed', speedStr(phi), AMBER],
          ['γ', P.fmt(gam, 4)],
          ['front half of sky', `within ${half < 1 ? half.toPrecision(2) : P.fmt(half, 3)}°`],
          ['each H atom', eVStr(Ep / EV)],
          ['hits per cm² per s', P.sci(flux, 2)],
          ['power on shield', `${P.sci(Pw, 2)} W/m²`, HA],
          ['1 µg dust grain', dust / TNT >= 1 ? `${P.fmt(dust / TNT, 3)} t TNT` : `${P.fmt(dust / TNT * 1e3, 3)} kg TNT`, HA],
          ['CMB ahead', `${P.fmt(Tc, 3)} K`],
        ], { px });
      },
    };
  }, { aspect: 0.6 });

  // ---------- 3. the map redrawn as travel time (3D) ----------
  P.demo3d('fig-journey-map', (ctx) => {
    const mode = ctx.choice({ label: 'Map', options: [{ label: 'Auto', value: -1 }, { label: 'True distance', value: 0 }, { label: 'Ship time', value: 1 }], value: -1 });
    const acc = accelSlider(ctx);
    const scene = new THREE.Scene(); scene.background = new THREE.Color('#020308');
    const cam = new THREE.PerspectiveCamera(40, 1, 0.1, 6000);
    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(0, 0, 0), dist: 120, yaw: 0.6, pitch: 0.5, auto: 0.05 });
    // Galactic coordinates -> scene: Sun at origin, Galactic centre toward -z, north Galactic pole +y. Units: kly.
    const gal = (l, b, ly) => { const L = l * Math.PI / 180, B = b * Math.PI / 180, r = ly / 1000; return [r * Math.cos(B) * Math.sin(L) * -1, r * Math.sin(B), -r * Math.cos(B) * Math.cos(L)]; };
    const GC = gal(0, 0, 26700);                                    // GRAVITY 2019: 8.18 kpc
    const rnd = P.rng(7);
    const pos = [], col = [], siz = [];
    const add = (p, c, s) => { pos.push(...p); col.push(...c); siz.push(s); };
    // Toy Milky Way: exponential disc, 4 log-spiral arms, a bar-ish bulge; colours: young blue-white, H II pink, old gold.
    const gauss = () => { let u = 0; for (let k = 0; k < 4; k++) u += rnd(); return (u - 2) / 0.58; };
    for (let i = 0; i < 42000; i++) {
      let R, th, z, c, s = 1;
      if (i < 7000) { R = Math.abs(gauss()) * 3.2; th = rnd() * 6.283; z = gauss() * 1.4; c = [0.75, 0.58, 0.38]; }
      else {
        R = -11.4 * Math.log(1 - rnd() * 0.985); if (R > 52) R = 52 * rnd();
        const arm = Math.floor(rnd() * 4), pitch = 0.22;
        th = Math.log(Math.max(R, 1.5) / 4) / Math.tan(pitch) + arm * Math.PI / 2 + gauss() * 0.28 * (rnd() < 0.7 ? 1 : 3);
        z = gauss() * 0.35;
        const young = rnd() < 0.6;
        c = rnd() < 0.05 ? [1.0, 0.35, 0.32] : young ? [0.62, 0.75, 1.0] : [1.0, 0.86, 0.66];
        s = rnd() < 0.05 ? 1.8 : 1;
      }
      add([GC[0] + R * Math.cos(th), z, GC[2] + R * Math.sin(th)], c, s);
    }
    // LMC (Pietrzynski 2019, 162,000 ly; l=280.5 b=-32.9) and Andromeda (~2.5 Mly; l=121.2 b=-21.6), as small particle clouds.
    const LMC = gal(280.5, -32.9, 162000), AND = gal(121.2, -21.6, 2.5e6);
    for (let i = 0; i < 2500; i++) add([LMC[0] + gauss() * 3, LMC[1] + gauss() * 1.2, LMC[2] + gauss() * 2], [0.75, 0.8, 1.0], 1);
    for (let i = 0; i < 6000; i++) { const R = Math.abs(gauss()) * 25, th = rnd() * 6.283; add([AND[0] + R * Math.cos(th), AND[1] + R * Math.sin(th) * 0.35 + gauss() * 1.5, AND[2] + R * Math.sin(th) * 0.9], i < 2000 ? [1.0, 0.85, 0.6] : [0.7, 0.78, 1.0], 1.4); }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
    geo.setAttribute('aCol', new THREE.Float32BufferAttribute(col, 3));
    geo.setAttribute('aSize', new THREE.Float32BufferAttribute(siz, 1));
    // Vertex shader moves every particle from its true position to direction * ship time (1 g brachistochrone, units yr and ly):
    // tau = (2/a) acosh(1 + a d / 2), a in ly/yr^2 (1 g = 1.0323 ly/yr^2).
    const SHIPTAU = `
uniform float uA;
float tauOf(float d){ float x = 1.0 + 0.5 * uA * d; return 2.0 / uA * log(x + sqrt(x * x - 1.0)); }`;
    const mat = new THREE.ShaderMaterial({
      uniforms: { uMix: { value: 0 }, uA: { value: 1.0323 }, uS: { value: 2 }, uPx: { value: 1 }, uMap: { value: P.dotTexture() } },
      vertexShader: `
attribute vec3 aCol; attribute float aSize; uniform float uMix, uS, uPx; varying vec3 vC; varying float vA;
${SHIPTAU}
void main(){
  float d = length(position) * 1000.0;
  vec3 ship = d > 0.0 ? normalize(position) * tauOf(d) * uS : position;
  vec3 p = mix(position, ship, uMix);
  vec4 mv = modelViewMatrix * vec4(p, 1.0);
  gl_PointSize = uPx * aSize * clamp(140.0 / -mv.z, 1.0, 4.0);
  vC = aCol; vA = aSize > 1.5 ? 0.5 : 0.34;
  gl_Position = projectionMatrix * mv;
}`,
      fragmentShader: `uniform sampler2D uMap; varying vec3 vC; varying float vA; void main(){ gl_FragColor = vec4(vC * vA * texture2D(uMap, gl_PointCoord).r, 1.0); }`,
      transparent: true, depthWrite: false, blending: THREE.AdditiveBlending,
    });
    const cloud = new THREE.Points(geo, mat); cloud.frustumCulled = false; scene.add(cloud);
    // Rings in the Galactic plane, one set per mode; their opacity follows the morph.
    const ring = (r, color) => {
      const pts = []; for (let k = 0; k <= 128; k++) { const a = k / 128 * 6.2832; pts.push(new THREE.Vector3(r * Math.cos(a), 0, r * Math.sin(a))); }
      const m = new THREE.LineBasicMaterial({ color, transparent: true, opacity: 0.3, depthWrite: false });
      const l = new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), m); scene.add(l); return l;
    };
    const ringsD = [10, 25, 50].map(r => ({ v: r, line: ring(r, 0x9ea3ad) }));
    let ringsT = [];
    const LM = [
      { n: 'Proxima Cen.', ly: 4.246, l: 313.9, b: -1.9 }, { n: 'Pleiades', ly: 444, l: 166.6, b: -23.5 },
      { n: 'Orion Nebula', ly: 1350, l: 209.0, b: -19.4 }, { n: 'Pillars', ly: D / C.ly, l: 17.0, b: 0.8, pill: true },
      { n: 'Galactic centre', ly: 26700, l: 0, b: 0 }, { n: 'LMC', ly: 162000, l: 280.5, b: -32.9 },
      { n: 'Andromeda', ly: 2.5e6, l: 121.2, b: -21.6 },
    ].map(m => ({ ...m, p: gal(m.l, m.b, m.ly) }));
    let mix = 0, autoT = 0, lastA = -1, S = 2;
    const v3 = new THREE.Vector3();
    const aLy = () => 10 ** acc.value * C.g0 * C.yr * C.yr / C.ly;   // acceleration in ly/yr^2
    const tauLy = (ly, a) => { const x = 0.5 * a * ly; return 2 / a * acosh1p(x); };
    return {
      scene, camera: cam, scale: 1,
      update(t, dt, pw) {
        const a = aLy();
        if (a !== lastA) {                    // rescale ship mode so Andromeda always sits at ~60 units, rebuild time rings
          lastA = a; S = 60 / tauLy(2.5e6, a);
          ringsT.forEach(r => scene.remove(r.line));
          const tm = tauLy(2.5e6, a), step = tm > 200 ? 50 : tm > 60 ? 20 : tm > 25 ? 10 : 5;
          ringsT = []; for (let k = step; k < tm + step; k += step) ringsT.push({ v: k, line: ring(k * S, 0xf2a541) });
        }
        autoT += dt;
        const target = mode.value < 0 ? ((autoT % 16) < 8 ? 0 : 1) : mode.value;
        mix += (target - mix) * Math.min(1, dt * 1.6);
        const e = P.smooth(P.clamp(mix, 0, 1));
        mat.uniforms.uMix.value = e; mat.uniforms.uA.value = a; mat.uniforms.uS.value = S; mat.uniforms.uPx.value = pw / ctx.w;
        ringsD.forEach(r => { r.line.material.opacity = 0.28 * (1 - e); });
        ringsT.forEach(r => { r.line.material.opacity = 0.35 * e; });
        orb.dist = P.lerp(150, 150, e);
        orb.target.set(GC[0] * 0.55 * (1 - e), 0, GC[2] * 0.55 * (1 - e));
        orb.step(dt);
      },
      overlay(g, w, h) {
        const e = P.smooth(P.clamp(mix, 0, 1)), a = aLy(), px = pxFor(w);
        g.font = `${w < 520 ? 9 : 11}px ${MONO}`;
        const proj = (p) => { v3.set(p[0], p[1], p[2]).project(cam); return v3.z < 1 ? [(v3.x * 0.5 + 0.5) * w, (-v3.y * 0.5 + 0.5) * h] : null; };
        // ring labels
        g.fillStyle = 'rgba(220,224,232,0.55)'; g.textAlign = 'left';
        ringsD.forEach(r => { if (e < 0.5) { const s = proj([r.v, 0, 0]); if (s) g.fillText(`${r.v},000 ly`, s[0] + 3, s[1]); } });
        g.fillStyle = 'rgba(242,165,65,0.8)';
        ringsT.forEach(r => { if (e > 0.5) { const s = proj([r.v * S, 0, 0]); if (s) g.fillText(`${r.v} yr`, s[0] + 3, s[1]); } });
        // Sun and landmarks
        const sun = proj([0, 0, 0]);
        if (sun) { g.fillStyle = '#fff3d6'; g.beginPath(); g.arc(sun[0], sun[1], 3, 0, 6.283); g.fill(); }
        LM.forEach(m => {
          const tr = m.p, r = Math.hypot(...tr), k = tauLy(m.ly, a) * S / r;
          const p = tr.map(x => P.lerp(x, x * k, e));
          const s = proj(p);
          if (!s || s[0] < -50 || s[0] > w + 50 || s[1] < -20 || s[1] > h + 20) return;
          if (sun && Math.hypot(s[0] - sun[0], s[1] - sun[1]) < 10) return;
          const foot = proj([p[0], 0, p[2]]);       // drop-line to the Galactic plane shows height above or below it
          if (foot) { g.strokeStyle = 'rgba(220,224,232,0.35)'; g.setLineDash([2, 3]); g.beginPath(); g.moveTo(s[0], s[1]); g.lineTo(foot[0], foot[1]); g.stroke(); g.setLineDash([]); }
          g.fillStyle = m.pill ? HA : FG; g.beginPath(); g.arc(s[0], s[1], m.pill ? 3.5 : 2.5, 0, 6.283); g.fill();
          g.textAlign = s[0] > w - 120 ? 'right' : 'left';
          const dx = g.textAlign === 'right' ? -7 : 7;
          g.fillText(m.n, s[0] + dx, s[1] - 3);
          g.fillStyle = 'rgba(220,224,232,0.6)';
          g.fillText(e > 0.5 ? `${tauLy(m.ly, a).toFixed(1)} yr ship` : `${P.fmt(m.ly)} ly`, s[0] + dx, s[1] + 10);
        });
        const r = trip(10 ** acc.value * C.g0, D, false);
        g.textAlign = 'left';
        P.hud(g, 12, 12, [
          ['map', e > 0.5 ? 'ship travel time' : 'true distance', e > 0.5 ? AMBER : FG],
          ['acceleration', `${P.fmt(10 ** acc.value, 2)} g, braking to stop`],
          ['Pillars', `${yrs(r.tau)} ship, ${P.fmt(r.t / C.yr)} yr Earth`, HA],
          ['Andromeda', `${tauLy(2.5e6, a).toFixed(1)} yr ship`],
        ], { px });
      },
    };
  }, { aspect: 0.62 });
})();
