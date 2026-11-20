// Section 01 "Where to look": animated toy sky, a powers-of-ten flight to the Eagle, and a 3D parallax scene.
(function () {
  'use strict';
  const C = P.C;
  const D2R = Math.PI / 180;
  const ARCSEC = D2R / 3600;            // radians per arcsecond
  const D_PC = 1740;                    // canonical distance, facts.md (Kuhn+2019)
  const AU_PER_PC = C.pc / C.au;        // ~206,265
  const LY_PER_PC = C.pc / C.ly;        // ~3.26
  const NOW = 2026;
  const TRAVEL = Math.round(D_PC * LY_PER_PC);   // light travel time t = d / c = 5,675 yr
  const DEP = NOW - TRAVEL;                        // departure, astronomical year (-3649 = 3650 BCE)
  const yearLabel = (y) => (y <= 0 ? `${1 - Math.round(y)} BCE` : `${Math.round(y)} CE`);
  const mono = (px) => `${px}px "JetBrains Mono", ui-monospace, monospace`;
  const hudPx = (w) => (w < 520 ? 10 : 12);

  // Taller stages on phones: core reads data-aspect once when a demo starts, so set it before then.
  // shortcut: decided at load time only; rotating a phone afterwards keeps the first aspect.
  if (window.innerWidth < 600) [['fig-where-sky', 0.95], ['fig-where-flight', 1.05], ['fig-where-parallax', 1.05]].forEach(([id, a]) => {
    const st = document.querySelector(`#${id} .stage`); if (st) st.dataset.aspect = String(a);
  });

  // Lengths in cm to a readable string.
  function fmtLen(cm) {
    if (cm < 0.05 * C.au) return `${P.fmt(cm / 1e5, 3)} km`;
    if (cm < 0.05 * C.ly) return `${P.sci(cm / C.au, 2)} au`;
    if (cm < 1000 * C.pc) return `${P.fmt(cm / C.ly, 3)} ly`;
    return `${P.fmt(cm / C.ly, 3)} ly (${P.fmt(cm / C.pc / 1000, 3)} kpc)`;
  }

  // =====================================================================================
  // 1. Toy sky (2D canvas, its own dark palette).
  // =====================================================================================
  // IAU J2000 equatorial -> galactic rotation matrix (rows give galactic x, y, z).
  const T = [[-0.0548755604, -0.8734370902, -0.4838350155],
             [0.4941094279, -0.4448296300, 0.7469822445],
             [-0.8676661490, -0.1980763734, 0.4559837762]];
  const unit = (lon, lat) => { const a = lon * D2R, d = lat * D2R; return [Math.cos(d) * Math.cos(a), Math.cos(d) * Math.sin(a), Math.sin(d)]; };
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const lb = (v) => { const g = [dot(T[0], v), dot(T[1], v), dot(T[2], v)]; return [Math.atan2(g[1], g[0]) / D2R, Math.asin(Math.max(-1, Math.min(1, g[2]))) / D2R]; };
  const ra = (h, m) => (h + m / 60) * 15;
  const dec = (deg, min) => (deg < 0 ? deg - min / 60 : deg + min / 60);

  function hash2(i, j) { let h = (Math.imul(i, 374761393) + Math.imul(j, 668265263)) | 0; h = Math.imul(h ^ (h >>> 13), 1274126177); return ((h ^ (h >>> 16)) >>> 0) / 4294967296; }
  function vnoise(x, y) {
    const i = Math.floor(x), j = Math.floor(y), fx = x - i, fy = y - j;
    const sx = fx * fx * (3 - 2 * fx), sy = fy * fy * (3 - 2 * fy);
    const a = hash2(i, j), b = hash2(i + 1, j), c = hash2(i, j + 1), d = hash2(i + 1, j + 1);
    return a + (b - a) * sx + (c - a) * sy + (a - b - c + d) * sx * sy;
  }
  // Toy surface brightness of the Milky Way at galactic (l, b) in degrees, plus a dust-lane mask; not a survey map.
  function mwBright(l, b) {
    const n = 0.55 * vnoise(l / 3 + 50, b / 3 + 50) + 0.3 * vnoise(l + 9, b + 9) + 0.15 * vnoise(l * 3, b * 3);
    const width = 3.5 + 3 * Math.exp(-l * l / 800);
    const disc = Math.exp(-Math.abs(l) / 70) * Math.exp(-b * b / (2 * width * width));
    const bulge = 0.9 * Math.exp(-(l * l + 2 * b * b) / 162);
    const lane = (l > -8 && l < 70 ? 1 : 0) * Math.exp(-Math.pow((b - 1.6) / 1.4, 2)) * (0.5 + 0.5 * n) * 0.8;
    return Math.max(0, (disc + bulge) * (0.5 + 0.8 * n) * (1 - lane));
  }

  const M16 = { ra: ra(18, 18.8), dec: dec(-13, 49) };
  // Bright stars at real J2000 positions (approximate), V magnitude, rough colour temperature.
  const STARS = [
    ['Kaus Australis', ra(18, 24.2), dec(-34, 23), 1.85, 9000], ['Kaus Media', ra(18, 21.0), dec(-29, 50), 2.7, 4500],
    ['Kaus Borealis', ra(18, 28.0), dec(-25, 25), 2.8, 4800], ['Nunki', ra(18, 55.3), dec(-26, 18), 2.05, 18000],
    ['Ascella', ra(19, 2.6), dec(-29, 53), 2.6, 9000], ['Alnasl', ra(18, 5.8), dec(-30, 25), 3.0, 4800],
    ['φ Sgr', ra(18, 45.7), dec(-26, 59), 3.2, 12000], ['τ Sgr', ra(19, 6.9), dec(-27, 40), 3.3, 4500],
    ['μ Sgr', ra(18, 13.8), dec(-21, 3), 3.9, 20000], ['Antares', ra(16, 29.4), dec(-26, 26), 1.0, 3500],
    ['Shaula', ra(17, 33.6), dec(-37, 6), 1.6, 25000], ['Sargas', ra(17, 37.3), dec(-43, 0), 1.9, 7000],
    ['Rasalhague', ra(17, 34.9), dec(12, 34), 2.1, 8000], ['Cebalrai', ra(17, 43.5), dec(4, 34), 2.8, 4500],
    ['Sabik', ra(17, 10.4), dec(-15, 43), 2.4, 9000], ['ν Oph', ra(17, 59.0), dec(-9, 46), 3.3, 4800],
    ['η Ser', ra(18, 21.3), dec(-2, 54), 3.3, 4900], ['ξ Ser', ra(17, 37.6), dec(-15, 24), 3.5, 7000],
    ['θ Ser', ra(18, 56.2), dec(4, 12), 4.1, 8000], ['α Sct', ra(18, 35.2), dec(-8, 15), 3.85, 4300],
    ['β Sct', ra(18, 47.2), dec(-4, 45), 4.2, 5000], ['γ Sct', ra(18, 29.2), dec(-14, 34), 4.7, 9500],
    ['Altair', ra(19, 50.8), dec(8, 52), 0.77, 7700], ['δ Aql', ra(19, 25.5), dec(3, 7), 3.4, 7000],
    ['λ Aql', ra(19, 6.2), dec(-4, 53), 3.4, 11000], ['ζ Aql', ra(19, 5.4), dec(13, 52), 3.0, 9500],
  ].map(([name, a, d, m, T0]) => ({ name, v: unit(a, d), m, col: P.bbColor(T0) }));
  const S = Object.fromEntries(STARS.map(s => [s.name, s]));
  const TEAPOT = [['Alnasl', 'Kaus Media'], ['Alnasl', 'Kaus Australis'], ['Kaus Media', 'Kaus Australis'],
    ['Kaus Australis', 'Ascella'], ['Ascella', 'φ Sgr'], ['φ Sgr', 'Kaus Media'], ['Kaus Media', 'Kaus Borealis'],
    ['Kaus Borealis', 'φ Sgr'], ['φ Sgr', 'Nunki'], ['Nunki', 'τ Sgr'], ['τ Sgr', 'Ascella']];
  const DSO = [
    { name: 'M16 · Eagle Nebula', v: unit(M16.ra, M16.dec), a: 0.5, b: 0.35, main: true },
    { name: 'M17', v: unit(ra(18, 20.8), dec(-16, 10)), a: 0.2, b: 0.15 },
    { name: 'M8 Lagoon', v: unit(ra(18, 3.6), dec(-24, 23)), a: 1.5, b: 0.6 },
  ];
  const M24 = unit(ra(18, 17), dec(-18, 33));
  const CONST = [['SAGITTARIUS', ra(18, 50), -32], ['SCUTUM', ra(18, 40), -11], ['SERPENS CAUDA', ra(18, 5), -8],
    ['OPHIUCHUS', ra(17, 20), -4], ['AQUILA', ra(19, 30), 5], ['SCORPIUS', ra(17, 0), -38]];
  // Pillars ~4' south-west of the cluster centre (W205 lies ~2 pc NE of the pillars, McLeod+2015).
  const PILLARS = unit(M16.ra - 0.045, M16.dec - 0.045);

  // Synthetic faint stars in three tiers so every zoom level has a full field (deterministic).
  function makeTier(seed, n, radiusDeg, m0, m1, sigmaB) {
    const r = P.rng(seed), out = [];
    const f = unit(M16.ra, M16.dec), e = [-Math.sin(M16.ra * D2R), Math.cos(M16.ra * D2R), 0];
    const nn = [-Math.sin(M16.dec * D2R) * Math.cos(M16.ra * D2R), -Math.sin(M16.dec * D2R) * Math.sin(M16.ra * D2R), Math.cos(M16.dec * D2R)];
    const k = 0.35, span = Math.pow(10, k * (m1 - m0)) - 1;
    let guard = 0;
    while (out.length < 5 * n && guard++ < n * 40) {
      const x = Math.tan((r() * 2 - 1) * radiusDeg * D2R), y = Math.tan((r() * 2 - 1) * radiusDeg * D2R);
      const v = [f[0] + x * e[0] + y * nn[0], f[1] + x * e[1] + y * nn[1], f[2] + x * e[2] + y * nn[2]];
      const L = Math.hypot(v[0], v[1], v[2]); v[0] /= L; v[1] /= L; v[2] /= L;
      const b = lb(v)[1];
      if (r() > 0.25 + 0.75 * Math.exp(-b * b / (2 * sigmaB * sigmaB))) continue;
      const m = m0 + Math.log10(1 + r() * span) / k;   // number counts rising ~10^(0.35 m)
      out.push(v[0], v[1], v[2], m, r());               // last: colour/twinkle seed
    }
    return new Float32Array(out);
  }
  let tiers = null;
  function buildTiers() {
    const cl = [], r = P.rng(16);   // NGC 6611 members: a compact toy knot of hot stars
    for (let i = 0; i < 40; i++) {
      const rad = 4 / 60 * Math.sqrt(r()), th = r() * 2 * Math.PI;
      const v = unit(M16.ra + rad * Math.cos(th) / Math.cos(M16.dec * D2R), M16.dec + rad * Math.sin(th));
      cl.push(v[0], v[1], v[2], 8.2 + 3.5 * r(), 0.97);
    }
    tiers = [
      { data: makeTier(1, 2600, 50, 3.8, 6.6, 10), maxFov: 999 },
      { data: makeTier(2, 3200, 10, 6.6, 9.8, 6), maxFov: 40 },
      { data: makeTier(3, 2600, 1.2, 9.8, 12.8, 20), maxFov: 6 },
      { data: new Float32Array(cl), maxFov: 6 },
    ];
  }
  // Star colour from a seed: mostly sun-like and cooler, a few hot blue ones (toy distribution).
  const starCols = [3600, 4300, 5000, 5800, 6500, 8000, 11000, 20000].map(t => P.bbColor(t));
  const colOf = (s) => starCols[Math.min(7, Math.floor(Math.pow(s, 0.8) * 8))];
  // Faintest star visible: ~6 naked eye (60 deg), ~9.5 binoculars (7 deg), ~12.5 small telescope (0.5 deg).
  function limMag(fov) {
    const x = Math.log10(fov);
    if (x >= 0.845) return P.lerp(9.5, 6, (x - 0.845) / (1.778 - 0.845));
    return P.lerp(12.5, 9.5, (x + 0.301) / (0.845 + 0.301));
  }
  // Self-playing tour: [time s, field of view deg]; holds and log-eased zooms. Loops every 30 s.
  const TOUR = [[0, 60], [5, 60], [9.5, 7], [15, 7], [19.5, 0.5], [26, 0.5], [30, 60]];
  function tourFov(t) {
    t %= 30;
    for (let i = 1; i < TOUR.length; i++) if (t <= TOUR[i][0]) {
      const [t0, f0] = TOUR[i - 1], [t1, f1] = TOUR[i], u = P.smooth((t - t0) / (t1 - t0));
      return Math.pow(10, P.lerp(Math.log10(f0), Math.log10(f1), u));
    }
    return 60;
  }

  P.demo('fig-where-sky', (ctx) => {
    if (!tiers) buildTiers();
    const c0 = { ra: M16.ra, dec: M16.dec };
    const off = document.createElement('canvas');
    let idle = 99, tourT = 0, manual = false;
    const fovS = ctx.slider({ label: 'Field of view', min: Math.log10(0.4), max: Math.log10(70), step: 0.001, value: Math.log10(60),
      format: (v) => { const f = Math.pow(10, v); return f >= 1 ? `${P.fmt(f, 2)}°` : `${Math.round(f * 60)}′`; },
      onInput: () => { idle = 0; manual = true; } });
    ctx.choice({ label: 'Instrument', value: 60, options: [{ label: 'Naked eye', value: 60 }, { label: 'Binoculars', value: 7 }, { label: 'Telescope', value: 0.5 }],
      onInput: (v) => { fovS.value = Math.log10(v); c0.ra = M16.ra; c0.dec = M16.dec; idle = 0; manual = true; } });
    const moonT = ctx.toggle({ label: 'Moon outline', value: false });
    const hopT = ctx.toggle({ label: 'Star hop', value: false });
    let scale = 1;
    P.drag(ctx.canvas, (dx, dy, x, y, phase) => {
      idle = 0; manual = true;
      if (phase !== 'move') return;
      c0.ra = P.clamp(c0.ra + (dx / scale) / D2R / Math.cos(c0.dec * D2R), M16.ra - 15, M16.ra + 15);
      c0.dec = P.clamp(c0.dec + (dy / scale) / D2R, M16.dec - 15, M16.dec + 15);
    });

    function draw(t, dt) {
      const g = ctx.g, w = ctx.w, h = ctx.h;
      dt = dt || 0; idle += dt;
      // After 6 s untouched the tour resumes from the current zoom level, easing the view back onto M16.
      if (idle > 6) {
        if (manual) { manual = false; const f = Math.pow(10, fovS.value); tourT = f > 20 ? 0 : f > 2 ? 9.5 : 19.5; }
        tourT += dt;
        fovS.value = Math.log10(tourFov(tourT));
        c0.ra += (M16.ra - c0.ra) * Math.min(1, dt * 1.5); c0.dec += (M16.dec - c0.dec) * Math.min(1, dt * 1.5);
      }
      const tt = tourT % 30, touring = idle > 6;
      const fov = Math.pow(10, fovS.value), lim = limMag(fov);
      scale = (w / 2) / Math.tan(fov * D2R / 2);
      const a = c0.ra * D2R, d = c0.dec * D2R;
      const f = [Math.cos(d) * Math.cos(a), Math.cos(d) * Math.sin(a), Math.sin(d)];
      const e = [-Math.sin(a), Math.cos(a), 0];
      const n = [-Math.sin(d) * Math.cos(a), -Math.sin(d) * Math.sin(a), Math.cos(d)];
      // Gnomonic projection, north up, east to the left (as the sky looks facing south).
      const proj = (v) => { const z = dot(v, f); if (z < 0.2) return null; return [w / 2 - dot(v, e) / z * scale, h / 2 - dot(v, n) / z * scale]; };
      const bg = g.createLinearGradient(0, 0, 0, h);
      bg.addColorStop(0, '#03050a'); bg.addColorStop(1, '#0a0d16');
      g.fillStyle = bg; g.fillRect(0, 0, w, h);

      // Milky Way: evaluate the toy brightness on a coarse grid and stretch it smoothly.
      const gw = 110, gh = Math.max(2, Math.round(gw * h / w));
      off.width = gw; off.height = gh;
      const oc = off.getContext('2d'), img = oc.createImageData(gw, gh);
      const fade = P.clamp((Math.log10(fov) + 0.2) / 1.2, 0.3, 1);
      for (let j = 0; j < gh; j++) for (let i = 0; i < gw; i++) {
        const X = (w / 2 - (i + 0.5) * w / gw) / scale, Y = (h / 2 - (j + 0.5) * h / gh) / scale;
        const v = [f[0] + X * e[0] + Y * n[0], f[1] + X * e[1] + Y * n[1], f[2] + X * e[2] + Y * n[2]];
        const L = Math.hypot(v[0], v[1], v[2]); v[0] /= L; v[1] /= L; v[2] /= L;
        const [l, b] = lb(v), B = mwBright(l, b);
        const k = 4 * (j * gw + i);
        img.data[k] = 255; img.data[k + 1] = 228 - 30 * Math.min(1, B); img.data[k + 2] = 196 - 50 * Math.min(1, B);
        img.data[k + 3] = Math.round(255 * P.clamp(B * 0.42 * fade, 0, 0.55));
      }
      oc.putImageData(img, 0, 0);
      g.imageSmoothingEnabled = true; g.drawImage(off, 0, 0, w, h);

      g.globalCompositeOperation = 'lighter';
      // Nebulae as soft H-alpha glows once magnification makes them stand out.
      const nebA = P.clamp((25 - fov) / 20, 0, 1);
      if (nebA > 0) for (const o of DSO) {
        const p = proj(o.v); if (!p) continue;
        const rx = o.a / 2 * D2R * scale, ry = o.b / 2 * D2R * scale;
        if (rx < 0.5) continue;
        g.save(); g.translate(p[0], p[1]); g.scale(1, ry / rx);
        const gr = g.createRadialGradient(0, 0, 0, 0, 0, rx);
        gr.addColorStop(0, `rgba(255,70,70,${0.5 * nebA})`); gr.addColorStop(0.5, `rgba(220,50,60,${0.22 * nebA})`); gr.addColorStop(1, 'rgba(200,40,60,0)');
        g.fillStyle = gr; g.beginPath(); g.arc(0, 0, rx, 0, 2 * Math.PI); g.fill();
        if (o.main && fov < 3) { // [O III] glow nearer the hot cluster
          const g2 = g.createRadialGradient(0, -rx * 0.1, 0, 0, -rx * 0.1, rx * 0.45);
          g2.addColorStop(0, `rgba(80,210,200,${0.18 * nebA})`); g2.addColorStop(1, 'rgba(80,210,200,0)');
          g.fillStyle = g2; g.beginPath(); g.arc(0, -rx * 0.1, rx * 0.45, 0, 2 * Math.PI); g.fill();
        }
        g.restore();
      }

      // Stars, coloured by a toy temperature, twinkling gently in the naked-eye view.
      const cosLim = Math.cos(Math.min(89, fov * 0.9) * D2R);
      const tw = P.clamp((fov - 10) / 30, 0, 1) * 0.35;
      const star = (x, y, m, col, seed) => {
        let al = P.clamp(0.75 + 0.5 * (lim - m), 0, 1); if (al <= 0) return;
        al *= 1 - tw * (0.5 + 0.5 * Math.sin(t * (3 + 5 * seed) + seed * 40));
        const r = P.clamp(0.7 + 0.5 * (lim - m), 0.6, 4.5);
        g.fillStyle = `rgba(${col[0]},${col[1]},${col[2]},${al})`;
        if (r < 1.1) { g.fillRect(x - r, y - r, 2 * r, 2 * r); return; }
        g.beginPath(); g.arc(x, y, r, 0, 2 * Math.PI); g.fill();
        if (r > 2.2) { // soft halo for the brightest
          const gr = g.createRadialGradient(x, y, 0, x, y, r * 3.5);
          gr.addColorStop(0, `rgba(${col[0]},${col[1]},${col[2]},${0.3 * al})`); gr.addColorStop(1, `rgba(${col[0]},${col[1]},${col[2]},0)`);
          g.fillStyle = gr; g.beginPath(); g.arc(x, y, r * 3.5, 0, 2 * Math.PI); g.fill();
        }
      };
      for (const tr of tiers) {
        if (fov > tr.maxFov) continue;
        const D = tr.data;
        for (let i = 0; i < D.length; i += 5) {
          if (D[i + 3] > lim + 0.5) continue;
          const v = [D[i], D[i + 1], D[i + 2]];
          if (dot(v, f) < cosLim) continue;
          const p = proj(v); if (!p || p[0] < -5 || p[0] > w + 5 || p[1] < -5 || p[1] > h + 5) continue;
          star(p[0], p[1], D[i + 3], D[i + 4] > 0.96 ? starCols[7] : colOf(D[i + 4]), D[i + 4]);
        }
      }
      STARS.forEach((s, i) => { const p = proj(s.v); if (p) star(p[0], p[1], Math.min(s.m, lim - 1), s.col, i / 26); });
      g.globalCompositeOperation = 'source-over';

      // Pillars: three dark fingers pointing at the cluster (toy shapes, true angular size ~2.6').
      const pilA = P.clamp((1.6 - fov) / 0.8, 0, 1);
      const pp = proj(PILLARS), pc = proj(DSO[0].v);
      if (pilA > 0 && pp && pc) {
        const ux = pc[0] - pp[0], uy = pc[1] - pp[1], ul = Math.hypot(ux, uy) || 1;
        const u = [ux / ul, uy / ul], q = [-u[1], u[0]];
        const L = 2.6 / 60 * D2R * scale;
        g.fillStyle = `rgba(14,8,8,${0.85 * pilA})`; g.strokeStyle = `rgba(255,150,120,${0.6 * pilA})`; g.lineWidth = 1;
        [[-0.32, 1.0, 0.09], [0, 0.72, 0.07], [0.25, 0.32, 0.05]].forEach(([o, len, wd]) => {
          const bx = pp[0] + q[0] * o * L - u[0] * 0.5 * L, by = pp[1] + q[1] * o * L - u[1] * 0.5 * L;
          const tx = bx + u[0] * len * L, ty = by + u[1] * len * L;
          g.beginPath();
          g.moveTo(bx + q[0] * wd * L * 1.4, by + q[1] * wd * L * 1.4);
          g.lineTo(tx + q[0] * wd * L * 0.6, ty + q[1] * wd * L * 0.6);
          g.quadraticCurveTo(tx + u[0] * wd * L, ty + u[1] * wd * L, tx - q[0] * wd * L * 0.6, ty - q[1] * wd * L * 0.6);
          g.lineTo(bx - q[0] * wd * L * 1.4, by - q[1] * wd * L * 1.4);
          g.closePath(); g.fill(); g.stroke();
        });
      }

      // Teapot asterism, constellation names and labels.
      g.font = mono(11); g.textBaseline = 'middle';
      const lineA = P.clamp((fov - 12) / 10, 0, 1);
      if (lineA > 0) {
        g.strokeStyle = `rgba(170,190,220,${0.35 * lineA})`; g.lineWidth = 1; g.beginPath();
        for (const [a1, b1] of TEAPOT) { const p = proj(S[a1].v), q = proj(S[b1].v); if (p && q) { g.moveTo(p[0], p[1]); g.lineTo(q[0], q[1]); } }
        g.stroke();
        g.fillStyle = `rgba(170,190,220,${0.6 * lineA})`; g.textAlign = 'center';
        for (const [name, a1, d1] of CONST) { const p = proj(unit(a1, d1)); if (p) g.fillText(name, p[0], p[1]); }
      }
      g.textAlign = 'left';
      const named = (v, text, col) => { const p = proj(v); if (p && p[0] > -40 && p[0] < w && p[1] > 0 && p[1] < h) { g.fillStyle = col; g.fillText(text, p[0] + 7, p[1] - 7); } return p; };
      if (fov > 3 && fov < 40) for (const s of STARS) if (s.m < 3.5 || fov < 15) named(s.v, s.name, 'rgba(200,205,215,0.65)');
      if (fov < 30) {
        for (const o of DSO) named(o.v, o.name, o.main ? '#f2efe8' : 'rgba(200,205,215,0.8)');
        named(M24, 'M24 star cloud', 'rgba(200,205,215,0.8)');
      }
      if (fov < 1.6) {
        if (pc) { g.fillStyle = 'rgba(200,215,240,0.85)'; g.fillText('NGC 6611 cluster', pc[0] + 12, pc[1] + 12); }
        if (pp) { g.fillStyle = '#ffd9cc'; g.textAlign = 'right'; g.fillText('the Pillars', pp[0] - 10 - L2px(scale) * 0.4, pp[1] + 14); g.textAlign = 'left'; }
      }
      // Pulsing marker on M16 while it is too small to see.
      const pm = proj(DSO[0].v);
      if (pm && fov > 3) {
        const ph = (t * 0.8) % 1;
        g.strokeStyle = `rgba(95,208,196,${1 - ph})`; g.lineWidth = 1.5; g.beginPath(); g.arc(pm[0], pm[1], 8 + 14 * ph, 0, 2 * Math.PI); g.stroke();
        g.strokeStyle = '#5fd0c4'; g.beginPath(); g.arc(pm[0], pm[1], 8, 0, 2 * Math.PI); g.stroke();
      }
      // Star hop, drawn progressively during the naked-eye part of the tour (or on demand).
      const hopFrac = hopT.value ? 1 : touring ? P.clamp((tt - 0.5) / 3.5, 0, 1) * P.clamp((9.5 - tt) / 1.5, 0, 1) : 0;
      if (hopFrac > 0) {
        const pts = [S['Kaus Borealis'].v, S['μ Sgr'].v, M24, DSO[1].v, DSO[0].v].map(proj);
        if (pts.every(Boolean)) {
          let total = 0; const segs = [];
          for (let i = 1; i < pts.length; i++) { const l = Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]); segs.push(l); total += l; }
          let left = total * (hopT.value ? 1 : Math.min(1, (tt - 0.5) / 3.5));
          g.strokeStyle = `rgba(95,208,196,${hopT.value ? 1 : Math.min(1, hopFrac * 3)})`; g.lineWidth = 1.6; g.setLineDash([5, 4]);
          g.beginPath(); g.moveTo(pts[0][0], pts[0][1]);
          for (let i = 1; i < pts.length && left > 0; i++) {
            const u = Math.min(1, left / segs[i - 1]); left -= segs[i - 1];
            g.lineTo(P.lerp(pts[i - 1][0], pts[i][0], u), P.lerp(pts[i - 1][1], pts[i][1], u));
          }
          g.stroke(); g.setLineDash([]);
        }
      }
      // Full-Moon outline for scale, shown in the telescope part of the tour (or on demand).
      const moonA = moonT.value ? 1 : touring ? P.clamp((tt - 20.5) / 1, 0, 1) * P.clamp((26 - tt) / 1, 0, 1) : 0;
      if (moonA > 0 && pm) {
        const r = 0.259 * D2R * scale;   // the Moon's mean angular radius, ~0.26 deg
        g.strokeStyle = `rgba(240,236,228,${0.8 * moonA})`; g.setLineDash([3, 4]); g.lineWidth = 1.2;
        g.beginPath(); g.arc(pm[0], pm[1], Math.max(r, 2), 0, 2 * Math.PI); g.stroke(); g.setLineDash([]);
        if (r > 12) { g.fillStyle = `rgba(240,236,228,${moonA})`; g.textAlign = 'center'; g.fillText('size of the full Moon', pm[0], pm[1] - r - 10); g.textAlign = 'left'; }
      }

      const kind = fov > 20 ? 'naked eye' : fov > 2 ? 'binoculars' : 'small telescope';
      const fl = fov >= 1 ? `${P.fmt(fov, 2)}°` : `${Math.round(fov * 60)}′`;
      g.textAlign = 'left'; P.hud(g, 12, 12, [['view', kind], ['field', fl], ['faintest star', `mag ${lim.toFixed(1)}`], ['full Moons across', P.fmt(fov / 0.518, 2)]], { px: hudPx(w) });
      g.font = mono(10); g.fillStyle = 'rgba(200,205,215,0.55)'; g.textAlign = 'right'; g.textBaseline = 'bottom';
      g.fillText(touring ? 'toy sky · drag to explore' : 'toy sky · tour resumes when idle', w - 10, h - 8);
      g.textAlign = 'left';
    }
    const L2px = (sc) => 2.6 / 60 * D2R * sc;
    return { draw, animate: true };
  });

  // =====================================================================================
  // Shared 3D bits.
  // =====================================================================================
  // Additive point sprites whose size is given in world units (pc). On screen: px = size * uPx / depth,
  // with uPx = viewport height / (2 tan(fov/2)). Below uMinPx a point keeps uMinPx and dims (dimSmall) or not;
  // above uMaxPx it is capped and faded so near particles do not blow up.
  function pointsMat(THREE, o = {}) {
    return new THREE.ShaderMaterial({
      uniforms: { uPx: { value: 500 }, uK: { value: 1 }, uMinPx: { value: o.minPx ?? 1.2 }, uMaxPx: { value: o.maxPx ?? 40 }, uOpacity: { value: o.opacity ?? 1 }, uDim: { value: o.dimSmall ?? 1 } },
      vertexShader: `attribute float aSize; attribute vec3 aColor; uniform float uPx, uK, uMinPx, uMaxPx, uOpacity, uDim;
        varying vec3 vC; varying float vA;
        void main(){ vec4 mv = modelViewMatrix * vec4(position, 1.0); float d = max(-mv.z, 1e-30);
          float px = aSize * uK * uPx / d; vA = uOpacity;
          if (px < uMinPx) { vA *= mix(1.0, px / uMinPx, uDim); px = uMinPx; }
          if (px > uMaxPx) { vA *= uMaxPx / px; px = uMaxPx; }
          if (mv.z > 0.0) vA = 0.0;
          vC = aColor; gl_PointSize = px; gl_Position = projectionMatrix * mv; }`,
      fragmentShader: `varying vec3 vC; varying float vA;
        void main(){ vec2 q = gl_PointCoord - 0.5; float r2 = dot(q, q) * 4.0; if (r2 > 1.0) discard;
          float a = exp(-r2 * 4.5) + 0.25 * exp(-r2 * 1.2); gl_FragColor = vec4(vC * a * vA, 1.0); }`,
      blending: o.normal ? THREE.NormalBlending : THREE.AdditiveBlending, transparent: true, depthTest: false, depthWrite: false,
    });
  }
  function pointCloud(THREE, n, fill, mo) {
    const pos = new Float32Array(3 * n), col = new Float32Array(3 * n), size = new Float32Array(n);
    for (let i = 0; i < n; i++) fill(i, pos, col, size);
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    geo.setAttribute('aColor', new THREE.BufferAttribute(col, 3));
    geo.setAttribute('aSize', new THREE.BufferAttribute(size, 1));
    const pts = new THREE.Points(geo, pointsMat(THREE, mo));
    pts.frustumCulled = false;
    return pts;
  }
  const gauss = (r) => Math.sqrt(-2 * Math.log(Math.max(1e-9, r()))) * Math.cos(2 * Math.PI * r());
  const bb = (T0, k = 1) => P.bbColor(T0).map(c => c / 255 * k);
  // Galactic (l, b, distance pc) to scene coordinates: x toward the Galactic centre, y toward the north Galactic pole.
  const galXYZ = (l, b, d) => [d * Math.cos(b * D2R) * Math.cos(l * D2R), d * Math.sin(b * D2R), -d * Math.cos(b * D2R) * Math.sin(l * D2R)];
  // Shaded sphere: lit from a fixed direction (toward the Sun), dark side faintly visible.
  function litSphere(THREE, r, c1, c0, sunDir) {
    return new THREE.Mesh(new THREE.SphereGeometry(r, 48, 32), new THREE.ShaderMaterial({
      uniforms: { uS: { value: new THREE.Vector3(...sunDir).normalize() }, c1: { value: new THREE.Color(...c1) }, c0: { value: new THREE.Color(...c0) } },
      vertexShader: 'varying vec3 vN, vV; void main(){ vN = normalize(mat3(modelMatrix) * normal); vV = normalize(normalMatrix * normal); gl_Position = projectionMatrix * modelViewMatrix * vec4(position,1.0); }',
      fragmentShader: 'uniform vec3 uS, c1, c0; varying vec3 vN, vV; void main(){ float k = dot(vN, uS); float rim = pow(1.0 - abs(vV.z), 3.0); vec3 c = mix(c0, c1, smoothstep(-0.1, 0.7, k)); c += c1 * rim * 0.8 * smoothstep(-0.3, 0.3, k); gl_FragColor = vec4(c, 1.0); }',
      depthTest: false, depthWrite: false,
    }));
  }

  // =====================================================================================
  // 2. Powers-of-ten flight from Earth to the Eagle, with light-travel photons.
  // =====================================================================================
  // Scene unit = 1 pc, Earth at the origin. The camera's distance is eased in log10 so every power of ten
  // takes the same screen time. Near/far planes follow the camera (no depth test), so 16 decades fit.
  const AU = 1 / AU_PER_PC;                        // pc
  const KM = 1e5 / C.pc;                           // pc
  const EAGLE = galXYZ(16.95, 0.79, D_PC);         // l, b from facts.md
  const GC = [8200, -20, 0];                       // Galactic centre ~8.2 kpc away; Sun ~20 pc above the midplane (Bennett & Bovy 2019)
  const FLIGHT_T = 50;

  P.demo3d('fig-where-flight', (ctx, THREE) => {
    const scene = new THREE.Scene(), sky = new THREE.Scene();
    const cam = new THREE.PerspectiveCamera(50, 1, 1e-16, 1e5), skyCam = new THREE.PerspectiveCamera(50, 1, 0.01, 10);
    // Everything lives in `root`, rescaled each frame by k = 1/(camera distance) so the camera always sits
    // ~1 unit from its target. This keeps GPU float32 precision comfortable across 16 decades of scale.
    const root = new THREE.Group(); scene.add(root);
    const mats = [], skyMats = [];
    const add = (sc, obj) => { (sc === scene ? root : sc).add(obj); if (obj.material && obj.material.uniforms && obj.material.uniforms.uPx) (sc === sky ? skyMats : mats).push(obj.material); return obj; };
    const r = P.rng(1740);

    // Background sky at infinity (rotation only): stars plus a Milky Way band in the y=0 plane.
    const skyPts = add(sky, pointCloud(THREE, 9000, (i, pos, col, size) => {
      let x, y, z;
      if (i < 5500) { const lo = 2 * Math.PI * r(); const la = gauss(r) * 0.12; x = Math.cos(la) * Math.cos(lo); y = Math.sin(la); z = Math.cos(la) * Math.sin(lo); }
      else { const u = 2 * r() - 1, th = 2 * Math.PI * r(); x = Math.sqrt(1 - u * u) * Math.cos(th); y = u; z = Math.sqrt(1 - u * u) * Math.sin(th); }
      pos.set([x, y, z], 3 * i);
      const k = i < 5500 ? 0.25 + 0.3 * r() : 0.5 + 0.5 * r();
      col.set(bb(3500 + 9000 * Math.pow(r(), 2), k * (i < 5500 ? 0.5 : 0.8)), 3 * i);
      size[i] = (i < 5500 ? 0.006 : 0.0025) * (0.5 + r());
    }, { minPx: 1, maxPx: 6 }));

    // Earth, Moon, Sun.
    const sunDir = [-1, 0, 0];
    // Earth: procedural toy continents and clouds (value-noise fbm on the sphere), day/night terminator, blue rim.
    const earth = new THREE.Mesh(new THREE.SphereGeometry(6371 * KM, 96, 64), new THREE.ShaderMaterial({
      uniforms: { uS: { value: new THREE.Vector3(...sunDir) }, uT: { value: 0 } },
      vertexShader: 'varying vec3 vP, vN, vV; void main(){ vP = normalize(position); vN = normalize(mat3(modelMatrix) * normal); vV = normalize(normalMatrix * normal); gl_Position = projectionMatrix * modelViewMatrix * vec4(position,1.0); }',
      fragmentShader: `uniform vec3 uS; uniform float uT; varying vec3 vP, vN, vV;
        float h(vec3 p){ p = fract(p*0.3183099+0.1); p *= 17.0; return fract(p.x*p.y*p.z*(p.x+p.y+p.z)); }
        float n3(vec3 x){ vec3 i = floor(x), f = fract(x); f = f*f*(3.0-2.0*f);
          return mix(mix(mix(h(i),h(i+vec3(1,0,0)),f.x),mix(h(i+vec3(0,1,0)),h(i+vec3(1,1,0)),f.x),f.y),
                     mix(mix(h(i+vec3(0,0,1)),h(i+vec3(1,0,1)),f.x),mix(h(i+vec3(0,1,1)),h(i+vec3(1,1,1)),f.x),f.y),f.z); }
        float fbm(vec3 p){ float a=0.5,s=0.0; for(int i=0;i<5;i++){ s+=a*n3(p); p=p*2.02+3.1; a*=0.5; } return s; }
        void main(){
          vec3 p = vP; float c = cos(uT*0.05), sn = sin(uT*0.05); p.xz = mat2(c,-sn,sn,c) * p.xz;
          float land = smoothstep(0.50, 0.53, fbm(p*2.2 + 1.3));
          float ice = smoothstep(0.82, 0.9, abs(vP.y));
          vec3 col = mix(vec3(0.02,0.09,0.25), mix(vec3(0.13,0.3,0.1), vec3(0.45,0.38,0.24), fbm(p*6.0)), land);
          col = mix(col, vec3(0.9), ice);
          float cl = smoothstep(0.52, 0.72, fbm(p*3.5 + vec3(uT*0.01, 0.0, 7.0)));
          col = mix(col, vec3(0.95), cl*0.85);
          float k = dot(vN, uS); float day = smoothstep(-0.12, 0.25, k);
          vec3 night = vec3(1.0,0.75,0.4) * land * (1.0-cl) * step(0.62, n3(p*40.0)) * 0.25;
          float rim = pow(1.0 - abs(vV.z), 3.0);
          gl_FragColor = vec4(col*day + night*(1.0-day) + vec3(0.3,0.55,1.0)*rim*smoothstep(-0.3,0.4,k), 1.0); }`,
      depthTest: false, depthWrite: false,
    }));
    root.add(earth);
    const moonPos = [0, 384400 * KM * 0.4, 384400 * KM * 0.92];
    const moon = litSphere(THREE, 1737 * KM, [0.75, 0.74, 0.72], [0.03, 0.03, 0.03], sunDir); moon.position.set(...moonPos); root.add(moon);
    const sunPos = [-AU, 0, 0];
    const sun = new THREE.Mesh(new THREE.SphereGeometry(696000 * KM, 32, 16), new THREE.MeshBasicMaterial({ color: 0xfff1d6, depthTest: false }));
    sun.position.set(...sunPos); root.add(sun);
    // Glow markers that never vanish: Sun (bright) and Earth (blue dot).
    add(scene, pointCloud(THREE, 2, (i, pos, col, size) => {
      pos.set(i ? [0, 0, 0] : sunPos, 3 * i); col.set(i ? [0.35, 0.6, 1] : [1.6, 1.35, 1.0], 3 * i); size[i] = i ? 6371 * KM * 3 : 696000 * KM * 6;
    }, { minPx: 7, maxPx: 30, dimSmall: 0 }));

    // Planet orbits around the Sun, in the ecliptic, tilted ~60 deg to the Galactic plane.
    const solar = new THREE.Group(); solar.position.set(...sunPos); solar.rotation.x = 60.2 * D2R; root.add(solar);
    const orbitMats = [];
    [0.387, 0.723, 1, 1.524, 5.2, 9.58, 19.2, 30.07].forEach((a, k) => {
      const pts = []; for (let i = 0; i < 256; i++) { const th = i / 256 * 2 * Math.PI; pts.push(new THREE.Vector3(a * AU * Math.cos(th), 0, a * AU * Math.sin(th))); }
      const m = new THREE.LineBasicMaterial({ color: k === 2 ? 0x5fa0ff : 0x8a93a8, transparent: true, opacity: 0.5, depthTest: false });
      m.userData.a = a * AU; orbitMats.push(m);
      solar.add(new THREE.LineLoop(new THREE.BufferGeometry().setFromPoints(pts), m));
    });
    // Kuiper belt (30-50 au) and Oort cloud (~2,000 to ~100,000 au, estimated; NASA).
    const belts = add(solar, pointCloud(THREE, 6000, (i, pos, col, size) => {
      const kuiper = i < 1200;
      let rr, x, y, z;
      if (kuiper) { rr = (30 + 20 * r()) * AU; const th = 2 * Math.PI * r(); x = rr * Math.cos(th); z = rr * Math.sin(th); y = gauss(r) * rr * 0.05; }
      else { rr = Math.pow(10, P.lerp(Math.log10(2000), 5, Math.pow(r(), 0.7))) * AU; const u = 2 * r() - 1, th = 2 * Math.PI * r(); x = rr * Math.sqrt(1 - u * u) * Math.cos(th); y = rr * u * 0.85; z = rr * Math.sqrt(1 - u * u) * Math.sin(th); }
      pos.set([x, y, z], 3 * i); col.set(kuiper ? [0.5, 0.55, 0.65] : [0.45, 0.6, 0.85], 3 * i); size[i] = rr * (kuiper ? 0.03 : 0.025);
    }, { minPx: 1.2, maxPx: 5 }));

    // Nearby stars: a few real ones (Galactic l, b, d) plus a toy field at ~the local density.
    const NAMED = [['Proxima Centauri', 313.9, -1.9, 1.30, 3000], ['α Centauri', 315.7, -0.7, 1.34, 5800], ['Barnard\'s Star', 31.0, 14.1, 1.83, 3200],
      ['Sirius', 227.2, -8.9, 2.64, 9900], ['Altair', 47.7, -8.9, 5.13, 7700], ['Vega', 67.4, 19.2, 7.68, 9600]];
    const near = add(scene, pointCloud(THREE, 2200, (i, pos, col, size) => {
      if (i < NAMED.length) { const [, l, b, d, T0] = NAMED[i]; pos.set(galXYZ(l, b, d), 3 * i); col.set(bb(T0, 1.4), 3 * i); size[i] = 0.06; return; }
      const d = 2 + 28 * Math.cbrt(r()), u = 2 * r() - 1, th = 2 * Math.PI * r();
      pos.set([d * Math.sqrt(1 - u * u) * Math.cos(th), d * u, d * Math.sqrt(1 - u * u) * Math.sin(th)], 3 * i);
      col.set(bb(3000 + 8000 * Math.pow(r(), 3), 0.9), 3 * i); size[i] = 0.025 * (0.5 + r());
    }, { minPx: 1.4, maxPx: 10 }));

    // Galactic disc: field stars within ~1.5 kpc and schematic spiral arms (log spirals, pitch 12 deg),
    // with the Sagittarius arm passing through the Eagle (~6.5 kpc from the centre).
    const eg = new THREE.Vector3(...EAGLE).sub(new THREE.Vector3(...GC));
    const phiE = Math.atan2(eg.z, eg.x), RE = Math.hypot(eg.x, eg.z), cotp = 1 / Math.tan(12 * D2R);
    const disc = add(scene, pointCloud(THREE, 24000, (i, pos, col, size) => {
      let x, y, z, c, s;
      if (i < 9000) { // field stars around the Sun
        const rr = 1500 * Math.sqrt(r()), th = 2 * Math.PI * r();
        x = rr * Math.cos(th); z = rr * Math.sin(th); y = -20 + gauss(r) * 90;
        c = bb(3500 + 9000 * Math.pow(r(), 2.5), 1.3); s = 0.6 + 1.0 * r();
      } else if (i < 11000) { // bulge
        const rr = 1400 * Math.abs(gauss(r)), u = 2 * r() - 1, th = 2 * Math.PI * r();
        x = GC[0] + rr * Math.sqrt(1 - u * u) * Math.cos(th); y = GC[1] + rr * u * 0.55; z = rr * Math.sqrt(1 - u * u) * Math.sin(th);
        c = [1.0, 0.8, 0.55].map(v => v * 0.35); s = 80 + 100 * r();
      } else { // arms; one in five particles is an H II region (H-alpha pink) or young blue stars
        const local = i < 12500;
        const R = local ? 7600 + 1600 * r() : 3000 + 11000 * Math.pow(r(), 1.3);
        const arm = local ? 0 : Math.floor(r() * 4);
        const phi = local ? Math.PI + (R - 8200) / 8200 * 1.6 + gauss(r) * 0.03
          : phiE + Math.log(R / RE) * cotp + arm * Math.PI / 2 + gauss(r) * 0.05;
        const rj = R + gauss(r) * 180;
        x = GC[0] + rj * Math.cos(phi); z = GC[2] + rj * Math.sin(phi); y = GC[1] + gauss(r) * 70;
        const k = r();
        c = k < 0.12 ? [1.0, 0.25, 0.32] : k < 0.3 ? [0.55, 0.7, 1.0] : [0.95, 0.82, 0.65].map(v => v * 0.6);
        c = c.map(v => v * (local ? 0.6 : 0.85)); s = (local ? 10 : 25) + 35 * r();
      }
      pos.set([x, y, z], 3 * i); col.set(c, 3 * i); size[i] = s;
    }, { minPx: 1.0, maxPx: 12 }));

    // The Eagle: H-alpha nebula (~15 pc bright part), [O III] glow near the cluster, hot cluster stars,
    // and three dark dust columns for the Pillars (toy shapes at true size, tips toward the cluster).
    const E = new THREE.Vector3(...EAGLE), clusterOff = new THREE.Vector3(1.4, 1.2, 0.6);
    const neb = add(scene, pointCloud(THREE, 5000, (i, pos, col, size) => {
      let p, c, s;
      if (i < 4200) { const o = i < 3200; const rr = o ? 6 : 2.5; p = [gauss(r) * rr, gauss(r) * rr * 0.75, gauss(r) * rr];
        if (!o) p = p.map((v, k) => v + clusterOff.getComponent(k));
        c = o ? [1.0, 0.22, 0.2].map(v => v * (0.05 + 0.07 * r())) : [0.25, 0.8, 0.75].map(v => v * 0.06); s = 0.5 + 1.0 * r(); }
      else { p = [gauss(r) * 0.8, gauss(r) * 0.8, gauss(r) * 0.8].map((v, k) => v + clusterOff.getComponent(k)); c = bb(30000, 1.6); s = 0.05; }
      pos.set([E.x + p[0], E.y + p[1], E.z + p[2]], 3 * i); col.set(c, 3 * i); size[i] = s;
    }, { minPx: 1.0, maxPx: 70 }));
    const pillars = add(scene, pointCloud(THREE, 1500, (i, pos, col, size) => {
      const k = i % 3, len = [1.3, 1.0, 0.4][k], wid = [0.13, 0.1, 0.06][k], xo = [-0.5, 0.1, 0.6][k], zo = [-0.4, 0, 0.4][k];
      const u = r(), dir = clusterOff.clone().normalize();
      const base = new THREE.Vector3(xo, -0.8, zo);
      const tip = base.clone().add(new THREE.Vector3(0, len, 0).lerp(dir.multiplyScalar(len), 0.35));
      const q = base.lerp(tip, u);
      pos.set([E.x + q.x + gauss(r) * wid * (1 - 0.5 * u), E.y + q.y + gauss(r) * wid * 0.5, E.z + q.z + gauss(r) * wid * (1 - 0.5 * u)], 3 * i);
      col.set([0.06, 0.035, 0.03], 3 * i); size[i] = 0.05 + 0.05 * r();
    }, { minPx: 1.0, maxPx: 50, normal: true, opacity: 0.35 }));
    pillars.renderOrder = 2;

    // Photons from the Eagle to Earth. Half are spaced evenly in log(distance) and fall inward at a constant
    // logarithmic rate so a stream is visible at every scale; half travel linearly from the nebula. Toy timing.
    const NP = 3000, pr = new Float32Array(NP), plat = new Float32Array(2 * NP);
    const uE = E.clone().normalize(), side1 = new THREE.Vector3(0, 1, 0).cross(uE).normalize(), side2 = uE.clone().cross(side1);
    const R_E = 6371 * KM;
    const spawn = (i, rr) => { pr[i] = rr; const a = 2 * Math.PI * r(), m = Math.sqrt(r()); plat[2 * i] = m * Math.cos(a); plat[2 * i + 1] = m * Math.sin(a); };
    for (let i = 0; i < NP; i++) spawn(i, i % 2 ? D_PC * r() : Math.pow(10, P.lerp(Math.log10(R_E), Math.log10(D_PC), r())));
    const photons = add(scene, pointCloud(THREE, NP, (i, pos, col, size) => { col.set([1.0, 0.55, 0.42], 3 * i); size[i] = 1e-25; }, { minPx: 2.6, maxPx: 2.6, dimSmall: 0 }));   // fixed 2.6 px sprites
    const phPos = photons.geometry.attributes.position;
    function stepPhotons(dt) {
      for (let i = 0; i < NP; i++) {
        let rr = pr[i];
        rr = i % 2 ? rr - D_PC / 14 * dt : rr * Math.exp(-0.55 * dt);
        if (rr < R_E * 0.9) spawn(i, D_PC * (0.995 + 0.005 * r())); else pr[i] = rr;
        rr = pr[i];
        const lat = R_E * 1.4 + rr * 0.004;      // beam: Earth-sized at Earth, nebula-sized at the Eagle
        const a = plat[2 * i] * lat, b = plat[2 * i + 1] * lat;
        phPos.setXYZ(i, uE.x * rr + side1.x * a + side2.x * b, uE.y * rr + side1.y * a + side2.y * b, uE.z * rr + side1.z * a + side2.z * b);
      }
      phPos.needsUpdate = true;
    }

    // Flight script: log10(camera distance / pc) and look target (0 = Earth, 1 = Eagle) against time.
    // Start ~20,000 km from Earth (Earth radius 2.1e-10 pc), end ~14 kpc out, then dive to ~25 pc from the Eagle.
    const KEYS = [[0, -8.95, 0], [3, -8.95, 0], [27, 4.15, 0], [31, 4.15, 0], [39, 1.6, 1], [47, 1.4, 1], [50, 1.4, 1]];
    function flight(t) {
      for (let i = 1; i < KEYS.length; i++) if (t <= KEYS[i][0]) {
        const [t0, L0, a0] = KEYS[i - 1], [t1, L1, a1] = KEYS[i], u = P.smooth((t - t0) / (t1 - t0));
        return [P.lerp(L0, L1, u), P.lerp(a0, a1, P.smooth(u))];
      }
      return [KEYS[KEYS.length - 1][1], 1];
    }
    const STAGES = [[-7.4, 'Earth and Moon'], [-4.4, 'the inner Solar System'], [-2.6, 'the planets end here'], [-0.2, 'the Oort cloud'],
      [1.4, 'the nearest stars'], [3.0, 'our neighbourhood of the disc'], [99, 'the Milky Way\'s spiral arms']];
    const orb = P.orbit(ctx, cam, { yaw: -0.9, pitch: 0.42, dist: 1, auto: 0.025 });
    const tS = ctx.slider({ label: 'Flight', min: 0, max: FLIGHT_T, step: 0.01, value: 0, format: (v) => `${Math.round(v / FLIGHT_T * 100)}%`, onInput: (v) => { clock = v; } });
    const pause = ctx.toggle({ label: 'Pause', value: false });
    let clock = 0, L = -8.95, aim = 0, fadeA = 0, k = 1;
    const tmp = new THREE.Vector3();
    const labels = [
      ['Earth', () => [0, 0, 0], -10, -3.2], ['Moon', () => moonPos, -9.3, -6.8], ['Sun', () => sunPos, -6.6, -2.4],
      ['Neptune\'s orbit', () => { tmp.set(30.07 * AU, 0, 0).applyEuler(solar.rotation).add(solar.position); return tmp.toArray(); }, -4.3, -2.4],
      ['Oort cloud (estimated)', () => { tmp.set(0, 0.6e5 * AU, 0.8e5 * AU).add(solar.position); return tmp.toArray(); }, -2.3, 0.2],
      ...NAMED.map(([n, l, b, d]) => [n, () => galXYZ(l, b, d), n === 'Vega' || n === 'Altair' ? 0.3 : -0.6, 1.5]),
      ['the Sun is here', () => [0, 0, 0], 1.6, 4.3], ['Galactic centre', () => GC, 3.2, 4.3],
      ['Sagittarius arm', () => { const R = 5200, p = phiE + Math.log(R / RE) * cotp; return [GC[0] + R * Math.cos(p), GC[1], GC[2] + R * Math.sin(p)]; }, 3.3, 4.3],
      ['Eagle Nebula', () => EAGLE, 1.6, 9], ['NGC 6611', () => E.clone().add(clusterOff).toArray(), 0, 2.0, true], ['the Pillars', () => EAGLE.map((v, j) => v + [0, -0.4, 0][j]), 0, 1.9, true],
    ];
    const proj = new THREE.Vector3();

    return {
      scale: 1,
      update(t, dt, pw, ph) {
        dt = dt || 0;
        if (!pause.value) { clock += dt; if (clock > FLIGHT_T) clock = 0; tS.value = clock; }
        [L, aim] = flight(clock);
        const dist = Math.pow(10, L);
        k = 1 / dist;
        root.scale.setScalar(k);
        orb.dist = 1;
        orb.target.set(...EAGLE).multiplyScalar(aim * k);
        orb.step(dt);
        cam.aspect = ctx.w / ctx.h;
        cam.near = 1e-4; cam.far = 1e7; cam.updateProjectionMatrix();
        cam.updateMatrixWorld();
        skyCam.quaternion.copy(cam.quaternion); skyCam.aspect = cam.aspect; skyCam.updateProjectionMatrix();
        const uPx = ph / (2 * Math.tan(cam.fov * D2R / 2));
        for (const m of mats) { m.uniforms.uPx.value = uPx; m.uniforms.uK.value = k; }
        for (const m of skyMats) m.uniforms.uPx.value = uPx;
        // Fade layers in and out by scale.
        skyPts.material.uniforms.uOpacity.value = 1 - P.smooth(P.clamp((L - 1.2) / 1.6, 0, 1));
        orbitMats.forEach(m => { const px = m.userData.a / dist * uPx; m.opacity = 0.55 * P.clamp((px - 3) / 10, 0, 1) * P.clamp((dist / m.userData.a - 0.08) * 4, 0, 1); });
        belts.material.uniforms.uOpacity.value = P.clamp((L + 6) / 1.5, 0, 1) * (1 - P.clamp((L - 0.6) / 0.8, 0, 1));
        near.material.uniforms.uOpacity.value = 1 - P.clamp((L - 2.2) / 0.8, 0, 1);
        disc.material.uniforms.uOpacity.value = P.clamp((L - 1.6) / 1.2, 0, 1) * (1 - 0.85 * aim);
        photons.material.uniforms.uOpacity.value = 0.9;
        fadeA = P.clamp((clock - (FLIGHT_T - 2)) / 2, 0, 1) + P.clamp(1 - clock / 1.2, 0, 1);
        stepPhotons(pause.value ? 0 : dt);
        earth.material.uniforms.uT.value = t;
      },
      renderTo(r, pw, ph) {
        r.setClearColor(0x020308, 1); r.clear();
        r.autoClear = false;
        r.render(sky, skyCam);
        r.render(scene, cam);
        r.autoClear = true;
      },
      overlay(g, w, h) {
        // Labels at projected positions, shown only in their range of scales.
        g.font = mono(w < 520 ? 10 : 11); g.textBaseline = 'middle';
        for (const [name, at, lo, hi, eagleOnly] of labels) {
          if (L < lo || L > hi || (eagleOnly && aim < 0.6)) continue;
          const a = Math.min(1, (L - lo) / 0.4, (hi - L) / 0.4);
          proj.set(...at()).multiplyScalar(k).project(cam);
          if (proj.z > 1 || Math.abs(proj.x) > 1.05 || Math.abs(proj.y) > 1.05) continue;
          const x = (proj.x + 1) / 2 * w, y = (1 - proj.y) / 2 * h;
          g.strokeStyle = `rgba(220,224,232,${0.35 * a})`; g.beginPath(); g.moveTo(x + 4, y - 4); g.lineTo(x + 14, y - 14); g.stroke();
          g.fillStyle = `rgba(236,232,224,${0.9 * a})`; g.textAlign = 'left'; g.fillText(name, x + 17, y - 15);
        }
        const camFromEarth = cam.position.length() / k;           // pc
        const cm = camFromEarth * C.pc;
        const lt = cm / C.c;                                        // light travel time t = d / c
        g.textAlign = 'left'; P.hud(g, 12, 12, [
          ['camera from Earth', fmtLen(cm)],
          ['light from here takes', P.duration(lt)],
          ['Eagle Nebula', `1,740 pc · ${P.fmt(TRAVEL)} ly`],
          ['arriving photons left', `~${yearLabel(DEP)}`, '#ff9d86'],
        ], { px: hudPx(w) });
        // Stage title, bottom-left, in the style of a card.
        const st = aim > 0.4 ? 'the Eagle Nebula' : STAGES.find(s => L < s[0])[1];
        g.font = `600 ${w < 520 ? 15 : 19}px "Archivo", "Helvetica Neue", Arial, sans-serif`;
        g.fillStyle = 'rgba(242,239,232,0.92)'; g.textAlign = 'left'; g.textBaseline = 'bottom';
        g.fillText(st, 14, h - 14);
        g.font = mono(10); g.fillStyle = 'rgba(200,205,215,0.6)'; g.textAlign = 'right';
        g.fillText('true sizes · schematic arms · photon speed not to scale', w - 12, h - 12);
        if (fadeA > 0) { g.fillStyle = `rgba(2,3,8,${fadeA})`; g.fillRect(0, 0, w, h); }
      },
    };
  }, { aspect: 0.6 });

  // =====================================================================================
  // 3. Parallax in 3D, with Gaia-style averaging of cluster stars.
  // =====================================================================================
  // Parallax: p[arcsec] = 1 au / d = 1 / d[pc]; at 1,740 pc, p = 0.575 mas.
  // Gaia per-star sigma ~0.03 mas for bright members (Stoop+2023 quote 0.0024 mas on the mean of 137 stars,
  // so sigma_1 ≈ 0.0024·√137); systematic floor ~0.01 mas (Lindegren+2021; Maíz Apellániz+2021).
  // Error of the mean of N stars: sigma_N = sqrt(sigma_1² / N + floor²).
  const P_MAS = 1000 / D_PC, SIG1 = 0.03, SYS = 0.01, NMAX = 160;
  P.demo3d('fig-where-parallax', (ctx, THREE) => {
    const scene = new THREE.Scene(), mats = [];
    const cam = new THREE.PerspectiveCamera(40, 1, 0.05, 500);
    const add = (o) => { scene.add(o); if (o.material && o.material.uniforms && o.material.uniforms.uPx) mats.push(o.material); return o; };
    const r = P.rng(57);
    const STAR = new THREE.Vector3(11, 0.4, 0), WALL = 70, PLANE = 24;   // star drawn at 11 au, background plane at 24: not to scale
    // Background: distant stars on a shell, and a faint Milky Way haze.
    add(pointCloud(THREE, 6000, (i, pos, col, size) => {
      const u = 2 * r() - 1, th = 2 * Math.PI * r();
      const band = i < 3000; const yy = band ? gauss(r) * 0.12 : u;
      const rr = Math.sqrt(Math.max(0, 1 - yy * yy));
      pos.set([WALL * rr * Math.cos(th), WALL * yy, WALL * rr * Math.sin(th)], 3 * i);
      col.set(bb(3500 + 9000 * Math.pow(r(), 2), band ? 0.35 : 0.8), 3 * i); size[i] = (band ? 0.5 : 0.18) * (0.5 + r());
    }, { minPx: 1, maxPx: 5 }));
    // A patch of 'distant background' stars on a plane behind the target, where the apparent shift is traced.
    add(pointCloud(THREE, 900, (i, pos, col, size) => {
      pos.set([PLANE + 0.3 * r(), (r() * 2 - 1) * 6, (r() * 2 - 1) * 9], 3 * i);
      col.set(bb(3500 + 9000 * Math.pow(r(), 2), 0.9), 3 * i); size[i] = 0.05 * (0.5 + r());
    }, { minPx: 1.2, maxPx: 5 }));
    // Sun, Earth, orbit, target star.
    add(pointCloud(THREE, 2, (i, pos, col, size) => { pos.set(i ? STAR.toArray() : [0, 0, 0], 3 * i); col.set(i ? [1.0, 0.45, 0.4] : [1.6, 1.35, 1.0], 3 * i); size[i] = i ? 0.5 : 1.2; }, { minPx: 5, maxPx: 80, dimSmall: 0 }));
    const sunBall = new THREE.Mesh(new THREE.SphereGeometry(0.12, 32, 16), new THREE.MeshBasicMaterial({ color: 0xfff1d6, depthTest: false })); scene.add(sunBall);
    const ring = []; for (let i = 0; i <= 128; i++) { const a = i / 128 * 2 * Math.PI; ring.push(new THREE.Vector3(Math.cos(a), 0, Math.sin(a))); }
    scene.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(ring), new THREE.LineBasicMaterial({ color: 0x5fa0ff, transparent: true, opacity: 0.55, depthTest: false })));
    const earthBall = litSphere(THREE, 0.07, [0.35, 0.6, 1.0], [0.03, 0.05, 0.1], [0, 1, 0]); scene.add(earthBall);
    const earthGlow = add(pointCloud(THREE, 1, (i, pos, col, size) => { col.set([0.35, 0.6, 1], 0); size[0] = 0.3; }, { minPx: 4, maxPx: 30, dimSmall: 0 }));
    // Sight lines: current (bright) from Earth through the star to the background, plus the six-month extremes.
    const mkLine = (c, o) => { const l = new THREE.Line(new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), new THREE.Vector3()]), new THREE.LineBasicMaterial({ color: c, transparent: true, opacity: o, depthTest: false })); scene.add(l); return l; };
    const sight = mkLine(0x5fd0c4, 0.95), ex1 = mkLine(0x8a93a8, 0.35), ex2 = mkLine(0x8a93a8, 0.35);
    const through = (e) => { const d = STAR.clone().sub(e); return [e, e.clone().add(d.multiplyScalar((PLANE - e.x) / d.x))]; };   // ray Earth -> star, extended to the background plane
    [ex1, ex2].forEach((l, k) => l.geometry.setFromPoints(through(new THREE.Vector3(0, 0, k ? 1 : -1))));
    // Trail on the background: where the star appears to sit as Earth moves.
    const TR = 200, trail = add(pointCloud(THREE, TR, (i, pos, col, size) => { col.set([0.37, 0.82, 0.77], 3 * i); size[i] = 0.12; }, { minPx: 2, maxPx: 6 }));
    let trI = 0;

    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(12, -2.2, 0), dist: 31, yaw: -0.55, pitch: 0.45, auto: 0.02 });
    const dS = ctx.slider({ label: 'Distance (sets the real parallax)', min: 1, max: Math.log10(3000), step: 0.001, value: Math.log10(D_PC), format: (v) => `${P.fmt(Math.pow(10, v), 3)} pc`, onInput: () => { meas.length = 0; } });
    let phase = 0, nextDrop = 0;
    const meas = [];   // simulated Gaia parallaxes of cluster members (mas)
    const gr = P.rng(99);
    return {
      update(t, dt, pw, ph) {
        dt = dt || 0;
        orb.step(dt);
        phase += dt * 2 * Math.PI / 7;                         // one year every 7 s
        const e = new THREE.Vector3(Math.cos(phase), 0, Math.sin(phase));
        earthBall.position.copy(e); earthGlow.geometry.attributes.position.setXYZ(0, e.x, e.y, e.z); earthGlow.geometry.attributes.position.needsUpdate = true;
        const [a, b] = through(e);
        sight.geometry.setFromPoints([a, b]);
        trail.geometry.attributes.position.setXYZ(trI++ % TR, b.x, b.y, b.z); trail.geometry.attributes.position.needsUpdate = true;
        const uPx = ph / (2 * Math.tan(cam.fov * D2R / 2));
        for (const m of mats) m.uniforms.uPx.value = uPx;
        // Measurements arrive four per second; restart after NMAX.
        nextDrop -= dt;
        if (nextDrop <= 0) {
          nextDrop = 0.25;
          if (meas.length >= NMAX) meas.length = 0;
          const p = 1000 / Math.pow(10, dS.value);
          meas.push({ v: p + SIG1 * gauss(gr), y: gr(), t: 0 });
        }
        meas.forEach(m => { m.t += dt; });
      },
      scene, camera: cam,
      overlay(g, w, h) {
        const dpc = Math.pow(10, dS.value), p = 1000 / dpc;
        const N = Math.max(1, meas.length), mean = meas.reduce((s, m) => s + m.v, 0) / N;
        const sig = Math.sqrt(SIG1 * SIG1 / N + SYS * SYS);
        // Gaia panel, bottom: each dot is one cluster star's parallax; the band is the error of the mean.
        const px0 = 16, px1 = w - 16, py1 = h - 14, py0 = py1 - Math.min(90, h * 0.2);
        const lo = p - 0.15, hi = p + 0.15, X = (v) => px0 + (v - lo) / (hi - lo) * (px1 - px0);
        g.fillStyle = 'rgba(6,8,12,0.6)'; g.fillRect(px0 - 6, py0 - 22, px1 - px0 + 12, py1 - py0 + 28);
        g.fillStyle = 'rgba(95,208,196,0.18)'; g.fillRect(X(mean - sig), py0, X(mean + sig) - X(mean - sig), py1 - py0 - 14);
        g.fillStyle = 'rgba(255,190,170,0.85)';
        for (const m of meas) { const y = py0 + 4 + m.y * (py1 - py0 - 22) * Math.min(1, m.t * 3); g.globalAlpha = Math.min(1, m.t * 4); g.beginPath(); g.arc(X(m.v), y, 2.2, 0, 2 * Math.PI); g.fill(); }
        g.globalAlpha = 1;
        g.strokeStyle = '#5fd0c4'; g.lineWidth = 2; g.beginPath(); g.moveTo(X(mean), py0 - 2); g.lineTo(X(mean), py1 - 12); g.stroke();
        g.strokeStyle = 'rgba(242,239,232,0.6)'; g.lineWidth = 1; g.setLineDash([3, 3]); g.beginPath(); g.moveTo(X(p), py0 - 2); g.lineTo(X(p), py1 - 12); g.stroke(); g.setLineDash([]);
        g.font = mono(10); g.fillStyle = 'rgba(220,224,232,0.75)'; g.textBaseline = 'bottom';
        for (let k = -2; k <= 2; k++) { const v = Math.round((p + k * 0.05) * 100) / 100; g.textAlign = 'center'; g.fillText(`${v.toFixed(2)}`, X(v), py1 + 2); }
        g.textAlign = 'left'; g.fillText('Gaia parallaxes of cluster stars (mas) · teal: running mean ± error', px0, py0 - 8);
        const near = 1 / (mean + sig), far = mean > sig ? 1 / (mean - sig) : Infinity;
        g.textAlign = 'left'; P.hud(g, 12, 12, [
          ['true parallax', `${P.fmt(p, 3)} mas`],
          ['≈ a coin seen from', `${P.fmt(0.02325 / (p * 1e-3 * ARCSEC) / 1000, 2)} km`],
          ['stars averaged', String(meas.length)],
          ['mean', `${mean.toFixed(3)} ± ${sig.toFixed(3)} mas`, '#5fd0c4'],
          ['distance', isFinite(far) ? `${P.fmt(near, 3)}–${P.fmt(far, 3)} kpc` : `> ${P.fmt(near, 3)} kpc`, '#5fd0c4'],
        ], { px: hudPx(w) });
        g.font = mono(10); g.fillStyle = 'rgba(200,205,215,0.6)'; g.textAlign = 'right'; g.textBaseline = 'top';
        g.fillText(`not to scale: the star is really ${P.sci(dpc * AU_PER_PC, 2)} au away, drawn at 11`, w - 12, 12);
      },
    };
  }, { aspect: 0.62 });
})();
