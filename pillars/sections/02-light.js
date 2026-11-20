// Section 02, "Light from atoms" (round 2): three self-playing WebGL scenes.
//   fig-light-atom  : a hydrogen atom recombining and cascading; photons fly onto a growing spectrum,
//                     plus O++ and S+ ions whose slow "forbidden" photons are quenched at high density.
//   fig-light-palette: a ray-marched pillar field split into H-alpha, [O III] and [S II] channels,
//                     shown in natural colour or the Hubble palette.
//   fig-light-dust  : starlight at six wavelengths crossing a pillar; each photon is absorbed with the
//                     CCM89 optical depth for its wavelength.
(function () {
  'use strict';

  // ---------------- physics ----------------
  // Bohr levels of hydrogen: E_n = -13.6057 eV / n^2.
  const EH = 13.6057;
  // Rydberg formula: 1/lambda = R_H (1/n1^2 - 1/n2^2), R_H = 109677.58 cm^-1. Returns vacuum wavelength in nm.
  const RH = 109677.58;
  const lamVac = (n1, n2) => 1e7 / (RH * (1 / (n1 * n1) - 1 / (n2 * n2)));
  // Air wavelength above 200 nm (refractive index of air ~1.000277 in the visible; simplified Edlen 1966).
  const lamAir = (nm) => (nm > 200 ? nm / 1.000277 : nm);
  // Hydrogen Einstein A coefficients, l-averaged (s^-1), Wiese & Fuhr 2009.
  const A = {
    '2,1': 4.699e8, '3,1': 5.575e7, '3,2': 4.410e7, '4,1': 1.278e7, '4,2': 8.419e6, '4,3': 8.986e6,
    '5,1': 4.125e6, '5,2': 2.530e6, '5,3': 2.201e6, '5,4': 2.699e6,
    '6,1': 1.644e6, '6,2': 9.732e5, '6,3': 7.783e5, '6,4': 7.711e5, '6,5': 1.010e6,
  };
  // Case B (Osterbrock & Ferland 2006): Lyman photons from n >= 3 are re-absorbed, so those branches are dropped.
  function branches(u) {
    const out = []; let sum = 0;
    for (let l = 1; l < u; l++) { if (l === 1 && u >= 3) continue; const a = A[u + ',' + l]; out.push([l, a]); sum += a; }
    return out.map(([l, a]) => [l, a / sum]);
  }
  // Toy capture distribution onto n = 2..6 with weight n^-1.5: gives ~0.40 H-alpha photons per recombination
  // and H-alpha/H-beta ~ 2.8 in energy (full case-B value 0.45 and 2.86).
  const CAPW = [2, 3, 4, 5, 6].map((n) => [n, Math.pow(n, -1.5)]);
  const CAPSUM = CAPW.reduce((s, x) => s + x[1], 0);
  const pick = (list, r) => { let acc = 0; for (const [v, w] of list) { acc += w; if (r <= acc) return v; } return list[list.length - 1][0]; };
  const capture = (r) => pick(CAPW.map(([k, w]) => [k, w / CAPSUM]), r);

  // Two-level ion: share of collisional excitations that end in a photon, f = 1/(1 + n_e/n_crit).
  const fRad = (n, ncrit) => 1 / (1 + n / ncrit);
  // Critical densities at 1e4 K (Osterbrock & Ferland 2006, Table 3.15), cm^-3.
  const NCRIT_OIII = 6.8e5, NCRIT_SII = 2.5e3; // [S II]: effective value between 6716 (1.5e3) and 6731 (3.9e3)
  // [S II] 6716/6731 ratio vs n_e at 1e4 K: R = a (b + n)/(c + n), Sanders et al. 2016 (ApJ 816, 23).
  const siiRatio = (n) => 0.4315 * (2107 + n) / (627.1 + n);

  // Cardelli, Clayton & Mathis 1989 (ApJ 345, 245): A_lambda/A_V = a(x) + b(x)/R_V, x = 1/lambda[um].
  // Valid 0.3 <= x <= 3.3 here; for lambda > 3.3 um the IR power law is extrapolated.
  function ccm(lamUm, Rv = 3.1) {
    const x = 1 / lamUm; let a, b;
    if (x < 1.1) { a = 0.574 * Math.pow(x, 1.61); b = -0.527 * Math.pow(x, 1.61); }
    else {
      const y = x - 1.82;
      a = 1 + 0.17699 * y - 0.50447 * y ** 2 - 0.02427 * y ** 3 + 0.72085 * y ** 4 + 0.01979 * y ** 5 - 0.77530 * y ** 6 + 0.32999 * y ** 7;
      b = 1.41338 * y + 2.28305 * y ** 2 + 1.07233 * y ** 3 - 5.38434 * y ** 4 - 0.62251 * y ** 5 + 5.30260 * y ** 6 - 2.09002 * y ** 7;
    }
    return a + b / Rv;
  }
  // Magnitudes to optical depth: A = 2.5 log10(e) tau = 1.0857 tau; a photon survives with probability exp(-tau).
  const tauOf = (Amag) => Amag / 1.0857;

  // ---------------- shared GL helpers ----------------
  const sup = (e) => String(e).split('').map((ch) => ({ '-': '⁻', 0: '⁰', 1: '¹', 2: '²', 3: '³', 4: '⁴', 5: '⁵', 6: '⁶', 7: '⁷', 8: '⁸', 9: '⁹' }[ch])).join('');
  const nm01 = (nm) => P.nmColor(nm).map((v) => v / 255);
  const IRCOL = [0.72, 0.45, 1.0], UVCOL = [0.62, 0.42, 1.0];
  const lineCol = (nm) => (nm < 390 ? UVCOL : nm > 720 ? IRCOL : nm01(nm));

  // Soft additive sprites with per-point colour, size (world units) and alpha.
  function makePoints(N) {
    const geo = new THREE.BufferGeometry();
    const pos = new Float32Array(N * 3), col = new Float32Array(N * 3), size = new Float32Array(N), alpha = new Float32Array(N);
    geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    geo.setAttribute('aColor', new THREE.BufferAttribute(col, 3));
    geo.setAttribute('aSize', new THREE.BufferAttribute(size, 1));
    geo.setAttribute('aAlpha', new THREE.BufferAttribute(alpha, 1));
    const mat = new THREE.ShaderMaterial({
      uniforms: { uTex: { value: P.dotTexture() }, uScale: { value: 400 } },
      vertexShader: `attribute vec3 aColor; attribute float aSize; attribute float aAlpha; uniform float uScale;
        varying vec3 vC; varying float vA;
        void main(){ vC = aColor; vA = aAlpha; vec4 mv = modelViewMatrix * vec4(position, 1.0);
          gl_PointSize = max(1.0, aSize * uScale / -mv.z); gl_Position = projectionMatrix * mv; }`,
      fragmentShader: `uniform sampler2D uTex; varying vec3 vC; varying float vA;
        void main(){ float a = texture2D(uTex, gl_PointCoord).a * vA; if (a < 0.003) discard; gl_FragColor = vec4(vC, a); }`,
      transparent: true, depthWrite: false, depthTest: false, blending: THREE.AdditiveBlending,
    });
    const pts = new THREE.Points(geo, mat); pts.frustumCulled = false; pts.renderOrder = 5;
    const o = {
      pts, pos, col, size, alpha, N,
      set(i, x, y, z, c, s, a) { pos[3 * i] = x; pos[3 * i + 1] = y; pos[3 * i + 2] = z; col[3 * i] = c[0]; col[3 * i + 1] = c[1]; col[3 * i + 2] = c[2]; size[i] = s; alpha[i] = a; },
      hide(i) { alpha[i] = 0; },
      commit(cam, ph) {
        mat.uniforms.uScale.value = ph / (2 * Math.tan(cam.fov * Math.PI / 360));
        for (const k of ['position', 'aColor', 'aSize', 'aAlpha']) geo.attributes[k].needsUpdate = true;
      },
    };
    for (let i = 0; i < N; i++) alpha[i] = 0;
    return o;
  }
  // Camera drift: gentle sway when idle, drag to look around (P.orbit with auto-spin off).
  function sway(orb, t, dt, yaw0, amp) {
    if (orb.idle > 3) orb.yaw += (yaw0 + amp * Math.sin(t * 0.13) - orb.yaw) * Math.min(1, dt * 0.6);
    orb.step(dt);
  }
  const toScreen = (v, cam, w, h) => { const p = v.clone().project(cam); return [(p.x + 1) / 2 * w, (1 - p.y) / 2 * h, p.z < 1]; };
  const font = (px) => `${px}px "JetBrains Mono", ui-monospace, monospace`;
  const css = (c, a = 1) => `rgba(${Math.round(c[0] * 255)},${Math.round(c[1] * 255)},${Math.round(c[2] * 255)},${a})`;

  // =====================================================================
  // 1. The atom: recombination, cascade, forbidden lines
  // =====================================================================
  P.demo3d('fig-light-atom', (ctx) => {
    const scene = new THREE.Scene(); scene.background = new THREE.Color(0x05070c);
    const cam = new THREE.PerspectiveCamera(40, 1, 0.05, 80);
    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(0, -0.05, 0), dist: 7.4, yaw: 0, pitch: 0.2, auto: 0 });
    const dens = ctx.slider({ label: 'Electron density', min: 1, max: 7, step: 0.01, value: 2, format: (v) => `${P.sci(Math.pow(10, v), 2)} cm⁻³`, onInput: () => reset() });
    const rnd = P.rng(21);
    const AT = new THREE.Vector3(0, 0.75, 0);
    const ringR = (n) => 0.15 * Math.pow(n, 1.25);
    const atom = new THREE.Group(); atom.position.copy(AT); atom.rotation.x = -1.05; atom.rotation.z = 0.2; scene.add(atom);
    const rings = [];
    for (let n = 1; n <= 6; n++) {
      const g = new THREE.BufferGeometry().setFromPoints(Array.from({ length: 129 }, (_, i) => new THREE.Vector3(Math.cos(i / 64 * Math.PI) * ringR(n), Math.sin(i / 64 * Math.PI) * ringR(n), 0)));
      const m = new THREE.LineBasicMaterial({ color: 0x8fb4ff, transparent: true, opacity: 0.18, blending: THREE.AdditiveBlending, depthWrite: false });
      const l = new THREE.Line(g, m); atom.add(l); rings.push(l);
    }
    const ringPos = (n, phi) => new THREE.Vector3(Math.cos(phi) * ringR(n), Math.sin(phi) * ringR(n), 0).applyEuler(atom.rotation).add(AT);

    // spectrum floor: piecewise wavelength axis (log UV stub, linear visible, log IR stub)
    const FY = -1.45, FZ = 0.55;
    const xOf = (nm) => nm < 380 ? -3.05 + 0.55 * Math.log(nm / 90) / Math.log(380 / 90)
      : nm <= 720 ? -2.35 + 4.4 * (nm - 380) / 340 : 2.3 + 0.75 * Math.log(nm / 720) / Math.log(7500 / 720);
    const nmOf = (x) => x < -2.5 ? 90 * Math.pow(380 / 90, (x + 3.05) / 0.55) : x <= 2.05 ? 380 + 340 * (x + 2.35) / 4.4 : x < 2.3 ? -1 : 720 * Math.pow(7500 / 720, (x - 2.3) / 0.75);
    {
      const c = document.createElement('canvas'); c.width = 1024; c.height = 4; const g = c.getContext('2d');
      for (let i = 0; i < 1024; i++) {
        const x = -3.1 + 6.2 * i / 1023, nm = nmOf(x);
        const col = nm < 0 || x < -3.05 || x > 3.05 ? [0, 0, 0] : nm < 380 ? [0.16, 0.08, 0.3] : nm > 720 ? [0.2, 0.07, 0.26] : nm01(nm).map((v) => v * 0.55);
        g.fillStyle = css(col); g.fillRect(i, 0, 1, 4);
      }
      const tex = new THREE.CanvasTexture(c);
      const floor = new THREE.Mesh(new THREE.PlaneGeometry(6.2, 0.36), new THREE.MeshBasicMaterial({ map: tex, transparent: true, opacity: 0.95, depthWrite: false }));
      floor.rotation.x = -Math.PI / 2; floor.position.set(0, FY, FZ); scene.add(floor);
    }
    // spectral lines on the floor
    const LINES = [];
    const addLine = (key, nm, label) => {
      const c = lineCol(nm), x = xOf(nm);
      const mk = (wid, op) => { const g = new THREE.PlaneGeometry(wid, 1); g.translate(0, 0.5, 0); const m = new THREE.MeshBasicMaterial({ color: new THREE.Color(c[0], c[1], c[2]), transparent: true, opacity: op, blending: THREE.AdditiveBlending, depthWrite: false }); const mesh = new THREE.Mesh(g, m); mesh.position.set(x, FY, FZ); mesh.scale.y = 0.001; scene.add(mesh); return mesh; };
      const L = { key, nm, label, c, x, count: 0, flash: 0, bar: mk(0.02, 0.95), glow: mk(0.11, 0.16), h: 0 };
      LINES.push(L); return L;
    };
    const HL = {};
    addLine('H1,2', lamVac(1, 2), 'Lyα');
    for (let d = 2; d <= 5; d++) for (let u = d + 1; u <= 6; u++) {
      const nm = lamAir(lamVac(d, u));
      const lab = { '2,3': 'Hα', '2,4': 'Hβ', '2,5': 'Hγ', '3,4': 'Paα' }[d + ',' + u] || '';
      addLine(`H${d},${u}`, nm, lab);
    }
    for (const L of LINES) HL[L.key] = L;
    HL.O1 = addLine('O1', 500.7, '[O III]'); HL.O2 = addLine('O2', 495.9, '');
    HL.S1 = addLine('S1', 671.6, '[S II]'); HL.S2 = addLine('S2', 673.1, '');
    let nRec = 0, nHa = 0, lostO = 0, lostS = 0;
    function reset() { for (const L of LINES) { L.count = 0; } nRec = 0; nHa = 0; lostO = 0; lostS = 0; }

    // particles
    const P1 = makePoints(1400 + 40 * 26 + 64); scene.add(P1.pts);
    const NFREE = 1400, PK0 = NFREE, NPK = 40, PKN = 26, MISC = PK0 + NPK * PKN;
    const free = [];
    for (let i = 0; i < NFREE; i++) {
      const u = rnd() * 2 - 1, ph = rnd() * 6.283, r = 1.9 + 1.6 * Math.cbrt(rnd());
      const s = Math.sqrt(1 - u * u);
      free.push({ p: new THREE.Vector3(r * s * Math.cos(ph), r * u * 0.7, r * s * Math.sin(ph)).add(AT), v: new THREE.Vector3(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).multiplyScalar(0.25) });
    }
    // photon wave packets flying from an emitter to their line on the floor
    const packets = Array.from({ length: NPK }, () => ({ on: false }));
    const viewDir = new THREE.Vector3();
    function emit(from, line, dur = 1.5) {
      const k = packets.findIndex((p) => !p.on); if (k < 0) { line.count++; return; }
      const to = new THREE.Vector3(line.x, FY + 0.02, FZ);
      const mid = from.clone().lerp(to, 0.5); mid.y += 0.9; mid.z += 0.6;
      Object.assign(packets[k], { on: true, t: 0, dur, p0: from.clone(), p1: to, pc: mid, line, c: line.c, landed: false });
    }
    const bez = (P0, Pc, P1_, s, out) => out.set(
      (1 - s) * (1 - s) * P0.x + 2 * (1 - s) * s * Pc.x + s * s * P1_.x,
      (1 - s) * (1 - s) * P0.y + 2 * (1 - s) * s * Pc.y + s * s * P1_.y,
      (1 - s) * (1 - s) * P0.z + 2 * (1 - s) * s * Pc.z + s * s * P1_.z);

    // the featured atom's electron
    const el = { state: 'free', t: 0, n: 0, phi: 0, p: new THREE.Vector3(), from: new THREE.Vector3(), to: 0, dur: 1 };
    const CLUSTER = new THREE.Vector3(4.5, 3.5, -1.5);
    let ionPh = null; // incoming ionizing photon {t, p0}
    function startFall() { el.state = 'fall'; el.t = 0; el.dur = 0.9; el.n = capture(rnd()); el.phi = rnd() * 6.283; el.from.copy(free[Math.floor(rnd() * NFREE)].p); }

    // forbidden-line ions
    const IONS = [
      { name: 'O III', pos: new THREE.Vector3(-2.55, 0.95, 0.1), c: [0.37, 0.9, 0.82], ncrit: NCRIT_OIII, wait: 1.4, next: 1.0 },
      { name: 'S II', pos: new THREE.Vector3(2.55, 0.95, 0.1), c: [1.0, 0.68, 0.28], ncrit: NCRIT_SII, wait: 2.2, next: 2.0 },
    ];
    for (const I of IONS) Object.assign(I, { state: 'idle', t: 0, hit: null, quenchAt: 0, spark: 0 });

    // unseen atoms: time-lapse statistics so the spectrum fills at a readable rate
    let bgAcc = 0;
    function bgRecombination(n) {
      let lev = capture(rnd()); nRec++;
      while (lev > 1) { const lo = pick(branches(lev), rnd()); HL[`H${lo},${lev}`].count++; if (lo === 2 && lev === 3) nHa++; lev = lo; }
      // forbidden lines per recombination at low density (photon ratios tuned to [O III]5007/Hbeta ~1.5, [S II]6716/Halpha ~0.11)
      const fO = fRad(n, NCRIT_OIII), fS = fRad(n, NCRIT_SII), R = siiRatio(n);
      if (rnd() < 0.185 * fO) HL.O1.count++; else if (rnd() < 0.185) lostO++;
      if (rnd() < 0.062 * fO) HL.O2.count++;
      const sTot = 0.11 * fS; if (rnd() < sTot) { if (rnd() < R / (1 + R)) HL.S1.count++; else HL.S2.count++; } else if (rnd() < 0.11) lostS++;
    }

    const tmp = new THREE.Vector3(), tmp2 = new THREE.Vector3(), tan = new THREE.Vector3(), side = new THREE.Vector3();
    let T = 0;
    return {
      scene, camera: cam, scale: 1,
      update(t, dt, pw, ph) {
        dt = Math.max(0, dt || 0); T += dt; orb.dist = ctx.w < 560 ? 9.6 : 7.4; sway(orb, T, dt, 0, 0.32);
        const n = Math.pow(10, dens.value);
        cam.getWorldDirection(viewDir);
        // free electrons drift
        for (let i = 0; i < NFREE; i++) {
          const f = free[i]; f.p.addScaledVector(f.v, dt);
          tmp.copy(f.p).sub(AT); const r = tmp.length();
          if (r > 3.6 || r < 1.7) f.v.addScaledVector(tmp, -dt * (r > 3.6 ? 0.6 : -0.6) / r);
          P1.set(i, f.p.x, f.p.y, f.p.z, [0.55, 0.72, 1.0], 0.05, 0.22);
        }
        // featured electron state machine
        el.t += dt;
        let ep = null;
        if (el.state === 'free') { if (el.t > 0.6) startFall(); }
        else if (el.state === 'fall') {
          const s = P.smooth(Math.min(1, el.t / el.dur)); el.phi += dt * 2;
          ep = el.from.clone().lerp(ringPos(el.n, el.phi), s);
          if (el.t >= el.dur) { el.state = 'bound'; el.t = 0; nRec++; }
        } else if (el.state === 'bound') {
          el.phi += dt * 3.2 / Math.pow(el.n, 1.2);
          ep = ringPos(el.n, el.phi);
          if (el.n > 1 && el.t > 0.55) {
            const lo = pick(branches(el.n), rnd());
            const L = HL[`H${lo},${el.n}`]; emit(ep, L); if (lo === 2 && el.n === 3) nHa++;
            el.state = 'jump'; el.t = 0; el.to = lo;
          } else if (el.n === 1 && el.t > 1.1 && !ionPh) ionPh = { t: 0 };
        } else if (el.state === 'jump') {
          const s = P.smooth(Math.min(1, el.t / 0.22)); el.phi += dt * 3;
          const r = P.lerp(ringR(el.n), ringR(el.to), s);
          ep = new THREE.Vector3(Math.cos(el.phi) * r, Math.sin(el.phi) * r, 0).applyEuler(atom.rotation).add(AT);
          if (el.t >= 0.22) { el.n = el.to; el.state = 'bound'; el.t = 0; }
        } else if (el.state === 'escape') {
          const s = P.smooth(Math.min(1, el.t / 1.0));
          ep = el.from.clone().lerp(tmp2, s);
          if (el.t >= 1.0) { el.state = 'free'; el.t = 0; }
        }
        // ionizing UV photon from the cluster knocks the electron out again
        if (ionPh) {
          ionPh.t += dt; const s = Math.min(1, ionPh.t / 0.8);
          const target = ringPos(1, el.phi);
          for (let i = 0; i < 12; i++) { const si = s - i * 0.02; if (si < 0) { P1.hide(MISC + 40 + i); continue; } tmp.copy(CLUSTER).lerp(target, si); P1.set(MISC + 40 + i, tmp.x, tmp.y, tmp.z, UVCOL, 0.1, Math.exp(-i * 0.25) * 0.9); }
          if (s >= 1) { ionPh = null; for (let i = 0; i < 12; i++) P1.hide(MISC + 40 + i); el.state = 'escape'; el.t = 0; el.from.copy(ep || target); tmp2.copy(AT).add(new THREE.Vector3(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalize().multiplyScalar(2.6)); }
        }
        if (ep) P1.set(MISC, ep.x, ep.y, ep.z, [0.75, 0.92, 1.0], 0.16, 1.0); else P1.hide(MISC);
        P1.set(MISC + 1, AT.x, AT.y, AT.z, [1.0, 0.92, 0.85], 0.22, 0.95); // proton
        P1.set(MISC + 2, AT.x, AT.y, AT.z, [0.5, 0.6, 1.0], 0.9, 0.12);
        rings.forEach((r, i) => { r.material.opacity = (el.state === 'bound' || el.state === 'jump') && el.n === i + 1 ? 0.55 : 0.16; });

        // ions
        IONS.forEach((I, k) => {
          I.t += dt; const base = MISC + 4 + k * 8;
          let glow = 0.12;
          if (I.state === 'idle' && I.t > I.next) {
            I.state = 'hit'; I.t = 0; I.hit = free[Math.floor(rnd() * NFREE)].p.clone();
          } else if (I.state === 'hit') {
            const s = Math.min(1, I.t / 0.45); tmp.copy(I.hit).lerp(I.pos, P.smooth(s));
            P1.set(base + 2, tmp.x, tmp.y, tmp.z, [0.75, 0.92, 1.0], 0.12, 1);
            if (s >= 1) {
              P1.hide(base + 2); I.state = 'excited'; I.t = 0;
              I.survive = rnd() < fRad(n, I.ncrit);
              I.quenchAt = I.survive ? 1e9 : rnd() * I.wait * 0.9;
              if (!I.survive) I.hit = free[Math.floor(rnd() * NFREE)].p.clone();
            }
          } else if (I.state === 'excited') {
            glow = 0.35 + 0.25 * Math.sin(I.t * 9);
            if (!I.survive) {
              const s = Math.min(1, I.t / Math.max(0.05, I.quenchAt));
              tmp.copy(I.hit).lerp(I.pos, P.smooth(s)); P1.set(base + 2, tmp.x, tmp.y, tmp.z, [0.75, 0.92, 1.0], 0.12, 1);
              if (s >= 1) { P1.hide(base + 2); I.spark = 1; I.state = 'idle'; I.t = 0; I.next = 0.6 + rnd() * 1.2; if (k === 0) lostO++; else lostS++; }
            } else if (I.t > I.wait) {
              let L;
              if (k === 0) L = rnd() < 0.75 ? HL.O1 : HL.O2;
              else { const R = siiRatio(n); L = rnd() < R / (1 + R) ? HL.S1 : HL.S2; }
              emit(I.pos, L, 1.7); I.state = 'idle'; I.t = 0; I.next = 0.6 + rnd() * 1.2;
            }
          }
          P1.set(base, I.pos.x, I.pos.y, I.pos.z, I.c, 0.16, 1);
          P1.set(base + 1, I.pos.x, I.pos.y, I.pos.z, I.c, 0.85, glow);
          I.spark = Math.max(0, I.spark - dt * 1.6);
          P1.set(base + 3, I.pos.x, I.pos.y, I.pos.z, [0.7, 0.7, 0.75], 0.3 + (1 - I.spark) * 0.9, I.spark * 0.8);
        });

        // background time-lapse
        bgAcc += dt * 9; while (bgAcc >= 1) { bgAcc -= 1; bgRecombination(n); }

        // packets
        packets.forEach((pk, k) => {
          const b0 = PK0 + k * PKN;
          if (!pk.on) { for (let i = 0; i < PKN; i++) P1.hide(b0 + i); return; }
          pk.t += dt; const s = pk.t / pk.dur;
          if (s >= 1 && !pk.landed) { pk.landed = true; pk.line.count++; pk.line.flash = 1; }
          if (s > 1 + PKN * 0.011) { pk.on = false; for (let i = 0; i < PKN; i++) P1.hide(b0 + i); return; }
          for (let i = 0; i < PKN; i++) {
            const si = s - i * 0.011;
            if (si < 0 || si > 1) { P1.hide(b0 + i); continue; }
            bez(pk.p0, pk.pc, pk.p1, si, tmp); bez(pk.p0, pk.pc, pk.p1, Math.min(1, si + 0.01), tan); tan.sub(tmp).normalize();
            side.crossVectors(tan, viewDir).normalize();
            const wob = Math.sin(i * 1.15 - pk.t * 26) * 0.045 * Math.exp(-Math.pow((i - 12) / 9, 2));
            tmp.addScaledVector(side, wob);
            const env = Math.exp(-Math.pow((i - 12) / 7, 2));
            P1.set(b0 + i, tmp.x, tmp.y, tmp.z, pk.c, 0.085, env * (0.55 + 0.45 * Math.cos(i * 1.15 - pk.t * 26)));
          }
        });

        // bars: H-alpha sets the scale; bars grow in as statistics accumulate
        const ha = Math.max(1, HL['H2,3'].count);
        const grow = Math.min(1, nRec / 120);
        for (const L of LINES) {
          const target = Math.min(1.25, 1.25 * L.count / ha) * grow + (L.count > 0 ? 0.015 : 0);
          L.h += (target - L.h) * Math.min(1, dt * 3);
          L.bar.scale.y = Math.max(0.001, L.h); L.glow.scale.y = Math.max(0.001, L.h);
          L.flash = Math.max(0, L.flash - dt * 2);
          L.bar.material.opacity = 0.75 + 0.25 * L.flash; L.glow.material.opacity = 0.12 + 0.3 * L.flash;
        }
        P1.commit(cam, ph);
      },
      overlay(g, w, h) {
        const n = Math.pow(10, dens.value), narrow = w < 560, px = narrow ? 9 : 12;
        const rows = [
          ['electrons', `${P.sci(n, 2)} cm⁻³`],
          ['recombinations', nRec.toLocaleString('en-US')],
          ['Hα per recomb.', nRec ? (nHa / nRec).toFixed(2) : '–', '#ff6b5e'],
          ['[O III] photons kept', `${(100 * fRad(n, NCRIT_OIII)).toFixed(n > 3e4 ? 0 : 1)}%`, '#5fd0c4'],
          ['[S II] photons kept', `${(100 * fRad(n, NCRIT_SII)).toFixed(0)}%`, '#f2a541'],
          ['[S II] 6716/6731', siiRatio(n).toFixed(2), '#f2a541'],
        ];
        P.hud(g, 8, 8, narrow ? [rows[2], rows[3], rows[4]] : rows, { px });
        g.font = font(px - 1); g.textAlign = 'center';
        const done = {};
        for (const L of LINES) {
          if (!L.label || done[L.label]) continue; done[L.label] = 1;
          const [x, y0] = toScreen(new THREE.Vector3(L.x, FY + L.h + 0.07, FZ), cam, w, h);
          const y = y0 - (L.label === '[O III]' || L.label === '[S II]' ? 14 : 0);
          g.fillStyle = css(L.c.map((v) => 0.55 + 0.45 * v), 0.95); g.fillText(L.label, x, y);
        }
        g.fillStyle = 'rgba(200,205,215,0.6)';
        for (const [nm, s] of [[200, 'UV'], [550, 'visible'], [2500, 'infrared']]) { const [x, y] = toScreen(new THREE.Vector3(xOf(nm), FY - 0.05, FZ + 0.32), cam, w, h); g.fillText(s, x, y + 6); }
        for (const I of IONS) { const [x, y] = toScreen(I.pos.clone().add(new THREE.Vector3(0, -0.32, 0)), cam, w, h); g.fillStyle = css(I.c, 0.85); g.fillText(I.name === 'O III' ? 'O²⁺' : 'S⁺', x, y + 4); }
        { const [x, y] = toScreen(AT.clone().add(new THREE.Vector3(0, -0.95, 0)), cam, w, h); g.fillStyle = 'rgba(200,215,255,0.7)'; g.fillText('H', x, y); }
        g.textAlign = 'left';
      },
    };
  }, { aspect: 0.62 });

  // =====================================================================
  // 2. Narrowband channels and colour palettes on the 3D pillars
  //    (shader adapted from lib/pillars-volume.js; emission split into three lines)
  // =====================================================================
  const PAL_FRAG = `
precision highp float;
varying vec2 vUv;
uniform mat4 uInvProj, uCamWorld;
uniform vec3 uCam, uCluster, uCS, uCH, uCO, uW;
uniform float uTime, uSteps;
float hash1(vec3 p){ p = fract(p*0.3183099 + 0.1); p *= 17.0; return fract(p.x*p.y*p.z*(p.x+p.y+p.z)); }
float noise(vec3 x){ vec3 i = floor(x), f = fract(x); f = f*f*(3.0-2.0*f);
  return mix(mix(mix(hash1(i),hash1(i+vec3(1,0,0)),f.x), mix(hash1(i+vec3(0,1,0)),hash1(i+vec3(1,1,0)),f.x),f.y),
             mix(mix(hash1(i+vec3(0,0,1)),hash1(i+vec3(1,0,1)),f.x), mix(hash1(i+vec3(0,1,1)),hash1(i+vec3(1,1,1)),f.x),f.y), f.z); }
float fbm(vec3 p){ float a = 0.5, s = 0.0; for(int i=0;i<5;i++){ s += a*noise(p); p = p*2.03 + vec3(1.7,9.2,3.1); a *= 0.5; } return s; }
float fbm3(vec3 p){ float a = 0.5, s = 0.0; for(int i=0;i<3;i++){ s += a*noise(p); p = p*2.03 + vec3(1.7,9.2,3.1); a *= 0.5; } return s; }
float capsule(vec3 p, vec3 a, vec3 b, float ra, float rb){ vec3 pa = p-a, ba = b-a; float h = clamp(dot(pa,ba)/dot(ba,ba), 0.0, 1.0); return length(pa - ba*h) - mix(ra, rb, h); }
float smin(float a, float b, float k){ float h = clamp(0.5+0.5*(b-a)/k, 0.0, 1.0); return mix(b, a, h) - k*h*(1.0-h); }
float sdBase(vec3 p){
  float d1 = capsule(p, vec3(-0.75,-1.45,-0.55), vec3(-0.35, 0.05,-0.45), 0.36, 0.17);
  d1 = smin(d1, capsule(p, vec3(-0.35, 0.05,-0.45), vec3(-0.22, 0.42,-0.42), 0.17, 0.2), 0.12);
  float d2 = capsule(p, vec3( 0.12,-1.45,-0.05), vec3( 0.42,-0.25, 0.0), 0.3, 0.13);
  d2 = smin(d2, capsule(p, vec3(0.42,-0.25,0.0), vec3(0.5,-0.05,0.02), 0.13, 0.12), 0.08);
  float d3 = capsule(p, vec3( 0.75,-1.45, 0.45), vec3( 0.92,-0.75, 0.5), 0.24, 0.1);
  float base = length((p - vec3(0.1,-2.9,0.0)) / vec3(2.6, 1.45, 1.3)) - 1.0;
  return smin(smin(smin(d1, d2, 0.18), d3, 0.18), base*0.9, 0.3);
}
float sdCloud(vec3 p){ float n = fbm3(p*vec3(3.2, 1.1, 3.2)); return sdBase(p) + (n - 0.5)*0.24 + (noise(p*vec3(11.0,4.0,11.0))-0.5)*0.05; }
float stars(vec3 dir){
  vec3 q = dir*180.0; vec3 id = floor(q); vec3 f = fract(q) - 0.5; float h = hash1(id);
  if (h < 0.975) return 0.0;
  vec3 off = vec3(hash1(id+1.3), hash1(id+2.7), hash1(id+5.1)) - 0.5;
  float r = length(f - off*0.6); return pow((h-0.975)/0.025, 2.0) * 3.0 * exp(-r*r*160.0);
}
void main(){
  vec2 ndc = vUv*2.0 - 1.0;
  vec4 vp = uInvProj * vec4(ndc, 1.0, 1.0); vp /= vp.w;
  vec3 rd = normalize((uCamWorld * vec4(vp.xyz, 0.0)).xyz), ro = uCam;
  // line channels: x = [S II], y = H-alpha, z = [O III]
  float glow = fbm(rd*3.0 + vec3(0.0, uTime*0.004, 0.0));
  vec3 toC = normalize(uCluster);
  float nearC = pow(max(dot(rd, toC), 0.0), 2.0);
  vec3 sky = vec3(0.05, 0.42, 0.10 + 0.45*nearC) * (0.45 + 1.2*glow*glow);
  vec3 lines = vec3(0.0); float T = 1.0;
  vec3 bmin = vec3(-2.6,-3.4,-1.6), bmax = vec3(2.6, 1.1, 1.6);
  vec3 t0 = (bmin - ro)/rd, t1 = (bmax - ro)/rd; vec3 tmin = min(t0,t1), tmax = max(t0,t1);
  float tn = max(max(tmin.x,tmin.y), max(tmin.z, 0.0)), tf = min(min(tmax.x,tmax.y), tmax.z);
  if (tf > tn) {
    float ds = (tf - tn)/uSteps;
    float t = tn + ds*hash1(vec3(gl_FragCoord.xy, uTime));
    for (int i = 0; i < 120; i++){
      if (float(i) >= uSteps || T < 0.01) break;
      vec3 p = ro + rd*t; float d = sdCloud(p);
      if (d < 0.6) {
        float inside = smoothstep(0.005, -0.035, d);
        float rho = inside * (0.6 + 0.8*fbm3(p*7.0)) * (1.0 + 1.4*smoothstep(-0.6, 0.3, p.y));
        vec3 Lc = normalize(uCluster - p);
        float lit = smoothstep(0.0, 0.1, min(sdCloud(p + Lc*0.12), sdCloud(p + Lc*0.45)));
        float sun = 1.0 - smoothstep(-0.02, 0.06, sdCloud(p - Lc*0.16));
        float hgt = mix(0.25, 1.0, smoothstep(-0.7, 0.35, p.y));
        float out_ = step(0.0, d);
        // H-alpha: the ionised skin and its photoevaporation flow
        float hA = 1.1 * exp(-abs(d - 0.012)/0.03) * lit * hgt + 0.7 * exp(-max(d,0.0)/0.08) * out_ * lit * sun;
        // [S II]: a thinner layer right at the ionisation front, just inside the skin
        float sII = 0.5 * exp(-abs(d + 0.006)/0.02) * lit * hgt;
        // [O III]: needs >35 eV photons, so it glows farther out in the flow, away from the surface
        float oIII = 0.5 * exp(-max(d,0.0)/0.3) * (1.0 - exp(-max(d,0.0)/0.07)) * out_ * lit * sun;
        lines += T * vec3(sII, hA, oIII) * ds * 7.0;
        T *= exp(-rho * ds * 70.0);
        sky *= exp(-rho * ds * 70.0);
      }
      t += ds;
    }
  }
  lines += T * sky + vec3(T * stars(rd) * 0.35);   // stars: continuum, equal in every narrow filter
  vec3 col = (uCS * lines.x * uW.x + uCH * lines.y * uW.y + uCO * lines.z * uW.z) * 1.3;
  col = 1.0 - exp(-col); col = pow(col, vec3(0.9));
  gl_FragColor = vec4(col, 1.0);
}`;
  // Channel colours. Natural: each line at its own hue (nmColor; H-alpha plus H-beta's blue-green at 1/2.86).
  const PALETTES = {
    natural: { name: 'Natural colour', CS: [1.0, 0.02, 0.0], CH: [1.0, 0.24, 0.32], CO: [0.04, 0.8, 0.55], W: [1, 1, 1] },
    hubble: { name: 'Hubble palette, true ratios', CS: [1, 0, 0], CH: [0, 1, 0], CO: [0, 0, 1], W: [1, 1, 1] },
    balanced: { name: 'Hubble palette, balanced', CS: [1, 0, 0], CH: [0, 1, 0], CO: [0, 0, 1], W: [5.0, 0.75, 1.5] },
  };
  // True line strengths relative to H-alpha in this toy, set by the shader coefficients above:
  // [S II] doublet ~0.2-0.3 and [O III] 500.7 ~0.5 (illustrative H II region ratios).
  P.demo3d('fig-light-palette', (ctx) => {
    const cam = new THREE.PerspectiveCamera(36, 1, 0.05, 50);
    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(0.05, -0.6, 0), dist: 4.7, yaw: -0.5, pitch: 0.06, auto: 0.035 });
    const U = {
      uInvProj: { value: new THREE.Matrix4() }, uCamWorld: { value: new THREE.Matrix4() }, uCam: { value: new THREE.Vector3() },
      uCluster: { value: new THREE.Vector3(1.4, 2.4, 0.9) }, uTime: { value: 0 }, uSteps: { value: 80 },
      uCS: { value: new THREE.Vector3() }, uCH: { value: new THREE.Vector3() }, uCO: { value: new THREE.Vector3() }, uW: { value: new THREE.Vector3(1, 1, 1) },
    };
    const mat = new THREE.ShaderMaterial({ uniforms: U, vertexShader: 'varying vec2 vUv; void main(){ vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }', fragmentShader: PAL_FRAG, depthTest: false, depthWrite: false });
    const scene = new THREE.Scene(); const quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), mat); quad.frustumCulled = false; scene.add(quad);
    const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    const mode = ctx.choice({ label: 'Palette', options: [{ label: 'Tour', value: 'tour' }, { label: 'Natural', value: 'natural' }, { label: 'Hubble', value: 'hubble' }, { label: 'Hubble, balanced', value: 'balanced' }], value: 'tour' });
    const solo = ctx.choice({ label: 'Lines', options: [{ label: 'All lines', value: 'all' }, { label: '[S II]', value: 's' }, { label: 'Hα', value: 'h' }, { label: '[O III]', value: 'o' }], value: 'all' });
    const cur = { CS: [1, 0.02, 0], CH: [1, 0.24, 0.32], CO: [0.04, 0.8, 0.55], W: [1, 1, 1] };
    const TOUR = ['natural', 'hubble', 'balanced'];
    let T = 0, tourName = 'natural';
    return {
      scale: 0.62,
      update(t, dt) {
        dt = Math.max(0, dt || 0); T += dt; orb.step(dt);
        if (mode.value === 'tour') tourName = TOUR[Math.floor(T / 6) % 3];
        const pal = PALETTES[mode.value === 'tour' ? tourName : mode.value];
        const k = Math.min(1, dt * 2.5);
        const m = { s: [1, 0, 0], h: [0, 1, 0], o: [0, 0, 1], all: [1, 1, 1] }[solo.value];
        for (const key of ['CS', 'CH', 'CO', 'W']) for (let i = 0; i < 3; i++) cur[key][i] += (pal[key][i] - cur[key][i]) * k;
        U.uCS.value.set(...cur.CS); U.uCH.value.set(...cur.CH); U.uCO.value.set(...cur.CO);
        U.uW.value.set(cur.W[0] * m[0], cur.W[1] * m[1], cur.W[2] * m[2]);
      },
      renderTo(r, pw, ph) {
        cam.updateMatrixWorld(); cam.updateProjectionMatrix();
        U.uInvProj.value.copy(cam.projectionMatrixInverse); U.uCamWorld.value.copy(cam.matrixWorld);
        U.uCam.value.setFromMatrixPosition(cam.matrixWorld); U.uTime.value = T;
        r.render(scene, ortho);
      },
      overlay(g, w, h) {
        const name = mode.value === 'tour' ? tourName : mode.value;
        const pal = PALETTES[name], px = w < 560 ? 10 : 12;
        const sw = (c) => css(c.map((v) => Math.min(1, v * 1.1)));
        const bal = name === 'balanced';
        P.hud(g, 10, 10, [
          ['palette', pal.name],
          ['[S II] 671.6/673.1 →', name === 'natural' ? 'deep red' : 'red', sw(pal.CS)],
          ['Hα 656.3 →', name === 'natural' ? 'red (+ Hβ)' : 'green', sw(pal.CH)],
          ['[O III] 500.7 →', name === 'natural' ? 'teal' : 'blue', sw(pal.CO)],
          ['brightness', bal ? 'each line scaled up' : 'true line ratios'],
        ], { px });
      },
    };
  }, { aspect: 0.6 });

  // =====================================================================
  // 3. Dust: a beam of starlight at six wavelengths crossing a pillar
  // =====================================================================
  const BANDS = [
    { um: 0.45, label: '0.45 µm', c: [0.25, 0.42, 1.0] },
    { um: 0.55, label: '0.55 µm', c: [0.55, 1.0, 0.25] },
    { um: 0.6563, label: 'Hα 0.66', c: [1.0, 0.18, 0.12] },
    { um: 1.25, label: 'J 1.25', c: [0.95, 0.55, 1.0] },
    { um: 2.2, label: 'K 2.2', c: [0.72, 0.42, 1.0] },
    { um: 4.4, label: '4.4 µm', c: [0.52, 0.36, 0.98] },
  ];
  // At 4.4 um use the measured mid-IR value A/A_V = 0.049 (Indebetouw et al. 2005) instead of the extrapolation.
  for (const b of BANDS) b.ratio = b.um > 4 ? 0.049 : ccm(b.um);
  // A_V through each column: N_H (Karim et al. 2023) / 2.2e21 cm^-2 per mag (Guver & Ozel 2009).
  const COLUMNS = [
    { av: 5, label: 'pillar edge, A_V 5' },
    { av: 30, label: 'P3 head, A_V 30' },
    { av: 50, label: 'P2 head, A_V 50' },
    { av: 130, label: 'P1a core, A_V 130' },
  ];
  P.demo3d('fig-light-dust', (ctx) => {
    const scene = new THREE.Scene(); scene.background = new THREE.Color(0x04060a);
    const cam = new THREE.PerspectiveCamera(38, 1, 0.05, 80);
    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(0.1, 0, 0), dist: 8.2, yaw: 0.42, pitch: 0.16, auto: 0 });
    const colSel = ctx.choice({ label: 'Column', options: [{ label: 'Tour', value: -1 }].concat(COLUMNS.map((c, i) => ({ label: `A_V ${c.av}`, value: i }))), value: -1, onInput: () => resetStats() });
    const rnd = P.rng(5);
    const R = 0.95, STAR = new THREE.Vector3(-3.6, 0.05, 0), DET = 3.4, SPEED = 2.6, BEAM = 0.62;
    // pillar: translucent dust column with an ionised rim on the star side
    const pmat = new THREE.ShaderMaterial({
      uniforms: { uHeat: { value: 0 }, uOpa: { value: 1 }, uStar: { value: STAR }, uTime: { value: 0 } },
      vertexShader: `varying vec3 vW, vN; void main(){ vec4 w = modelMatrix*vec4(position,1.0); vW = w.xyz; vN = normalize(mat3(modelMatrix)*normal); gl_Position = projectionMatrix*viewMatrix*w; }`,
      fragmentShader: `uniform float uHeat, uOpa, uTime; uniform vec3 uStar; varying vec3 vW, vN;
        float h(vec3 p){ p = fract(p*0.3183 + 0.1); p *= 17.0; return fract(p.x*p.y*p.z*(p.x+p.y+p.z)); }
        float n3(vec3 x){ vec3 i = floor(x), f = fract(x); f = f*f*(3.0-2.0*f);
          return mix(mix(mix(h(i),h(i+vec3(1,0,0)),f.x), mix(h(i+vec3(0,1,0)),h(i+vec3(1,1,0)),f.x),f.y),
                     mix(mix(h(i+vec3(0,0,1)),h(i+vec3(1,0,1)),f.x), mix(h(i+vec3(0,1,1)),h(i+vec3(1,1,1)),f.x),f.y), f.z); }
        void main(){
          vec3 V = normalize(cameraPosition - vW), N = normalize(vN);
          float facing = abs(dot(N, V));
          float cl = 0.55*n3(vW*vec3(3.0,1.2,3.0) + vec3(0.0, uTime*0.02, 0.0)) + 0.45*n3(vW*7.0);
          float thick = facing * (0.75 + 0.5*cl);                   // chord length through the column, roughly
          float a = (1.0 - exp(-3.2*thick*uOpa)) * 0.92;
          float lit = clamp(dot(N, normalize(uStar - vW)), 0.0, 1.0);
          float rim = pow(1.0 - facing, 2.5) * lit;
          vec3 col = vec3(0.10, 0.055, 0.035)*(0.6 + 0.8*cl) + vec3(1.0, 0.28, 0.2)*rim*0.9 + vec3(0.9,0.35,0.12)*uHeat*thick*0.35;
          gl_FragColor = vec4(col, max(a, rim*0.8));
        }`,
      transparent: true, depthWrite: false,
    });
    const pillar = new THREE.Mesh(new THREE.CylinderGeometry(R * 0.82, R * 1.08, 5.2, 64, 1, true), pmat);
    pillar.renderOrder = 1; scene.add(pillar);
    const cap = new THREE.Mesh(new THREE.SphereGeometry(R * 0.82, 48, 24, 0, Math.PI * 2, 0, Math.PI / 2), pmat);
    cap.position.y = 2.6; cap.renderOrder = 1; scene.add(cap);
    // detector screen
    const det = new THREE.Mesh(new THREE.PlaneGeometry(1.8, 1.8), new THREE.MeshBasicMaterial({ color: 0x1a2030, transparent: true, opacity: 0.3, side: THREE.DoubleSide, depthWrite: false }));
    det.position.set(DET + 0.02, 0, 0); det.rotation.y = Math.PI / 2; scene.add(det);

    const NPH = 2200, TR = 3, NSP = 260, NBG = 500;
    const PT = makePoints(NPH * TR + NSP + NBG + 4); scene.add(PT.pts);
    const SP0 = NPH * TR, BG0 = SP0 + NSP, ST0 = BG0 + NBG;
    for (let i = 0; i < NBG; i++) { const u = rnd(), v = rnd(); const c = 0.5 + 0.5 * rnd(); PT.set(BG0 + i, (u - 0.5) * 40, (v - 0.5) * 22, -12 - rnd() * 6, [c, c * 0.95, 1.0], 0.05 + 0.08 * rnd() * rnd(), 0.35 + 0.4 * rnd()); }
    const ph = Array.from({ length: NPH }, () => ({ on: false }));
    const sparks = Array.from({ length: NSP }, () => ({ a: 0 }));
    let spawnAcc = 0, heat = 0, absorbedRate = 0;
    const stat = BANDS.map(() => ({ sent: 0, through: 0 }));
    let epoch = 0;
    function resetStats() { epoch++; for (const s of stat) { s.sent = 0; s.through = 0; } }
    let tourIdx = 0, tourT = 0, colIdx = 1;
    // Mean transmission over the beam: aim points uniform in a disk of radius BEAM; chord through a
    // cylinder of radius R at offset z is 2 sqrt(R^2 - z^2); tau scales with chord/(2R) (the centre gets the full A_V).
    function predicted(b, av) {
      let s = 0, n = 0;
      for (let i = 0; i < 40; i++) for (let j = 0; j < 40; j++) {
        const y = (i + 0.5) / 40 * 2 - 1, z = (j + 0.5) / 40 * 2 - 1; if (y * y + z * z > 1) continue;
        const zz = z * BEAM; s += Math.exp(-tauOf(b.ratio * av) * Math.sqrt(Math.max(0, R * R - zz * zz)) / R); n++;
      }
      return s / n;
    }
    let pred = BANDS.map((b) => predicted(b, COLUMNS[colIdx].av)), predFor = colIdx;
    let T = 0;
    return {
      scene, camera: cam, scale: 1,
      update(t, dt, pw, phh) {
        dt = Math.max(0, dt || 0); T += dt; orb.dist = ctx.w < 560 ? 11 : 8.2; sway(orb, T, dt, 0.42, 0.3);
        if (colSel.value === -1) { tourT += dt; if (tourT > 9) { tourT = 0; tourIdx = (tourIdx + 1) % COLUMNS.length; resetStats(); } colIdx = tourIdx; }
        else colIdx = colSel.value;
        if (predFor !== colIdx) { pred = BANDS.map((b) => predicted(b, COLUMNS[colIdx].av)); predFor = colIdx; }
        const av = COLUMNS[colIdx].av;
        pmat.uniforms.uTime.value = T;
        pmat.uniforms.uOpa.value = 0.6 + 0.3 * Math.log10(av);
        // emit photons: equal numbers at each wavelength
        spawnAcc += dt * 330;
        while (spawnAcc >= 1) {
          spawnAcc -= 1;
          const k = ph.findIndex((p) => !p.on); if (k < 0) break;
          const bi = Math.floor(rnd() * BANDS.length);
          let y, z; do { y = rnd() * 2 - 1; z = rnd() * 2 - 1; } while (y * y + z * z > 1);
          const aim = new THREE.Vector3(0, y * BEAM, z * BEAM);
          const p = STAR.clone().add(new THREE.Vector3(0.12, (rnd() - 0.5) * 0.12, (rnd() - 0.5) * 0.12));
          const v = aim.clone().sub(p).normalize().multiplyScalar(SPEED);
          Object.assign(ph[k], { on: true, p, v, b: bi, tauStar: -Math.log(1 - rnd()), tau: 0, ep: epoch, dead: false, fade: 1, hist: [p.clone(), p.clone(), p.clone()] });
        }
        let absorbedNow = 0;
        for (let k = 0; k < NPH; k++) {
          const q = ph[k], b0 = k * TR;
          if (!q.on) { for (let j = 0; j < TR; j++) PT.hide(b0 + j); continue; }
          const step = SPEED * dt;
          q.p.addScaledVector(q.v, dt);
          const inside = q.p.x * q.p.x + q.p.z * q.p.z < R * R && Math.abs(q.p.y) < 2.6;
          if (inside && !q.dead) {
            // d(tau) = (A_lambda/1.0857) * ds / (2R): the full diameter carries the column's A_V
            q.tau += tauOf(BANDS[q.b].ratio * av) * step / (2 * R); // photons launched before a column change keep flying but are not counted
            if (q.tau >= q.tauStar) {
              q.dead = true; absorbedNow++; if (q.ep === epoch) stat[q.b].sent++;
              const s = sparks.find((x) => x.a <= 0); if (s) { s.a = 1; s.p = q.p.clone(); }
            }
          }
          if (q.dead) q.fade -= dt * 6;
          if (q.p.x >= DET && !q.dead) { if (q.ep === epoch) { stat[q.b].through++; stat[q.b].sent++; } const s = sparks.find((x) => x.a <= 0); if (s) { s.a = 0.8; s.p = q.p.clone(); s.c = BANDS[q.b].c; s.det = true; } q.on = false; }
          if (q.fade <= 0 || q.p.x > DET + 0.1) q.on = false;
          q.hist.unshift(q.p.clone()); q.hist.length = TR;
          const c = BANDS[q.b].c;
          for (let j = 0; j < TR; j++) { const hp = q.hist[j]; PT.set(b0 + j, hp.x, hp.y, hp.z, c, 0.075 - j * 0.015, (q.on ? Math.max(0, q.fade) : 0) * (j === 0 ? 0.9 : 0.35)); }
        }
        absorbedRate += (absorbedNow / Math.max(dt, 1e-3) - absorbedRate) * Math.min(1, dt * 2);
        heat += (Math.min(1, absorbedRate / 330) - heat) * Math.min(1, dt);
        pmat.uniforms.uHeat.value = heat;
        sparks.forEach((s, i) => {
          if (s.a <= 0) { PT.hide(SP0 + i); return; }
          s.a -= dt * (s.det ? 1.4 : 2.2);
          PT.set(SP0 + i, s.p.x, s.p.y, s.p.z, s.det ? s.c : [1.0, 0.55, 0.2], s.det ? 0.16 : 0.06 + (1 - s.a) * 0.12, Math.max(0, s.a) * (s.det ? 0.7 : 0.6));
          if (s.a <= 0) s.det = false;
        });
        PT.set(ST0, STAR.x, STAR.y, STAR.z, [0.85, 0.9, 1.0], 0.5, 1.0);
        PT.set(ST0 + 1, STAR.x, STAR.y, STAR.z, [0.6, 0.75, 1.0], 2.2, 0.35);
        PT.commit(cam, phh);
      },
      overlay(g, w, h) {
        const av = COLUMNS[colIdx].av, narrow = w < 560, px = narrow ? 9 : 12;
        P.hud(g, 8, 8, narrow ? [['A_V', String(av)], ['absorbed', `${(heat * 100).toFixed(0)}%`]] : [['column', COLUMNS[colIdx].label], ['absorbed by dust', `${(heat * 100).toFixed(0)}%`]], { px });
        // transmission panel: bar = photons counted at the screen, tick = exp(-tau) averaged over the beam
        const rowH = px + (narrow ? 6 : 12), pw = narrow ? 170 : Math.min(250, w * 0.42), x0 = w - pw - 8;
        let y = narrow ? h - rowH * BANDS.length - 34 : 10;
        g.save();
        g.fillStyle = 'rgba(6,8,12,0.55)'; g.beginPath(); g.roundRect ? g.roundRect(x0, y, pw, rowH * BANDS.length + 26, 6) : g.rect(x0, y, pw, rowH * BANDS.length + 26); g.fill();
        g.font = font(px - 1); g.fillStyle = 'rgba(220,224,232,0.7)'; g.fillText('reaching the screen', x0 + 9, y + px + 4);
        y += px + 12;
        BANDS.forEach((b, i) => {
          const A = b.ratio * av, s = stat[i], frac = s.sent > 20 ? s.through / s.sent : 0;
          const yy = y + i * rowH;
          g.fillStyle = css(b.c.map((v) => 0.5 + 0.5 * v)); g.font = font(px - 1); g.fillText(b.label, x0 + 9, yy + px - 2);
          const bx = x0 + 9 + px * 5.6, bw = pw - (bx - x0) - 52;
          g.fillStyle = 'rgba(255,255,255,0.08)'; g.fillRect(bx, yy + 2, bw, px - 2);
          g.fillStyle = css(b.c, 0.85); g.fillRect(bx, yy + 2, bw * frac, px - 2);
          g.fillStyle = '#f2efe8'; g.fillRect(bx + bw * pred[i] - 1, yy, 2, px + 2);
          g.fillStyle = 'rgba(220,224,232,0.8)'; g.fillText(A < 10 ? `${A.toFixed(1)}m` : `${A.toFixed(0)}m`, bx + bw + 6, yy + px - 2);
        });
        g.restore();
        if (narrow) return;
        g.font = font(px - 1); g.fillStyle = 'rgba(220,224,232,0.55)';
        g.fillText('bar: photons counted · tick: e^−τ · m: A_λ in magnitudes', 10, h - 10);
      },
    };
  }, { aspect: 0.6 });
})();
