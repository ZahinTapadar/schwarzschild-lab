// Section 04 "A cradle and a clock" (round 2): three self-playing 3D scenes.
//   fig-cradle-collapse  particles: a cold clump fragments into Jeans-mass cores; one core falls in,
//                        spins up into a disk and launches jets with H2 bow shocks.
//   fig-cradle-race      ray-marched pillar head eroded by the ionization front, with an EGG inside.
//   fig-cradle-clock     the shared pillars volume, eroding over Myr until the first O star explodes.
(function () {
  'use strict';
  const C = P.C;
  const MU = 2.33;                       // mean mass per particle, molecular gas incl. He (facts.md §E)
  const MYR = 1e6 * C.yr;
  const AU_PC = C.pc / C.au;

  // ---------- physics (cgs) ----------
  // Isothermal sound speed c_s = sqrt(kT / (mu m_H)). facts.md §E.
  const soundSpeed = (T, mu = MU) => Math.sqrt(C.kB * T / (mu * C.mH));
  // rho = mu m_H n (n = number density of H2 molecules)
  const rhoOf = (n) => MU * C.mH * n;
  // Jeans length lambda_J = sqrt(pi c_s^2 / (G rho)). Jeans 1902; facts.md §E.
  const jeansLength = (T, n) => Math.sqrt(Math.PI * soundSpeed(T) ** 2 / (C.G * rhoOf(n)));
  // Jeans mass of a sphere of diameter lambda_J: (4 pi / 3) rho (lambda_J / 2)^3. facts.md §E.
  const jeansMass = (T, n) => (4 * Math.PI / 3) * rhoOf(n) * Math.pow(jeansLength(T, n) / 2, 3);
  // Free-fall time of a pressureless uniform sphere t_ff = sqrt(3 pi / (32 G rho)). facts.md §E.
  const freeFall = (n) => Math.sqrt(3 * Math.PI / (32 * C.G * rhoOf(n)));
  // Shu (1977) inside-out collapse: accretion rate ~ c_s^3 / G.
  const shuRate = (T) => Math.pow(soundSpeed(T), 3) / C.G;
  // Ionization-front advance into molecular gas: v = 1.4 m_H n_IF c_s(ion) / rho_mol. facts.md §F4.
  const frontSpeed = (nIF, csIon, nMol) => 1.4 * C.mH * nIF * csIon / rhoOf(nMol);
  // Centrifugal radius r_c = j^2 / (G M) for specific angular momentum j = Omega R^2 (conserved).
  const centrifugal = (omega, R, M) => Math.pow(omega * R * R, 2) / (C.G * M);

  // Pressureless collapse, exact cycloid: r/r0 = cos^2(b), t/t_ff = (2/pi)(b + sin b cos b).
  // Tabulated r/r0 against t/t_ff in [0, 1].
  const CYC = new Float32Array(1025);
  (function () {
    for (let i = 0; i <= 1024; i++) {
      const tau = i / 1024; let lo = 0, hi = Math.PI / 2;
      for (let k = 0; k < 40; k++) { const b = (lo + hi) / 2; if ((2 / Math.PI) * (b + Math.sin(b) * Math.cos(b)) < tau) lo = b; else hi = b; }
      CYC[i] = Math.pow(Math.cos((lo + hi) / 2), 2);
    }
    CYC[1024] = 0;
  })();
  const cyc = (q) => q <= 0 ? 1 : q >= 1 ? 0 : CYC[Math.floor(q * 1024)];

  // Total (H + He + C burning) lifetimes in Myr, non-rotating..rotating: Ekström et al. 2012 via facts.md §F8.
  const EK = [[20, 8.6, 10.4], [40, 4.9, 6.1], [60, 3.9, 4.8], [85, 3.4, 4.0], [120, 3.0, 3.5]];
  function lifetime(M) { // log-log interpolation, returns [min, max] Myr
    M = P.clamp(M, 20, 120);
    let i = 0; while (i < EK.length - 2 && M > EK[i + 1][0]) i++;
    const a = EK[i], b = EK[i + 1], f = Math.log(M / a[0]) / Math.log(b[0] / a[0]);
    const ip = (k) => Math.exp(Math.log(a[k]) + f * (Math.log(b[k]) - Math.log(a[k])));
    return [ip(1), ip(2)];
  }

  // ---------- helpers ----------
  const ease = (x) => { x = P.clamp(x, 0, 1); return x * x * (3 - 2 * x); };
  const yrs = (y) => y >= 1e6 ? `${(y / 1e6).toFixed(2)} Myr` : `${Math.round(y).toLocaleString('en-US')} yr`;
  const hudPx = (w) => (w < 520 ? 10 : 12);

  // Additive soft-sprite points with per-vertex colour and size. Size is in px at distance uRef.
  function pointsMaterial() {
    return new THREE.ShaderMaterial({
      uniforms: { uMap: { value: P.dotTexture() }, uPx: { value: 1 }, uRef: { value: 1 }, uFade: { value: 1 } },
      vertexShader: `attribute float aSize; attribute vec3 aColor; uniform float uPx, uRef; varying vec3 vC;
        void main(){ vec4 mv = modelViewMatrix * vec4(position, 1.0); vC = aColor;
          float z = max(-mv.z, 1e-4); vC *= smoothstep(0.12 * uRef, 0.5 * uRef, z);   // fade sprites that pass close to the camera
          gl_PointSize = clamp(aSize * uPx * uRef / z, 0.0, 22.0); gl_Position = projectionMatrix * mv; }`,
      fragmentShader: `uniform sampler2D uMap; uniform float uFade; varying vec3 vC;
        void main(){ vec4 s = texture2D(uMap, gl_PointCoord); gl_FragColor = vec4(vC * s.a * uFade, 1.0); }`,
      blending: THREE.AdditiveBlending, depthTest: false, depthWrite: false, transparent: true,
    });
  }
  // A starfield shell that follows the camera.
  function starShell(n, seed) {
    const R = P.rng(seed), pos = new Float32Array(n * 3), col = new Float32Array(n * 3), size = new Float32Array(n);
    for (let i = 0; i < n; i++) {
      const z = 2 * R() - 1, a = 2 * Math.PI * R(), s = Math.sqrt(1 - z * z);
      pos.set([400 * s * Math.cos(a), 400 * z, 400 * s * Math.sin(a)], i * 3);
      const T = 3000 + 25000 * Math.pow(R(), 3), [r, g, b] = P.bbColor(T), br = 0.25 + 0.9 * Math.pow(R(), 6);
      col.set([r / 255 * br, g / 255 * br, b / 255 * br], i * 3); size[i] = 1.2 + 2.5 * Math.pow(R(), 4);
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    geo.setAttribute('aColor', new THREE.BufferAttribute(col, 3));
    geo.setAttribute('aSize', new THREE.BufferAttribute(size, 1));
    const m = pointsMaterial(); m.uniforms.uRef.value = 400;
    const pts = new THREE.Points(geo, m); pts.frustumCulled = false; pts.renderOrder = -1;
    return pts;
  }

  // =====================================================================
  // 1. Collapse: fragmentation -> free fall -> disk -> jets with H2 bow shocks (toy particle model)
  // =====================================================================
  P.demo3d('fig-cradle-collapse', (ctx) => {
    const tempChoice = ctx.choice({ label: 'gas temperature', value: 20,
      options: [{ label: '10 K', value: 10 }, { label: '20 K', value: 20 }, { label: '40 K', value: 40 }],
      onInput: () => build() });

    const U = 1000 * C.au;                      // scene unit = 1,000 au
    const PCU = C.pc / U;                       // ≈ 206 units per parsec
    const N0 = 3e4, NCORE = 1e5;                // clump and core densities (cm^-3)
    const RCL = 0.15 * PCU;                     // clump radius: 0.15 pc, a pillar head
    const OMEGA0 = 1e5 / C.pc;                  // initial spin 1 km/s per pc (Goodman et al. 1993)
    const VJET = 150e5;                         // jet speed 150 km/s (Hubble 1995->2014 jet)
    const NG = 34000, NJ = 8000, NB = 7000, N = NG + NJ + NB + 1;
    const T_A = 6, T_B = 8, T_C = 14, T_END = T_A + T_B + T_C + 2.5; // scene seconds
    const YRS_C = 400;                          // years shown in the jet phase

    const scene = new THREE.Scene(); scene.background = new THREE.Color(0x040509);
    const cam = new THREE.PerspectiveCamera(40, 1, 0.005, 2000);
    const stars = starShell(1800, 11); scene.add(stars);
    const pos = new Float32Array(N * 3), col = new Float32Array(N * 3), size = new Float32Array(N);
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(pos, 3).setUsage(THREE.DynamicDrawUsage));
    geo.setAttribute('aColor', new THREE.BufferAttribute(col, 3).setUsage(THREE.DynamicDrawUsage));
    geo.setAttribute('aSize', new THREE.BufferAttribute(size, 1).setUsage(THREE.DynamicDrawUsage));
    const mat = pointsMaterial();
    const pts = new THREE.Points(geo, mat); pts.frustumCulled = false; scene.add(pts);
    const DISKX = 4;
    // cold dim red -> orange -> white-gold
    const rgb = [0, 0, 0];
    const ramp = (h, o) => {
      const a = [0.55, 0.11, 0.08], m = [1.0, 0.46, 0.13], w = [1.0, 0.9, 0.72];
      const t = h < 0.5 ? h / 0.5 : (h - 0.5) / 0.5, A = h < 0.5 ? a : m, B = h < 0.5 ? m : w;
      o[0] = A[0] + (B[0] - A[0]) * t; o[1] = A[1] + (B[1] - A[1]) * t; o[2] = A[2] + (B[2] - A[2]) * t;
    };
    // haze sprites: world-sized soft glows (size in scene units, projected with uScale = ph / (2 tan(fov/2)))
    const hPos = new Float32Array(14 * 3), hCol = new Float32Array(14 * 3), hSize = new Float32Array(14);
    const hGeo = new THREE.BufferGeometry();
    hGeo.setAttribute('position', new THREE.BufferAttribute(hPos, 3));
    hGeo.setAttribute('aColor', new THREE.BufferAttribute(hCol, 3));
    hGeo.setAttribute('aSize', new THREE.BufferAttribute(hSize, 1));
    const hMat = new THREE.ShaderMaterial({
      uniforms: { uMap: { value: P.dotTexture() }, uScale: { value: 500 }, uFade: { value: 1 } },
      vertexShader: `attribute float aSize; attribute vec3 aColor; uniform float uScale; varying vec3 vC;
        void main(){ vec4 mv = modelViewMatrix * vec4(position, 1.0); vC = aColor;
          gl_PointSize = min(aSize * uScale / max(-mv.z, 1e-4), 2048.0); gl_Position = projectionMatrix * mv; }`,
      fragmentShader: `uniform sampler2D uMap; uniform float uFade; varying vec3 vC;
        void main(){ float r = length(gl_PointCoord - 0.5) * 2.0; float a = exp(-r * r * 3.5) * (1.0 - smoothstep(0.85, 1.0, r));
          gl_FragColor = vec4(vC * a * uFade, 1.0); }`,
      blending: THREE.AdditiveBlending, depthTest: false, depthWrite: false, transparent: true,
    });
    const haze = new THREE.Points(hGeo, hMat); haze.frustumCulled = false; haze.renderOrder = 0; pts.renderOrder = 1;
    scene.add(haze);
    const orb = P.orbit(ctx, cam, { dist: 90, yaw: 0.6, pitch: 0.32, auto: 0.07 });

    // per-particle state
    const pInit = new Float32Array(NG * 3), frag = new Uint8Array(NG);
    const Rc0 = new Float32Array(NG), z0 = new Float32Array(NG), phi = new Float32Array(NG), r0 = new Float32Array(NG), rcI = new Float32Array(NG), jI = new Float32Array(NG), shade = new Float32Array(NG);
    const jetB = new Float32Array(NJ), jetS = new Int8Array(NJ), jetV = new Float32Array(NJ), jetA = new Float32Array(NJ), jetR = new Float32Array(NJ);
    const bowK = new Float32Array(NB), bowS = new Int8Array(NB), bowRho = new Float32Array(NB), bowA = new Float32Array(NB);
    let S = null; // run parameters
    let clock = 0;

    function build() {
      const T = tempChoice.value, R = P.rng(7 + T);
      const MJ0 = jeansMass(T, N0) / C.Msun;
      const Mcl = rhoOf(N0) * (4 / 3) * Math.PI * Math.pow(RCL * U, 3) / C.Msun;
      const nf = P.clamp(Math.round(Mcl / MJ0), 1, 14);
      const Rc = jeansLength(T, NCORE) / 2 / U;           // core radius = lambda_J / 2 at 1e5 cm^-3
      const Mcore = jeansMass(T, NCORE);                  // grams
      const rcEdge = centrifugal(OMEGA0, Rc * U, Mcore) / U;
      // fragment centres (main one at the origin), random orientations and small delays
      const cs = [[0, 0, 0]], rots = [new THREE.Matrix4()], delay = [0];
      for (let tries = 0; cs.length < nf && tries < 4000; tries++) {
        const v = [R() * 2 - 1, R() * 2 - 1, R() * 2 - 1];
        if (Math.hypot(...v) > 1) continue;
        const c = v.map((x) => x * RCL * 0.85);
        if (cs.every((o) => Math.hypot(c[0] - o[0], c[1] - o[1], c[2] - o[2]) > Rc * 1.7)) {
          cs.push(c); rots.push(new THREE.Matrix4().makeRotationFromEuler(new THREE.Euler(R() * 6, R() * 6, R() * 6))); delay.push(-0.1 + 0.35 * R());
        }
      }
      const inv = rots.map((m) => m.clone().invert()), v3 = new THREE.Vector3();
      for (let i = 0; i < NG; i++) {
        let x, y, z; do { x = R() * 2 - 1; y = R() * 2 - 1; z = R() * 2 - 1; } while (x * x + y * y + z * z > 1);
        const k = 0.7 + 0.3 * R();
        x *= RCL * k; y *= RCL * k; z *= RCL * k;
        pInit.set([x, y, z], i * 3);
        let best = 0, bd = 1e9;
        for (let f = 0; f < cs.length; f++) { const d = Math.hypot(x - cs[f][0], y - cs[f][1], z - cs[f][2]); if (d < bd) { bd = d; best = f; } }
        frag[i] = best;
        // core-frame position: rho ~ r^-2 means radius uniform in [0, Rc]
        const r = Rc * Math.max(0.02, R());
        v3.set(x - cs[best][0], y - cs[best][1], z - cs[best][2]).normalize().applyMatrix4(inv[best]).multiplyScalar(r);
        r0[i] = r; Rc0[i] = Math.hypot(v3.x, v3.z); z0[i] = v3.y; phi[i] = Math.atan2(v3.z, v3.x);
        rcI[i] = rcEdge * Math.pow(Rc0[i] / Rc, 4);       // r_c ∝ j^2 ∝ R^4
        jI[i] = OMEGA0 * Math.pow(Rc0[i] * U, 2);
        shade[i] = 0.6 + 0.8 * R();
      }
      // jets: episodic bursts every ~50 yr (knots), both lobes
      // episodic accretion: irregular bursts (knots) launched at uneven intervals
      const bt = []; for (let b = -60, k = 0; b < YRS_C; k++) { bt.push(b); b += 25 + 55 * R(); }
      for (let i = 0; i < NJ; i++) { jetS[i] = R() < 0.5 ? 1 : -1; jetB[i] = R() * YRS_C * 1.15 - 60; jetV[i] = 0.9 + 0.2 * R(); jetA[i] = R() * 6.283; jetR[i] = Math.pow(R(), 1.5); }
      for (let i = 0; i < NB; i++) { bowS[i] = R() < 0.5 ? 1 : -1; bowK[i] = bt[Math.floor(R() * bt.length)]; bowRho[i] = Math.sqrt(R()); bowA[i] = R() * 6.283; }
      S = { T, nf, Rc, Mcore, rcEdge, cs, rots, delay, MJ0, Mcl, tffA: freeFall(N0) / C.yr * 0.8, tffB: freeFall(NCORE) / C.yr };
      clock = 0;
    }
    build();
    ctx.fig.__seek = (sec) => { clock = sec; }; // test hook: jump the scene clock

    const tmp = new THREE.Vector3();
    let hud = [], dist = 90;
    return {
      scale: 1,
      update(t, dt, pw, ph) {
        clock += dt; if (clock > T_END) clock = 0;
        const s = S, Rc = s.Rc;
        // phase clocks
        const sA = P.clamp(clock / T_A, 0, 1), eA = ease(sA);
        const uB = P.clamp((clock - T_A) / T_B, 0, 1) * 1.06;   // fraction of the core's free-fall time
        const sC = P.clamp((clock - T_A - T_B) / T_C, 0, 1);
        const yC = sC * YRS_C;
        let years, rate, phase;
        if (clock < T_A) { years = s.tffA * sA; rate = s.tffA / T_A; phase = 'fragmenting'; }
        else if (clock < T_A + T_B) { years = s.tffA + uB * s.tffB; rate = s.tffB * 1.06 / T_B; phase = 'free fall'; }
        else { years = s.tffA + 1.06 * s.tffB + yC; rate = YRS_C / T_C; phase = sC < 1 ? 'disk + jets' : 'disk + jets'; }
        const fade = Math.min(1, clock / 0.6, (T_END - clock) / 1.2);
        mat.uniforms.uFade.value = fade; hMat.uniforms.uFade.value = fade;

        // gas particles
                for (let i = 0; i < NG; i++) {
          const f = frag[i], c = s.cs[f];
          // inside-out: shell time ∝ r0, stretched x2.2 so an envelope is still falling in (Shu-like) while the jets run
          const q = (uB - s.delay[f]) / Math.max(0.03, 2.2 * r0[i] / Rc);
          const fr = clock < T_A ? 1 : cyc(q);
          // the disk is drawn DISKX times larger than its true j²/GM radius so it reads on screen (caption says so)
          const rD = s.rcEdge * DISKX * Math.pow(rcI[i] / s.rcEdge, 0.35);   // displayed disk radius (enlarged, compressed range)
          const Rcyl = Math.max(fr * Rc0[i], clock < T_A ? 0 : rD);
          const onDisk = clock > T_A && fr * Rc0[i] <= rD;
          const zz = z0[i] * fr * (onDisk ? 0.12 : 1); // parcels held up by spin settle into the midplane
          // display spin: true omega = j / R^2 at the true radius; compressed (power 0.3) so it stays watchable
          const om = jI[i] / Math.pow(Math.max(onDisk ? rcI[i] : Rcyl, 1e-6) * U, 2);
          phi[i] += Math.min(5, 0.12 * Math.pow(om / OMEGA0, 0.3)) * dt;
          tmp.set(Rcyl * Math.cos(phi[i]), zz, Rcyl * Math.sin(phi[i])).applyMatrix4(s.rots[f]);
          const tx = c[0] + tmp.x, ty = c[1] + tmp.y, tz = c[2] + tmp.z;
          const k = i * 3;
          pos[k] = P.lerp(pInit[k], tx, eA); pos[k + 1] = P.lerp(pInit[k + 1], ty, eA); pos[k + 2] = P.lerp(pInit[k + 2], tz, eA);
          // density-weighted colour: compression 1/fr^3 (log scale) plus closeness to a core centre
          const comp = clock < T_A ? 0 : Math.log10(1 / Math.pow(Math.max(fr, 1e-3), 3)) / 6;
          const heat = P.clamp(comp + eA * 0.45 * Math.pow(1 - r0[i] / Rc, 2) + (onDisk ? 0.55 : 0), 0, 1);
          ramp(heat, rgb);
          const b = (0.17 + 0.6 * heat * heat) * shade[i] * (0.55 + 0.45 * eA + (clock > T_A ? 0.3 : 0)) * (onDisk ? 0.3 : 1);
          col[k] = b * rgb[0]; col[k + 1] = b * rgb[1]; col[k + 2] = b * rgb[2];
          size[i] = (4.2 + 2.6 * shade[i]) * (onDisk ? 0.7 : 1);
        }
        // one faint haze sprite per core, sized to the core's current outer radius
        for (let f = 0; f < 14; f++) {
          if (f >= s.nf) { hSize[f] = 0; continue; }
          const rf = Rc * (clock < T_A ? P.lerp(2.2, 1, eA) : Math.max(cyc((uB - s.delay[f]) / 2.2), 0.25));
          hPos.set(s.cs[f], f * 3);
          const g = 0.07 * (0.4 + 0.6 * eA) * (clock > T_A ? 1.4 : 1);
          hCol.set([g, g * 0.42, g * 0.22], f * 3); hSize[f] = 2.6 * rf;
        }
        haze.geometry.attributes.position.needsUpdate = haze.geometry.attributes.aColor.needsUpdate = haze.geometry.attributes.aSize.needsUpdate = true;
        // jets (along the main core's spin axis = local y)
        const ax = new THREE.Vector3(0, 1, 0), px = new THREE.Vector3(1, 0, 0), pz = new THREE.Vector3(0, 0, 1);
        const vU = VJET * C.yr / U;                        // units per year
        for (let i = 0; i < NJ; i++) {
          const k = (NG + i) * 3, age = yC - jetB[i];
          if (sC <= 0 || age < 0) { size[NG + i] = 0; continue; }
          const L = vU * jetV[i] * age, pr = 0.06 * Math.sin(jetB[i] * 0.021);   // slow precession of the launch direction
          const rr = (0.006 + 0.01 * L) * jetR[i];
          pos[k] = jetS[i] * pr * L + rr * Math.cos(jetA[i]); pos[k + 1] = jetS[i] * L; pos[k + 2] = 0.5 * pr * L + rr * Math.sin(jetA[i]);
          const br = 0.6 * Math.exp(-age / 260);
          col[k] = br * 1.0; col[k + 1] = br * 0.55; col[k + 2] = br * 0.75;      // ionized jet: H-alpha / [S II] pink
          size[NG + i] = 2.6;
        }
        // bow shocks: paraboloid caps at the head of each burst, glowing in H2 (Webb's red, 4.7 µm line)
        for (let i = 0; i < NB; i++) {
          const k = (NG + NJ + i) * 3, age = yC - bowK[i];
          if (sC <= 0 || age < 0) { size[NG + NJ + i] = 0; continue; }
          const h = vU * age * 0.92, w = 0.08 + 0.2 * h, pr = 0.06 * Math.sin(bowK[i] * 0.021);
          const rho = bowRho[i] * w, along = h - 1.1 * rho * rho / w + 0.05 * w * Math.sin(7 * bowA[i]);
          pos[k] = bowS[i] * pr * h + rho * Math.cos(bowA[i]); pos[k + 1] = bowS[i] * along; pos[k + 2] = bowS[i] * 0.5 * pr * h + rho * Math.sin(bowA[i]);
          const br = 0.85 * (0.35 + 0.65 * bowRho[i]) * Math.min(1, age / 25) * Math.exp(-age / 360);
          col[k] = br; col[k + 1] = br * 0.16; col[k + 2] = br * 0.12;
          size[NG + NJ + i] = 4.6;
        }
        // protostar (blackbody-ish glow) at the main core's centre
        const sp = (NG + NJ + NB) * 3, grow = P.clamp((uB - 0.05) / 0.9, 0, 1);
        pos[sp] = pos[sp + 1] = pos[sp + 2] = 0;
        col[sp] = 1.0 * grow; col[sp + 1] = 0.86 * grow; col[sp + 2] = 0.66 * grow; size[N - 1] = 20 * grow;
        geo.attributes.position.needsUpdate = geo.attributes.aColor.needsUpdate = geo.attributes.aSize.needsUpdate = true;

        // camera: whole clump -> one core -> its disk -> pulling back along the jets
        const rOut = Rc * cyc(uB);
        const lg = (a, b, x) => Math.exp(P.lerp(Math.log(a), Math.log(b), ease(x)));
        let target = clock < T_A ? lg(RCL * 3.0, Rc * 3.2, sA) : lg(Rc * 3.2, Rc * 0.7, uB / 1.06);
        if (sC > 0) target = Math.max(sC < 0.25 ? s.rcEdge * DISKX * 4 : Rc * 0.5, vU * yC * 2.2);
        dist *= Math.exp(Math.log(target / dist) * Math.min(1, dt * 1.8));
        orb.dist = dist; orb.step(dt);
        stars.position.copy(cam.position);
        mat.uniforms.uRef.value = dist; mat.uniforms.uPx.value = pw / 900;
        hMat.uniforms.uScale.value = ph / (2 * Math.tan(20 * Math.PI / 180));
        stars.material.uniforms.uPx.value = pw / 900;

        // HUD: mean density inside the outer shell of the main core
        // central density: a uniform core of mean density NCORE following the cycloid, floored at the disk scale
        const n = clock < T_A ? N0 * Math.pow(1 / P.lerp(1, Math.cbrt(N0 / NCORE), eA), 3) : NCORE / Math.pow(Math.max(rOut, s.rcEdge) / Rc, 3);
        const nShow = sC > 0 ? NCORE : n;      // in the jet phase: the envelope still falling in
        hud = [
          ['phase', phase],
          ['elapsed', yrs(years)],
          ['clock rate', `${yrs(rate).replace(' yr', '')} yr / s`],
          ['T', `${s.T} K`],
          [sC > 0 ? 'envelope n(H₂)' : 'central n(H₂)', `${P.sci(nShow, 2)} cm⁻³`],
          ['Jeans mass M_J', `${P.fmt(jeansMass(s.T, nShow) / C.Msun, 2)} M☉`],
          ['free-fall t_ff', yrs(freeFall(nShow) / C.yr)],
        ];
        if (clock < T_A) hud.push(['clump / M_J', `${P.fmt(s.Mcl, 2)} / ${P.fmt(s.MJ0, 2)} M☉ → ${s.nf} cores`]);
        else hud.push(['disk radius j²/GM', `${Math.round(s.rcEdge * 1000)} au`]);
        if (sC > 0) hud.push(['jet speed', '150 km/s'], ['jet length', `${Math.round(vU * yC * 1000).toLocaleString('en-US')} au`, '#ff6a5a']);
      },
      scene, camera: cam,
      overlay(g, w, h) {
        P.hud(g, 12, 12, hud, { px: hudPx(w) });
        // scale bar
        const viewH = 2 * dist * Math.tan(20 * Math.PI / 180), auPerPx = viewH * 1000 / h;
        const target = auPerPx * w * 0.18, p10 = Math.pow(10, Math.floor(Math.log10(target)));
        const bar = [1, 2, 5].map((k) => k * p10).filter((v) => v <= target).pop() || p10;
        g.save(); g.strokeStyle = 'rgba(240,238,230,0.85)'; g.lineWidth = 2;
        g.beginPath(); g.moveTo(w - 16 - bar / auPerPx, h - 16); g.lineTo(w - 16, h - 16); g.stroke();
        g.font = `${hudPx(w)}px "JetBrains Mono", monospace`; g.fillStyle = 'rgba(240,238,230,0.85)'; g.textAlign = 'right';
        g.fillText(bar >= 1e5 ? `${P.fmt(bar / AU_PC, 2)} pc` : `${bar.toLocaleString('en-US')} au`, w - 16, h - 24);
        g.restore();
      },
    };
  }, { aspect: 0.62 });

  // =====================================================================
  // 2. The race: an EGG inside a pillar head while the ionization front eats down to it
  // =====================================================================
  const raceFrag = `
precision highp float;
varying vec2 vUv;
uniform mat4 uInvProj, uCamWorld;
uniform vec3 uCam, uStar;
uniform float uTime, uHead, uHeadR, uEggR, uEggY, uStarB, uSteps;
float hash1(vec3 p){ p = fract(p*0.3183099 + 0.1); p *= 17.0; return fract(p.x*p.y*p.z*(p.x+p.y+p.z)); }
float noise(vec3 x){ vec3 i = floor(x), f = fract(x); f = f*f*(3.0-2.0*f);
  return mix(mix(mix(hash1(i),hash1(i+vec3(1,0,0)),f.x), mix(hash1(i+vec3(0,1,0)),hash1(i+vec3(1,1,0)),f.x),f.y),
             mix(mix(hash1(i+vec3(0,0,1)),hash1(i+vec3(1,0,1)),f.x), mix(hash1(i+vec3(0,1,1)),hash1(i+vec3(1,1,1)),f.x),f.y), f.z); }
float fbm(vec3 p){ float a=0.5, s=0.0; for(int i=0;i<4;i++){ s += a*noise(p); p = p*2.03 + vec3(1.7,9.2,3.1); a *= 0.5; } return s; }
float capsule(vec3 p, vec3 a, vec3 b, float r){ vec3 pa=p-a, ba=b-a; float h=clamp(dot(pa,ba)/dot(ba,ba),0.0,1.0); return length(pa-ba*h)-r; }
float smin(float a, float b, float k){ float h = clamp(0.5+0.5*(b-a)/k, 0.0, 1.0); return mix(b, a, h) - k*h*(1.0-h); }
// units: 1 = 0.1 pc. Head of radius 0.1 pc whose top recedes as the front advances (uHead = y of the top).
float sdHead(vec3 p){ return capsule(p, vec3(0.0,-6.0,0.0), vec3(0.0, uHead - uHeadR, 0.0), uHeadR); }
float sdEgg(vec3 p){
  vec3 c = vec3(0.0, uEggY, 0.0);
  float egg = length(p - c) - uEggR;
  float stalk = capsule(p, c, vec3(0.0,-6.0,0.0), uEggR*0.62);   // shadowed tail below the globule
  return smin(egg, stalk, 0.25);
}
float sdAll(vec3 p){
  float n = (fbm(p*vec3(2.2,0.8,2.2)) - 0.5)*0.16 + (noise(p*9.0)-0.5)*0.025;
  return min(sdHead(p), sdEgg(p)) + n;
}
vec3 normalAt(vec3 p){ vec2 e = vec2(0.02, 0.0);
  return normalize(vec3(sdAll(p+e.xyy)-sdAll(p-e.xyy), sdAll(p+e.yxy)-sdAll(p-e.yxy), sdAll(p+e.yyx)-sdAll(p-e.yyx))); }
void main(){
  vec2 ndc = vUv*2.0 - 1.0;
  vec4 vp = uInvProj * vec4(ndc, 1.0, 1.0); vp /= vp.w;
  vec3 rd = normalize((uCamWorld * vec4(vp.xyz, 0.0)).xyz), ro = uCam;
  vec3 Ldir = normalize(vec3(0.35, 1.0, 0.45));        // toward the O stars, up and to the right
  float up = max(dot(rd, Ldir), 0.0);
  float g = fbm(rd*3.0 + vec3(0.0, uTime*0.01, 0.0));
  vec3 sky = mix(vec3(0.10,0.02,0.03), vec3(0.04,0.20,0.24), pow(up, 1.5)) * (0.4 + 1.1*g*g) + vec3(0.9,0.95,1.0)*pow(up, 80.0)*0.8;
  vec3 L = vec3(0.0); float T = 1.0, Tst = 1.0;
  vec3 bmin = vec3(-2.6,-6.0,-2.6), bmax = vec3(2.6, 2.6, 2.6);
  vec3 t0 = (bmin-ro)/rd, t1 = (bmax-ro)/rd, tmn = min(t0,t1), tmx = max(t0,t1);
  float tn = max(max(tmn.x,tmn.y), max(tmn.z,0.0)), tf = min(min(tmx.x,tmx.y), tmx.z);
  float tStar = dot(uStar - ro, rd);
  bool hit = false;
  if (tf > tn){
    float ds = (tf-tn)/uSteps, t = tn + ds*hash1(vec3(gl_FragCoord.xy, uTime));
    for (int i=0;i<128;i++){
      if (float(i) >= uSteps || T < 0.01) break;
      vec3 p = ro + rd*t;
      float d = sdAll(p);
      // photoevaporation flow: teal haze streaming off lit surfaces toward the stars
      if (d > 0.0 && d < 0.7){
        float litF = smoothstep(0.0, 0.3, sdAll(p + Ldir*0.6)) * (1.0 - smoothstep(-0.05, 0.12, sdAll(p - Ldir*(d + 0.25))));
        float streak = 0.55 + 0.9*noise(vec3(p.x*6.0, (p.y - uTime*0.25)*2.0, p.z*6.0));
        L += T * vec3(0.20,0.78,0.80) * exp(-d/0.25) * litF * streak * 1.3 * ds;
      }
      if (d < 0.0 && !hit){
        // first contact: shade the surface crisply (lambert from the stars + ionized H-alpha rim on the lit side)
        hit = true;
        // refine the hit by bisection so the silhouette is crisp
        float a = t - ds, b = t;
        for (int k = 0; k < 5; k++){ float m = 0.5*(a+b); if (sdAll(ro + rd*m) < 0.0) b = m; else a = m; }
        p = ro + rd*b;
        vec3 nrm = normalAt(p);
        float lit = smoothstep(0.0, 0.3, sdAll(p + Ldir*0.5)) * smoothstep(0.0, 0.3, sdAll(p + Ldir*1.4));
        float lam = max(dot(nrm, Ldir), 0.0);
        float rim = pow(1.0 - max(dot(nrm, -rd), 0.0), 2.5);
        float tex = 0.75 + 0.5*fbm(p*6.0);
        vec3 surf = vec3(0.075,0.035,0.022) * (0.35 + 1.4*lam*lit) * tex;      // cold dusty gas
        surf += vec3(1.0,0.30,0.18) * lit * (0.75*lam + 1.8*rim*sqrt(lam));    // H-alpha / [S II] rim facing the stars
        surf += vec3(1.0,0.7,0.45) * lit * pow(lam, 12.0) * 0.45;               // brightest where the front faces the cluster
        float egg = smoothstep(0.15, -0.1, sdEgg(p));
        surf *= 1.0 + 0.25*egg;
        L += T * surf;
        if (t < tStar) Tst = T;
        T *= 0.04;                     // dust: nearly opaque at visible wavelengths
      } else if (d < 0.0) {
        T *= exp(-ds * 9.0);
      }
      if (t < tStar) Tst = T;
      t += ds;
    }
  }
  // embedded protostar, shown as its near-infrared glow (seen through the dust)
  vec3 sv = uStar - ro; float perp = length(sv - rd*dot(sv,rd));
  float vis = max(Tst, 0.55) * step(0.0, tStar);
  vec3 star = uStarB * vis * (vec3(1.0,0.9,0.75)*exp(-perp*perp*700.0)*5.0 + vec3(1.0,0.55,0.25)*exp(-perp*perp*30.0)*0.45);
  vec3 col = L + T*sky + star;
  col *= 1.35; col = 1.0 - exp(-col); col = pow(col, vec3(0.9));
  gl_FragColor = vec4(col, 1.0);
}`;

  P.demo3d('fig-cradle-race', (ctx) => {
    const DEPTHS = [{ label: 'shallow', value: 0.052 }, { label: 'middle', value: 0.08 }, { label: 'deep', value: 0.125 }]; // pc, centre depth
    let auto = true, idx = 0;
    const pick = ctx.choice({ label: 'depth of the core', options: DEPTHS, value: DEPTHS[0].value,
      onInput: (v) => { auto = false; idx = DEPTHS.findIndex((d) => d.value === v); run = 0; } });
    // Toy inputs, canonical values: core 20 K, 1e5 cm^-3; front fed by n_IF = 1,400 cm^-3 at c_s = 10 km/s.
    const T = 20, n = 1e5;
    const vIF = frontSpeed(1400, 10e5, n) * MYR / C.pc;   // pc / Myr  (~0.086)
    const tff = freeFall(n) / MYR;                         // Myr       (~0.107)
    const rate = shuRate(T) * MYR / C.Msun;                // M☉ / Myr  (~4.5)
    const Mres = jeansMass(T, n) / C.Msun;                 // M☉        (~2.6)
    const rE = jeansLength(T, n) / C.pc / 2;               // pc        (~0.047)
    const massAt = (t) => t < tff ? 0 : Math.min(Mres, rate * (t - tff));
    const TMAX = 1.3, SECS = 15, HOLD = 3;

    const uniforms = {
      uInvProj: { value: new THREE.Matrix4() }, uCamWorld: { value: new THREE.Matrix4() }, uCam: { value: new THREE.Vector3() },
      uStar: { value: new THREE.Vector3() }, uTime: { value: 0 }, uHead: { value: 1 }, uHeadR: { value: 1 }, uEggR: { value: rE * 10 }, uEggY: { value: 0 },
      uStarB: { value: 0 }, uSteps: { value: 96 },
    };
    const mat = new THREE.ShaderMaterial({ uniforms, depthTest: false, depthWrite: false, fragmentShader: raceFrag,
      vertexShader: 'varying vec2 vUv; void main(){ vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }' });
    const quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), mat); quad.frustumCulled = false;
    const scene = new THREE.Scene(); scene.add(quad);
    const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    const cam = new THREE.PerspectiveCamera(36, 1, 0.05, 100);
    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(0, -0.9, 0), dist: 11, yaw: -0.9, pitch: 0.1, auto: 0.05 });
    let run = 0, hud = [], verdict = '';
    ctx.fig.__seek = (sec, k) => { run = sec; if (k != null) idx = k; }; // test hook
    return {
      scale: 0.7,
      update(t, dt) {
        run += dt;
        if (run > SECS + HOLD) { run = 0; if (auto) { idx = (idx + 1) % 3; pick.value = DEPTHS[idx].value; ctx.controls.querySelectorAll('.seg .btn').forEach((el, k) => el.setAttribute('aria-pressed', String(k === idx))); } }
        const d = DEPTHS[idx].value, tt = TMAX * P.clamp(run / SECS, 0, 1);
        const tU = Math.max(0, (d - rE) / vIF);            // front reaches the globule's edge
        const exposed = tt >= tU;
        // after exposure the globule itself slowly evaporates (cosmetic shrink)
        const r = rE * (exposed ? Math.max(0.55, 1 - 0.35 * (tt - tU)) : 1);
        uniforms.uHead.value = 1 - vIF * tt * 10;           // top of the head, in 0.1 pc units
        uniforms.uHeadR.value = 1 - 0.35 * vIF * tt * 10;   // flanks see the stars obliquely: eroded ~3x slower (toy)
        uniforms.uEggY.value = 1 - d * 10;
        uniforms.uEggR.value = r * 10;
        uniforms.uStar.value.set(0, 1 - d * 10, 0);
        const m = massAt(Math.min(tt, tU));
        uniforms.uStarB.value = m > 0 ? 0.25 + 0.9 * Math.sqrt(m / Mres) : 0;
        orb.step(dt);
        const state = tt < tff ? (exposed ? 'never formed' : 'core collapsing') : exposed ? (m >= Mres ? 'finished' : 'supply cut off') : (m >= Mres ? 'finished' : 'accreting 4.5 M☉/Myr');
        verdict = tU < tff ? 'an empty EGG' : massAt(tU) >= Mres ? 'a finished star' : massAt(tU) < 0.08 ? 'a brown dwarf' : `a stalled star of ${P.fmt(massAt(tU), 2)} M☉`;
        hud = [
          ['time', `${tt.toFixed(2)} Myr`],
          ['front advance', `${(vIF * tt).toFixed(3)} pc  (${vIF.toFixed(3)} pc/Myr)`, '#ff7a5c'],
          ['core depth', `${d.toFixed(3)} pc`],
          ['uncovered at', `${tU.toFixed(2)} Myr`],
          ['t_ff', `${tff.toFixed(2)} Myr`],
          ['protostar', `${P.fmt(m, 2)} / ${P.fmt(Mres, 2)} M☉`, '#ffd9a0'],
          ['state', state],
        ];
      },
      renderTo(r, pw, ph) {
        cam.updateMatrixWorld(); cam.updateProjectionMatrix();
        uniforms.uInvProj.value.copy(cam.projectionMatrixInverse); uniforms.uCamWorld.value.copy(cam.matrixWorld);
        uniforms.uCam.value.setFromMatrixPosition(cam.matrixWorld); uniforms.uTime.value = performance.now() / 1000;
        r.render(scene, ortho);
      },
      overlay(g, w, h) {
        const px = hudPx(w);
        P.hud(g, 12, 12, hud, { px });
        if (run > SECS) {
          g.save(); g.globalAlpha = P.clamp((run - SECS) / 0.6, 0, 1);
          g.font = `600 ${px + 6}px "Archivo", sans-serif`; g.textAlign = 'center'; g.fillStyle = '#f4efe6';
          g.fillText(verdict, w / 2, h - 28); g.restore();
        }
        g.save(); g.font = `${px}px "JetBrains Mono", monospace`; g.fillStyle = 'rgba(240,238,230,0.7)'; g.textAlign = 'right';
        g.fillText('↑ to the O stars', w - 14, 22); g.restore();
        // the ionization front: a line at the current top of the head, projected from the 3D scene
        const yF = uniforms.uHead.value - 0.02, r = uniforms.uHeadR.value + 0.45;
        const pa = new THREE.Vector3(-r, yF, 0).project(cam), pb = new THREE.Vector3(r, yF, 0).project(cam);
        const sx = (v) => (v.x * 0.5 + 0.5) * w, sy = (v) => (-v.y * 0.5 + 0.5) * h;
        g.save(); g.strokeStyle = 'rgba(255,110,90,0.9)'; g.lineWidth = 1.5; g.setLineDash([5, 4]);
        g.beginPath(); g.moveTo(sx(pa), sy(pa)); g.lineTo(sx(pb), sy(pb)); g.stroke(); g.setLineDash([]);
        g.font = `${px}px "JetBrains Mono", monospace`; g.fillStyle = 'rgba(255,140,120,0.95)'; g.textAlign = 'left';
        g.fillText('ionization front ↓', Math.max(sx(pa), sx(pb)) + 6, (sy(pa) + sy(pb)) / 2 + 4); g.restore();
      },
    };
  }, { aspect: 0.62 });

  // =====================================================================
  // 3. The clock: the shared pillars erode over Myr; the first O star explodes; its shock sweeps past
  // =====================================================================
  P.demo3d('fig-cradle-clock', (ctx) => {
    const RATES = [{ label: '70 M☉/Myr', value: 70 }, { label: '200 M☉/Myr', value: 200 }];
    const rateC = ctx.choice({ label: 'pillar mass-loss rate', options: RATES, value: 70, onInput: () => { clock = 0; } });
    const V = P.pillarsVolume();
    const mat = V.scene.children[0].material;
    // Patch the shared shader (a private copy of its material): erosion, shock stripping, shock shell, flash.
    let fs = mat.fragmentShader, patched = true;
    const swap = (a, b) => { if (fs.includes(a)) fs = fs.replace(a, b); else patched = false; };
    swap('uniform vec2 uRes;', 'uniform vec2 uRes;\nuniform float uErode, uStrip, uShockR, uShock, uFlash;');
    swap('return sdBase(p) + (n - 0.5)*0.24', 'float shk = smoothstep(uShockR, uShockR - 1.4, length(p - uCluster));\n  return sdBase(p) + uErode*(0.5 + 0.5*smoothstep(-1.6, 0.4, p.y)) + uStrip*shk + (n - 0.5)*0.24');
    swap('vec3 col = L + T*sky + stars*T;', `vec3 col = L + T*sky + stars*T;
  { vec3 oc = ro - uCluster; float bq = dot(oc, rd); float dmin = sqrt(max(dot(oc,oc) - bq*bq, 0.0));
    float w = 0.05 + 0.05*uShockR;
    float ring = exp(-pow((dmin - uShockR)/w, 2.0));
    float fill = smoothstep(uShockR, uShockR*0.6, dmin) * 0.10;
    vec3 sc = mix(vec3(1.0,0.38,0.32), vec3(0.6,0.88,1.0), smoothstep(uShockR - 3.0*w, uShockR, dmin));
    col += (ring*2.6 + fill*2.0) * sc * uShock * (0.4 + 0.6*T);
    float ang = 1.0 - dot(rd, normalize(uCluster - ro));
    col += vec3(1.0,0.95,0.86) * (uFlash*(exp(-ang*6000.0)*8.0 + exp(-ang*150.0)*1.2) + 0.25*exp(-ang*2500.0)); }`);
    if (patched) {
      mat.fragmentShader = fs;
      Object.assign(mat.uniforms, { uErode: { value: 0 }, uStrip: { value: 0 }, uShockR: { value: 0 }, uShock: { value: 0 }, uFlash: { value: 0 } });
      mat.needsUpdate = true;
    } else console.warn('04-cradle: pillars shader patch failed; showing the unmodified volume');
    const U = V.uniforms;
    U.uSteps.value = 100; U.uAmbient.value = 0.6;

    const cam = new THREE.PerspectiveCamera(42, 1, 0.05, 60);
    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(0.55, 0.35, 0.2), dist: 8.0, yaw: 0.3, pitch: 0.06, auto: 0.035 });
    const MP = 300;                                  // M☉, Karim et al. 2023
    const AGE = 2.0;                                 // cluster age today, Myr
    const [lo, hi] = lifetime(70);                   // HD 168076 primary ~50-80 M☉: take 70
    const SN = (lo + hi) / 2 - AGE;                  // Myr from now (~2.05)
    const VSH = 2000e5;                              // toy blast speed 2,000 km/s
    const PCKYR = VSH * 1000 * C.yr / C.pc;          // pc per 1,000 yr (~2.0)
    const T1 = 18, T2 = 10, T3 = 3;                  // seconds: erosion, shock (slow clock), fade
    const SHOCKYRS = 3000;
    let clock = 0, hud = [], years = 0, snYrs = -1, massLeft = MP;
    // distance from the cluster to the nearest pillar head (scene units = pc)
    const dPill = new THREE.Vector3(-0.22, 0.42, -0.42).distanceTo(U.uCluster.value);
    ctx.fig.__seek = (sec) => { clock = sec; }; // test hook
    return {
      scale: 0.7,
      update(t, dt) {
        clock += dt; if (clock > T1 + T2 + T3) clock = 0;
        const rate = rateC.value;
        let rateYr;
        if (clock < T1) { years = SN * 1e6 * clock / T1; snYrs = -1; rateYr = SN * 1e6 / T1; }
        else { snYrs = SHOCKYRS * P.clamp((clock - T1) / T2, 0, 1); years = SN * 1e6 + snYrs; rateYr = clock < T1 + T2 ? SHOCKYRS / T2 : 0; }
        massLeft = Math.max(0, MP - rate * years / 1e6);
        const frac = massLeft / MP;
        const shockR = snYrs >= 0 ? PCKYR * snYrs / 1000 : 0;
        if (patched) {
          const m = mat.uniforms;
          m.uErode.value = 0.36 * Math.pow(1 - frac, 0.8);
          m.uShockR.value = shockR;
          m.uShock.value = snYrs >= 0 ? Math.min(1, snYrs / 60) * Math.exp(-snYrs / 4000) : 0;
          m.uFlash.value = snYrs >= 0 ? Math.exp(-snYrs / 150) : 0.0;
          // after the shell passes, the remnant gas is stripped away within a few hundred years (toy)
          m.uStrip.value = 1.4 * ease((shockR - dPill) / 1.6);
        }
        const fade = Math.min(1, clock / 1.0, (T1 + T2 + T3 - clock) / 1.5);
        U.uExposure.value = 1.4 * Math.max(0.02, fade) * (snYrs >= 0 ? 1 + 0.5 * Math.exp(-snYrs / 200) : 1);
        orb.step(dt);
        hud = [
          ['from now', snYrs < 0 ? `+${(years / 1e6).toFixed(2)} Myr` : `+${(years / 1e6).toFixed(2)} Myr + ${Math.round(snYrs).toLocaleString('en-US')} yr`],
          ['clock rate', rateYr >= 1e4 ? `${Math.round(rateYr / 1000).toLocaleString('en-US')},000 yr / s` : `${Math.round(rateYr)} yr / s`],
          ['pillar gas left', `${Math.round(massLeft * (1 - (patched ? mat.uniforms.uStrip.value / 1.4 : 0)))} of ${MP} M☉`, '#e0a070'],
          ['mass-loss rate', `${rate} M☉/Myr`],
          ['HD 168076 primary', snYrs < 0 ? `~70 M☉, ${(AGE + years / 1e6).toFixed(2)} Myr old` : 'supernova', snYrs < 0 ? undefined : '#bfe6ff'],
        ];
        if (snYrs >= 0) hud.push(['blast radius', `${shockR.toFixed(2)} pc at 2,000 km/s`, '#bfe6ff']);
      },
      renderTo(r, pw, ph) { V.renderTo(r, cam, performance.now() / 1000, pw, ph); },
      overlay(g, w, h) {
        const px = hudPx(w);
        P.hud(g, 12, 12, hud, { px });
        // timeline ribbon that fills as the scene plays: now -> 3 Myr
        const x0 = 16, x1 = w - 16, y = h - 22, TM = 3;
        const X = (m) => x0 + (x1 - x0) * P.clamp(m / TM, 0, 1);
        g.save();
        g.fillStyle = 'rgba(255,255,255,0.12)'; g.fillRect(x0, y, x1 - x0, 4);
        g.fillStyle = 'rgba(224,160,112,0.35)'; g.fillRect(X(1), y - 3, X(3) - X(1), 10);           // published pillar lifetime 1–3 Myr
        g.fillStyle = 'rgba(191,230,255,0.45)'; g.fillRect(X(lo - AGE), y - 6, X(hi - AGE) - X(lo - AGE), 3); // 70 M☉ explosion window
        g.fillStyle = '#f4efe6'; g.fillRect(x0, y, X(years / 1e6) - x0, 4);
        g.beginPath(); g.arc(X(years / 1e6), y + 2, 4, 0, 6.283); g.fill();
        g.font = `${px - 1}px "JetBrains Mono", monospace`; g.fillStyle = 'rgba(240,238,230,0.75)';
        g.textAlign = 'left'; g.fillText('now', x0, y - 8);
        g.textAlign = 'right'; g.fillText('+3 Myr', x1, y - 8);
        g.textAlign = 'center';
        g.fillStyle = 'rgba(191,230,255,0.9)'; g.fillText('first supernova', X((lo + hi) / 2 - AGE), y - 10);
        if (w > 520) { g.fillStyle = 'rgba(224,160,112,0.9)'; g.fillText('pillars gone (published 1–3 Myr)', X(1.6), y + 18); }
        g.restore();
      },
    };
  }, { aspect: 0.62 });

  // Self-check against facts.md (console only).
  const okJ = Math.abs(jeansLength(20, 1e5) / C.pc - 0.095) < 0.003 && Math.abs(jeansMass(20, 1e5) / C.Msun - 2.6) < 0.1 &&
    Math.abs(freeFall(1e5) / C.yr - 1.07e5) < 3e3 && Math.abs(cyc(0.5) - 0.837) < 0.005;
  if (!okJ) console.warn('04-cradle: physics self-check failed');
})();
