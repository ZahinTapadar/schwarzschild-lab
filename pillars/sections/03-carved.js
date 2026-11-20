// Section 03 "Carved by starlight": three self-playing scenes.
//  1. fig-carved-bubble : the cluster in 3D, ionising photons, a Strömgren bubble growing into the gas.
//  2. fig-carved-head   : close-up of a pillar head (shared volumetric pillars) with its photoevaporation flow.
//  3. fig-carved-shadow : GPU ping-pong simulation of an ionisation front eating a clumpy cloud (toy model).
(function () {
  'use strict';
  const C = P.C;
  const MYR = 1e6 * C.yr;
  const Q0 = 7e49;                       // canonical cluster ionising photon rate (facts.md)
  const px = (w) => (w < 520 ? 10 : 12);

  // ---- physics (cgs) ----
  // Strömgren radius: Q = (4π/3) R³ n² α_B  (Strömgren 1939; Osterbrock & Ferland 2006 §2.2)
  const stromgren = (Q, n) => Math.cbrt(3 * Q / (4 * Math.PI * n * n * C.alphaB));
  // Isothermal sound speed c_s = sqrt(kT / μ m_H)
  const soundSpeed = (T, mu) => Math.sqrt(C.kB * T / (mu * C.mH));
  // Photoevaporating head of radius ℓ at distance d (Bertoldi 1989):
  //   F = Q/4πd², n_IF = sqrt(F / α_B ℓ), ṁ = 1.4 m_H n_IF c_s, Ṁ = ṁ πℓ²
  function headPhysics(dpc, lpc) {
    const d = dpc * C.pc, l = lpc * C.pc;
    const F = Q0 / (4 * Math.PI * d * d);
    const nIF = Math.sqrt(F / (C.alphaB * l));
    const cs = soundSpeed(1e4, 0.61);
    const mdot = 1.4 * C.mH * nIF * cs;
    const Mdot = mdot * Math.PI * l * l;
    const Mhead = (4 / 3) * Math.PI * l * l * l * 1e5 * 2.8 * C.mH;      // n(H2) = 1e5 cm^-3, 2.8 m_H per H2 incl. He
    const Lstars = 2.2e6 * C.Lsun;                                        // O-star luminosity (facts.md §B)
    return {
      F, nIF, cs, Mdot, Mhead,
      Pgas: 2 * nIF * 1e4,                                                // P/k of the ionised skin
      Prad: Lstars / (4 * Math.PI * d * d * C.c * C.kB),                  // P/k of fully absorbed starlight
      fresh: nIF * cs / F,                                                // share of photons that ionise new atoms
    };
  }

  // ---- shared GLSL ----
  const NOISE = `
float hash1(vec3 p){ p = fract(p*0.3183099 + 0.1); p *= 17.0; return fract(p.x*p.y*p.z*(p.x+p.y+p.z)); }
float noise(vec3 x){ vec3 i = floor(x), f = fract(x); f = f*f*(3.0-2.0*f);
  return mix(mix(mix(hash1(i),hash1(i+vec3(1,0,0)),f.x), mix(hash1(i+vec3(0,1,0)),hash1(i+vec3(1,1,0)),f.x),f.y),
             mix(mix(hash1(i+vec3(0,0,1)),hash1(i+vec3(1,0,1)),f.x), mix(hash1(i+vec3(0,1,1)),hash1(i+vec3(1,1,1)),f.x),f.y), f.z); }
float fbm(vec3 p){ float a = 0.5, s = 0.0; for(int i=0;i<5;i++){ s += a*noise(p); p = p*2.03 + vec3(1.7,9.2,3.1); a *= 0.5; } return s; }
`;
  const SPRITE_FRAG = `
varying vec3 vCol; varying float vA;
void main(){ vec2 q = gl_PointCoord - 0.5; float r2 = dot(q,q)*4.0; if (r2 > 1.0) discard;
  float a = exp(-r2*4.0) + 0.25*exp(-r2*1.2); gl_FragColor = vec4(vCol * a * vA, 1.0); }`;
  const additive = (o) => new THREE.ShaderMaterial(Object.assign({ transparent: true, depthWrite: false, blending: THREE.AdditiveBlending }, o));

  // =====================================================================
  // 1. The cluster and its Strömgren bubble
  // =====================================================================
  P.demo3d('fig-carved-bubble', (ctx) => {
    const lq = ctx.slider({ label: 'Ionising photons Q', min: 48, max: 50.5, step: 0.005, value: Math.log10(Q0), format: v => P.sci(Math.pow(10, v), 2) + ' s⁻¹' });
    const ln = ctx.slider({ label: 'Gas density n', min: 1, max: 4, step: 0.01, value: 2, format: v => P.fmt(Math.pow(10, v), 2) + ' cm⁻³' });
    let restartAt = 0;
    const again = ctx.toggle({ label: 'Switch on again', value: false, onInput: () => { restartAt = 1; again.el.setAttribute('aria-pressed', 'false'); } });

    const scene = new THREE.Scene(); scene.background = new THREE.Color(0x05060a);
    const cam = new THREE.PerspectiveCamera(40, 1, 0.05, 100);
    const orb = P.orbit(ctx, cam, { dist: 6.2, yaw: 0.6, pitch: 0.28, auto: 0.07 });
    const rnd = P.rng(611);
    const RG = 2.6;                                          // display radius of the gas cloud

    // the O stars: blackbody colours from the Martins+2005 temperatures in facts.md §B
    const temps = [43900, 35900, 42900, 35900, 37900, 36900, 34900, 36900, 36000, 36900, 33900, 32900, 34000, 33000, 32500, 35000, 33500, 32000, 34500];
    const sp = new Float32Array(temps.length * 3), sc = new Float32Array(temps.length * 3), ss = new Float32Array(temps.length);
    temps.forEach((T, i) => {
      const r = 0.05 + 0.13 * Math.pow(rnd(), 0.7), a = rnd() * 6.283, z = rnd() * 2 - 1;
      sp.set([r * Math.sqrt(1 - z * z) * Math.cos(a), r * z, r * Math.sqrt(1 - z * z) * Math.sin(a)], 3 * i);
      sc.set(P.bbColor(T).map(v => v / 255), 3 * i);
      ss[i] = i === 0 ? 1.9 : 0.5 + 0.7 * (T - 32000) / 12000;
    });
    const starGeo = new THREE.BufferGeometry();
    starGeo.setAttribute('position', new THREE.BufferAttribute(sp, 3));
    starGeo.setAttribute('aCol', new THREE.BufferAttribute(sc, 3));
    starGeo.setAttribute('aSize', new THREE.BufferAttribute(ss, 1));
    const starMat = additive({
      uniforms: { uPix: { value: 1 }, uTime: { value: 0 } },
      vertexShader: `attribute vec3 aCol; attribute float aSize; uniform float uPix, uTime; varying vec3 vCol; varying float vA;
        void main(){ vec4 mv = modelViewMatrix*vec4(position,1.0); gl_Position = projectionMatrix*mv;
          vCol = aCol*1.15; vA = 0.85 + 0.15*sin(uTime*3.0 + position.x*40.0);
          gl_PointSize = uPix * aSize * 75.0 / -mv.z; }`,
      fragmentShader: SPRITE_FRAG,
    });
    scene.add(new THREE.Points(starGeo, starMat));

    // ionising photons: fly radially out of the cluster and are absorbed at the front
    const NPH = 7000, phDir = new Float32Array(NPH * 3), phSeed = new Float32Array(NPH);
    for (let i = 0; i < NPH; i++) {
      const z = rnd() * 2 - 1, a = rnd() * 6.283, s = Math.sqrt(1 - z * z);
      phDir.set([s * Math.cos(a), z, s * Math.sin(a)], 3 * i); phSeed[i] = rnd();
    }
    const phGeo = new THREE.BufferGeometry();
    phGeo.setAttribute('position', new THREE.BufferAttribute(phDir, 3));
    phGeo.setAttribute('aSeed', new THREE.BufferAttribute(phSeed, 1));
    const phMat = additive({
      uniforms: { uR: { value: 0.01 }, uTime: { value: 0 }, uPix: { value: 1 } },
      vertexShader: `attribute float aSeed; uniform float uR, uTime, uPix; varying vec3 vCol; varying float vA;
        void main(){
          float f = fract(uTime*0.45 + aSeed);
          float travel = f * (uR*1.08 + 0.05);
          float flash = exp(-pow((travel - uR)/(0.04 + 0.03*uR), 2.0));
          vA = (1.0 - step(uR + 0.02, travel)) * smoothstep(0.0, 0.08, f) * (0.35 + 0.6*flash) * clamp(uR*uR, 0.04, 1.0);
          vec4 mv = modelViewMatrix*vec4(position*min(travel, uR), 1.0); gl_Position = projectionMatrix*mv;
          vCol = mix(vec3(0.62,0.55,1.0), vec3(1.0,0.85,0.9), flash);
          gl_PointSize = uPix * (12.0 + 10.0*flash) / -mv.z; }`,
      fragmentShader: SPRITE_FRAG,
    });
    scene.add(new THREE.Points(phGeo, phMat));

    // atoms of the surrounding gas: neutral grey-brown outside, red inside, flickering when they recombine
    const NG = 14000, gp = new Float32Array(NG * 3), gs = new Float32Array(NG);
    for (let i = 0; i < NG; i++) {
      let x, y, z; do { x = rnd() * 2 - 1; y = rnd() * 2 - 1; z = rnd() * 2 - 1; } while (x * x + y * y + z * z > 1);
      gp.set([x * RG, y * RG, z * RG], 3 * i); gs[i] = rnd();
    }
    const gasGeo = new THREE.BufferGeometry();
    gasGeo.setAttribute('position', new THREE.BufferAttribute(gp, 3));
    gasGeo.setAttribute('aSeed', new THREE.BufferAttribute(gs, 1));
    const gasMat = additive({
      uniforms: { uR: { value: 0.01 }, uTime: { value: 0 }, uPix: { value: 1 }, uRate: { value: 1 } },
      vertexShader: `attribute float aSeed; uniform float uR, uTime, uPix, uRate; varying vec3 vCol; varying float vA;
        float h(float x){ return fract(sin(x)*43758.5453); }
        void main(){
          float r = length(position);
          float ion = 1.0 - smoothstep(uR*0.97, uR*1.03, r);
          float k = floor(uTime*4.0 + aSeed*37.0);
          float rec = step(1.0 - 0.012*uRate, h(aSeed*91.7 + k)) * ion;        // just recombined: a brief flash
          float ph = fract(uTime*4.0 + aSeed*37.0);
          rec *= 1.0 - ph;
          vCol = mix(vec3(0.42,0.33,0.26), vec3(1.0,0.22,0.16), ion);
          vCol = mix(vCol, vec3(0.75,0.9,1.0), rec);
          vA = mix(0.45, 0.5, ion) + 1.0*rec;
          vec4 mv = modelViewMatrix*vec4(position,1.0); gl_Position = projectionMatrix*mv;
          gl_PointSize = uPix * (12.0 + 4.0*ion + 12.0*rec) / -mv.z; }`,
      fragmentShader: SPRITE_FRAG,
    });
    scene.add(new THREE.Points(gasGeo, gasMat));

    // the ionised bubble itself: a volumetric fog integrated through the sphere in the fragment shader
    const fogMat = additive({
      side: THREE.BackSide,
      uniforms: { uR: { value: 0.01 }, uTime: { value: 0 } },
      vertexShader: `varying vec3 vW; void main(){ vec4 w = modelMatrix*vec4(position,1.0); vW = w.xyz; gl_Position = projectionMatrix*viewMatrix*w; }`,
      fragmentShader: `uniform float uR, uTime; varying vec3 vW;
        ${NOISE}
        void main(){
          vec3 ro = cameraPosition, rd = normalize(vW - ro);
          float b = dot(ro, rd), c = dot(ro,ro) - uR*uR, h = b*b - c;
          if (h <= 0.0) discard;
          h = sqrt(h); float t0 = max(-b - h, 0.0), t1 = -b + h, ds = (t1 - t0)/14.0;
          vec3 col = vec3(0.0);
          for (int i = 0; i < 14; i++){
            vec3 p = ro + rd*(t0 + ds*(float(i)+0.5));
            float r = length(p)/uR;
            float fb = fbm(p/uR*2.4 + vec3(0.0, uTime*0.05, 0.0)); float d = 0.2 + 2.2*fb*fb;
            vec3 em = mix(vec3(0.10,0.55,0.55), vec3(0.85,0.12,0.10), smoothstep(0.15, 0.85, r));   // [O III] inside, H-alpha outside
            em += vec3(1.0,0.42,0.14) * 4.0 * exp(-pow((1.0 - r)/0.04, 2.0));                     // the front
            col += em * d;
          }
          col *= ds / uR * 0.2;
          gl_FragColor = vec4(col, 1.0);
        }`,
    });
    const fog = new THREE.Mesh(new THREE.SphereGeometry(1, 48, 32), fogMat);
    scene.add(fog);

    // the pillars, 2 pc away: a small dark marker and a faint ring at that radius
    const pDir = new THREE.Vector3(0.75, -0.35, 0.55).normalize();
    const pillar = new THREE.Mesh(new THREE.CylinderGeometry(0.03, 0.06, 1, 10), new THREE.MeshBasicMaterial({ color: 0x3a2416 }));
    scene.add(pillar);
    const ringPts = []; for (let i = 0; i <= 96; i++) { const a = i / 96 * 6.2832; ringPts.push(new THREE.Vector3(Math.cos(a), 0, Math.sin(a))); }
    const ring = new THREE.Line(new THREE.BufferGeometry().setFromPoints(ringPts), new THREE.LineBasicMaterial({ color: 0x8a8f9a, transparent: true, opacity: 0.25 }));
    ring.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), new THREE.Vector3(0.2, 1, -0.3).normalize());
    scene.add(ring);

    let Rpc = 0, tau = 0, S = 0.3, hold = 0;   // front radius (pc), time in recombination times, display units per pc
    const tmpV = new THREE.Vector3();
    const st = {};
    return {
      scene, camera: cam, scale: 1,
      update(t, dt, pw) {
        orb.step(dt);
        const Q = Math.pow(10, lq.value), n = Math.pow(10, ln.value);
        const Rs = stromgren(Q, n) / C.pc, trec = 1 / (n * C.alphaB);
        if (restartAt) { Rpc = 0; tau = 0; hold = 0; restartAt = 0; }
        // Front growth: d(R³)/dt = 3Q/(4π n) − n α_B R³, so x = (R/R_S)³ relaxes to 1 with time constant t_rec (Spitzer 1978)
        const dtau = dt * 0.4;
        let x = Math.pow(Rpc / Rs, 3); x += (1 - x) * (1 - Math.exp(-dtau)); tau += dtau;
        // ...but no front outruns light: R ≤ c t (the formula above ignores light travel time)
        Rpc = Math.min(Rs * Math.cbrt(x), C.c * tau * trec / C.pc);
        if (tau > 6.5) { hold += dt; if (hold > 4) { Rpc = 0; tau = 0; hold = 0; } }   // replay: switch the stars on again
        S += (1.55 / Math.max(Rs, 2.3) - S) * Math.min(1, dt * 2);
        const R = Math.max(0.004, Rpc * S);
        fog.scale.setScalar(R); fogMat.uniforms.uR.value = R; fogMat.uniforms.uTime.value = t;
        phMat.uniforms.uR.value = R; phMat.uniforms.uTime.value = t;
        gasMat.uniforms.uR.value = R; gasMat.uniforms.uTime.value = t;
        const pix = pw / Math.max(1, ctx.w);
        [phMat, gasMat, starMat].forEach(m => { m.uniforms.uPix.value = pix * ctx.h / 400; });
        starMat.uniforms.uTime.value = t;
        const D = 2 * S;
        pillar.position.copy(pDir).multiplyScalar(D + 0.65 * S);
        pillar.scale.set(Math.max(0.6, S * 2), 1.3 * S, Math.max(0.6, S * 2));
        pillar.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), pDir);
        ring.scale.setScalar(D);
        Object.assign(st, { Q, n, Rs, Rpc, trec, tau, pillarAt: pillar.position });
      },
      overlay(g, w, h) {
        const f = px(w);
        const lit = st.Rpc > 2;
        P.hud(g, 12, 12, [
          ['Q', P.sci(st.Q, 2) + ' s⁻¹'],
          ['n', P.fmt(st.n, 2) + ' cm⁻³'],
          ['switched on', P.duration(st.tau * st.trec) + ' ago'],
          ['front R', P.fmt(st.Rpc, 2) + ' pc', '#ff8a6a'],
          ['Strömgren R_S', `${P.fmt(st.Rs, 2)} pc = ${P.fmt(st.Rs * 3.262, 2)} ly`, '#ff5d52'],
          ['recombination time', P.duration(st.trec)],
          ['pillars (2 pc)', lit ? 'lit' : 'in the dark', lit ? '#5fd0c4' : '#f2a541'],
        ], { px: f });
        // fill bar: R / R_S
        const bw = Math.min(200, w * 0.4), y = h - 22;
        g.fillStyle = 'rgba(255,255,255,0.12)'; g.fillRect(12, y, bw, 4);
        g.fillStyle = '#ff5d52'; g.fillRect(12, y, bw * P.clamp(st.Rpc / st.Rs, 0, 1), 4);
        g.font = `${f}px "JetBrains Mono", monospace`; g.fillStyle = 'rgba(220,224,232,0.7)';
        g.fillText('R / R_S', 12, y - 6);
        // label the pillars
        tmpV.copy(st.pillarAt).project(cam);
        if (tmpV.z < 1) {
          const sx = (tmpV.x * 0.5 + 0.5) * w, sy = (-tmpV.y * 0.5 + 0.5) * h;
          g.fillStyle = 'rgba(230,226,218,0.85)'; g.fillText('pillars', sx + 10, sy + 4);
        }
        g.textAlign = 'right'; g.fillStyle = 'rgba(220,224,232,0.45)'; g.fillText('drag to orbit', w - 12, h - 12); g.textAlign = 'left';
      },
    };
  }, { aspect: 0.62 });

  // =====================================================================
  // 2. Close-up of a pillar head boiling off (shared volumetric pillars + flow particles)
  // =====================================================================
  P.demo3d('fig-carved-head', (ctx) => {
    const dS = ctx.slider({ label: 'Distance to the cluster d', min: 1, max: 4, step: 0.05, value: 2, format: v => P.fmt(v, 3) + ' pc' });
    const V = P.pillarsVolume();
    V.uniforms.uSteps.value = 110; V.uniforms.uExposure.value = 1.55;
    const head = new THREE.Vector3(0.47, -0.12, 0.01), RH = 0.14;   // the middle pillar's head, ~0.1 pc radius plus its skin
    const cam = new THREE.PerspectiveCamera(36, 1, 0.01, 50);
    const orb = P.orbit(ctx, cam, { target: head.clone(), dist: 1.9, yaw: -0.8, pitch: 0.08, auto: 0.03 });
    const toC = V.uniforms.uCluster.value.clone().sub(head).normalize();

    // Flow particles leave the lit hemisphere along the surface normal at about the sound speed.
    // Colour follows the observed stratification: [S II] at the front, H-alpha, then [O III] farther out (Hester 1996; McLeod 2015).
    const N = 22000, rnd = P.rng(9), pos = new Float32Array(N * 3), seed = new Float32Array(N);
    for (let i = 0; i < N; i++) {
      let v; do { const z = rnd() * 2 - 1, a = rnd() * 6.283, s = Math.sqrt(1 - z * z); v = new THREE.Vector3(s * Math.cos(a), z, s * Math.sin(a)); } while (v.dot(toC) < 0.05 + 0.2 * rnd());
      pos.set([v.x, v.y, v.z], 3 * i); seed[i] = rnd();
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    geo.setAttribute('aSeed', new THREE.BufferAttribute(seed, 1));
    const mat = additive({
      uniforms: { uTime: { value: 0 }, uC: { value: head }, uToC: { value: toC }, uR: { value: RH }, uDens: { value: 1 }, uPix: { value: 1 } },
      vertexShader: `attribute float aSeed; uniform vec3 uC, uToC; uniform float uTime, uR, uDens, uPix; varying vec3 vCol; varying float vA;
        void main(){
          float f = fract(uTime*0.42 + aSeed);
          float s = f * 0.42;                                    // pc travelled off the surface
          vec3 n = normalize(position + 0.12*vec3(sin(aSeed*91.0), cos(aSeed*53.0), sin(aSeed*17.0)));
          vec3 p = uC + n*(uR + s) - uToC*s*s*0.6;               // flow bends away from the cluster as it is pushed
          float dens = pow(uR/(uR + s), 2.0);                    // density falls as (ℓ/(ℓ+r))²; drawn as sqrt to keep the stream visible
          vec3 sii = vec3(1.0,0.55,0.18), ha = vec3(1.0,0.16,0.12), o3 = vec3(0.2,0.85,0.8);
          float q = s/0.42;
          vCol = q < 0.08 ? mix(sii, ha, q/0.08) : mix(ha, o3, smoothstep(0.1, 0.6, q));
          vA = sqrt(dens) * uDens * smoothstep(0.0, 0.04, f) * 0.42;
          vec4 mv = modelViewMatrix*vec4(p,1.0); gl_Position = projectionMatrix*mv;
          gl_PointSize = uPix * (5.0 + 4.0*dens) / -mv.z; }`,
      fragmentShader: SPRITE_FRAG,
    });
    const scene = new THREE.Scene(); scene.add(new THREE.Points(geo, mat));

    let elapsed = 0, ph = headPhysics(2, 0.1);
    return {
      scale: 0.7,
      update(t, dt, pw) {
        orb.step(dt);
        ph = headPhysics(dS.value, 0.1);
        mat.uniforms.uTime.value = t;
        mat.uniforms.uDens.value = Math.sqrt(ph.nIF / 1350);
        mat.uniforms.uPix.value = pw / Math.max(1, ctx.w) * ctx.h / 400;
        V.uniforms.uSkin.value = 0.6 + 0.4 * Math.sqrt(ph.F / 1.46e11);
        elapsed += dt;
        if (elapsed * 11800 * C.yr * ph.Mdot > ph.Mhead) elapsed = 0;   // the head is gone: start over
      },
      renderTo(r, pw, ph2) {
        V.renderTo(r, cam, performance.now() / 1000, pw, ph2);
        r.autoClear = false; r.render(scene, cam); r.autoClear = true;
      },
      overlay(g, w, h) {
        const f = px(w), Myr = MYR;
        // particles cross 0.42 pc in 1/0.42 s at ~11.9 pc/Myr: one second of screen time is ~11,800 years
        const yrs = elapsed * 11800, lost = ph.Mdot * yrs * C.yr / C.Msun;
        P.hud(g, 12, 12, [
          ['flux F', P.sci(ph.F, 2) + ' cm⁻² s⁻¹', '#fff3d6'],
          ['skin density n_IF', P.fmt(ph.nIF, 3) + ' cm⁻³', '#ff5d52'],
          ['flow speed c_s', P.fmt(ph.cs / 1e5, 3) + ' km/s'],
          ['photons → new atoms', P.fmt(100 * ph.fresh, 2) + ' %'],
          ['gas push / light push', P.fmt(ph.Pgas / ph.Prad, 2) + '×'],
          ['mass loss, this head', P.fmt(ph.Mdot * Myr / C.Msun, 2) + ' M☉/Myr', '#f2efe8'],
          ['four such heads', P.fmt(4 * ph.Mdot * Myr / C.Msun, 2) + ' M☉/Myr', '#f2efe8'],
        ], { px: f });
        const bw = Math.min(220, w * 0.42), y = h - 24;
        g.font = `${f}px "JetBrains Mono", monospace`; g.fillStyle = 'rgba(220,224,232,0.75)';
        g.fillText(`${P.fmt(yrs, 3)} years: ${P.fmt(lost, 2)} of ${P.fmt(ph.Mhead / C.Msun, 2)} M☉ boiled off`, 12, y - 7);
        g.fillStyle = 'rgba(255,255,255,0.12)'; g.fillRect(12, y, bw, 4);
        g.fillStyle = '#f2a541'; g.fillRect(12, y, bw * P.clamp(lost / (ph.Mhead / C.Msun), 0, 1), 4);
      },
    };
  }, { aspect: 0.62 });

  // =====================================================================
  // 3. Shadowing: a GPU simulation of the ionisation front in a clumpy cloud (toy model)
  // =====================================================================
  P.demo3d('fig-carved-shadow', (ctx, THREE_, renderer) => {
    const W = 4, H = 2.4, NX = 480, NY = 288;                // pc; texel = 0.0083 pc
    const STAR = new THREE.Vector2(0.12, 1.2);
    const play = ctx.toggle({ label: 'Pause', value: false });
    const coresOn = ctx.toggle({ label: 'Dense cores', value: true, onInput: () => { dirty = true; } });
    const rs = ctx.toggle({ label: 'Restart', value: false, onInput: () => { tSim = 0; hold = 0; reset = 2; rs.el.setAttribute('aria-pressed', 'false'); } });
    const cores = [new THREE.Vector3(1.75, 0.62, 1), new THREE.Vector3(2.05, 1.3, 1), new THREE.Vector3(2.5, 0.95, 1), new THREE.Vector3(1.6, 1.85, 1)];
    // N_erode = n_IF c_s t with n_IF = 1,400 cm^-3 (d = 2 pc, ℓ = 0.1 pc) and c_s = 11.6 km/s: 4.96e22 cm^-2 per Myr
    const NIF = 1400, CS = 1.16e6, ERODE = NIF * CS * MYR / 1e22;   // in units of 1e22 cm^-2 per Myr
    let dirty = true, reset = 2, tSim = 0, hold = 0, grab = -1;

    const rtOpts = { type: THREE.HalfFloatType, format: THREE.RGBAFormat, minFilter: THREE.LinearFilter, magFilter: THREE.LinearFilter, depthBuffer: false, stencilBuffer: false };
    const densRT = new THREE.WebGLRenderTarget(NX, NY, rtOpts);
    let simA = new THREE.WebGLRenderTarget(NX, NY, rtOpts), simB = new THREE.WebGLRenderTarget(NX, NY, rtOpts);
    const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    const quadScene = (mat) => { const s = new THREE.Scene(); const m = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), mat); m.frustumCulled = false; s.add(m); return s; };
    const VERT = `varying vec2 vUv; void main(){ vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }`;
    const DOMAIN = `const vec2 DOM = vec2(${W.toFixed(1)}, ${H.toFixed(1)}); const vec2 TEX = vec2(${NX}.0, ${NY}.0);`;

    // Pass 1 (only when cores move): log10 density. Lognormal cloud (mean 1.5e4 cm^-3, σ_ln = 0.7) behind a rough
    // surface, plus Plummer cores n = 1.5e6 (1 + r²/a²)^(-5/2), a = 0.05 pc: central column ≈ 3e23 cm^-2 like P1a (Karim+2023).
    const densMat = new THREE.ShaderMaterial({
      uniforms: { uCores: { value: cores } },
      vertexShader: VERT,
      fragmentShader: `precision highp float; varying vec2 vUv; uniform vec3 uCores[4]; ${DOMAIN} ${NOISE}
        void main(){
          vec2 x = vUv*DOM;
          float g = clamp((fbm(vec3(x*1.25, 3.7)) - 0.5)/0.11, -3.0, 3.0);
          float surf = 1.15 + 0.12*clamp((fbm(vec3(0.3, x.y*1.25, 3.7)) - 0.5)/0.11, -3.0, 3.0);
          float inCloud = clamp((x.x - surf)/0.06 + 0.5, 0.0, 1.0);
          float n = 50.0 + inCloud*1.5e4*exp(0.7*g - 0.245);
          for (int i = 0; i < 4; i++){ vec2 d = (x - uCores[i].xy)/0.05; n += uCores[i].z * 1.5e6 * pow(1.0 + dot(d,d), -2.5); }
          gl_FragColor = vec4(log(n)/log(10.0), 0.0, 0.0, 1.0);
        }`,
    });
    const densScene = quadScene(densMat);

    // Pass 2 (every frame, ping-pong): column density to the star, ionisation state, evaporating flow.
    //   r: ionised (N < N_erode)   g: time it was ionised (Myr, -1 if neutral; kept from frame to frame)   a: column (1e22 cm^-2)
    const simMat = new THREE.ShaderMaterial({
      uniforms: { uDens: { value: densRT.texture }, uPrev: { value: null }, uStar: { value: STAR }, uT: { value: 0 }, uDt: { value: 0 }, uErode: { value: 0 }, uReset: { value: 0 } },
      vertexShader: VERT,
      fragmentShader: `precision highp float; varying vec2 vUv; uniform sampler2D uDens, uPrev; uniform vec2 uStar; uniform float uT, uDt, uErode, uReset; ${DOMAIN}
        void main(){
          vec2 x = vUv*DOM, s = uStar;
          // N = ∫ n ds along the straight ray from the cluster (96 samples), in units of 1e22 cm^-2
          float L = length(x - s), N = 0.0;
          for (int i = 0; i < 96; i++){ vec2 q = mix(s, x, (float(i)+0.5)/96.0); N += pow(10.0, texture2D(uDens, q/DOM).r); }
          N *= L/96.0 * 3.0857e18 / 1e22;
          float ion = step(N, uErode);
          vec4 prev = texture2D(uPrev, vUv);
          if (uReset > 0.5) prev = vec4(0.0, -1.0, 0.0, 0.0);
          float tion = ion > 0.5 ? (prev.g >= 0.0 ? prev.g : uT) : -1.0;
          gl_FragColor = vec4(ion, tion, 0.0, min(N, 60000.0));
        }`,
    });
    const simScene = quadScene(simMat);

    // Pass 3: display. 2.5D relief lit from the cluster, H-alpha skins, flows and photon streaks.
    const dispMat = new THREE.ShaderMaterial({
      uniforms: { uDens: { value: densRT.texture }, uSim: { value: null }, uStar: { value: STAR }, uT: { value: 0 }, uTime: { value: 0 }, uErode: { value: 0 } },
      vertexShader: VERT,
      fragmentShader: `precision highp float; varying vec2 vUv; uniform sampler2D uDens, uSim; uniform vec2 uStar; uniform float uT, uTime, uErode; ${DOMAIN} ${NOISE}
        void main(){
          vec2 x = vUv*DOM, d = x - uStar; float r = length(d);
          vec4 S = texture2D(uSim, vUv);
          float ln = texture2D(uDens, vUv).r;
          vec2 e = 1.0/TEX;
          float hx = texture2D(uDens, vUv+vec2(2.5*e.x,0.0)).r - texture2D(uDens, vUv-vec2(2.5*e.x,0.0)).r;
          float hy = texture2D(uDens, vUv+vec2(0.0,2.5*e.y)).r - texture2D(uDens, vUv-vec2(0.0,2.5*e.y)).r;
          vec3 nrm = normalize(vec3(-hx*1.2, -hy*1.2, 0.3));
          vec3 Ld = normalize(vec3(-d/r, 0.55));
          float lam = max(dot(nrm, Ld), 0.0);
          float ion = smoothstep(0.2, 0.8, S.r);
          // how close the gas is to the front, in column: FUV heats a warm layer just inside it
          float depth = max(S.a - uErode, 0.0);
          float warm = exp(-depth/0.6) * step(0.001, uT);
          // neutral cloud: dark dust with relief, warmed amber near the front
          float a = clamp((ln - 2.6)/2.6, 0.0, 1.0);
          vec3 dust = vec3(0.13,0.075,0.045)*(0.35 + 0.9*a) * (0.45 + 0.9*lam);
          dust += vec3(0.9,0.42,0.12) * warm * (0.25 + 0.9*lam) * 0.55;
          // ionised gas: faint H-alpha nebula, [O III] teal close to the stars
          vec3 neb = vec3(0.55,0.06,0.07)*(0.10 + 0.22*exp(-r/1.6)) + vec3(0.05,0.32,0.34)*0.35*exp(-r/0.5);
          neb *= 0.7 + 0.6*fbm(vec3(x*2.0, uTime*0.03));
          // photon streaks raking out of the cluster, only through ionised gas
          float ang = atan(d.y, d.x);
          float streak = smoothstep(0.72, 1.0, noise(vec3(ang*55.0, r*1.6 - uTime*2.2, 0.0))) * smoothstep(0.75, 1.0, noise(vec3(ang*9.0, uTime*0.4, 4.0)));
          neb += vec3(0.75,0.7,1.0) * streak * 1.3 / (0.5 + r);
          // skin halo: ionised neighbours of neutral gas (and vice versa) within a few texels
          float halo = 0.0;
          for (int i = 0; i < 12; i++){
            float an = float(i)*0.5236; vec2 o = vec2(cos(an), sin(an)) * (1.0 + mod(float(i),3.0)*0.9) / TEX;
            halo += abs(texture2D(uSim, vUv + o).r - S.r);
          }
          halo /= 12.0;
          float side = mix(1.0, 0.35, ion);                      // the bright layer is the lit surface of the neutral gas
          vec3 skin = (vec3(1.0,0.22,0.15)*halo*2.0 + vec3(1.0,0.62,0.3)*halo*halo*2.5) * side * step(0.001, uT);
          float fresh = S.g >= 0.0 ? exp(-(uT - S.g)/0.03) : 0.0;
          // photoevaporation flow: ionised gas on the star-facing side of a surface, thinning as (ℓ/(ℓ+r))² over ~0.1 pc
          vec2 toS = -d/r; float fl = 0.0;
          for (int k = 1; k <= 10; k++){ float rr = float(k)*0.018; vec2 q = (x - toS*rr)/DOM; float nb = (1.0 - texture2D(uSim, q).r) * step(0.0, q.x) * step(q.x, 1.0) * step(0.0, q.y) * step(q.y, 1.0); fl = max(fl, nb * pow(0.1/(0.1 + rr), 2.0)); }
          fl *= 0.8 + 0.4*noise(vec3(x*30.0 + toS*uTime*3.0, uTime));
          vec3 flow = mix(vec3(0.15,0.75,0.72), vec3(1.0,0.25,0.18), smoothstep(0.2, 0.8, fl)) * fl * 0.9 * step(0.001, uT);
          vec3 col = mix(dust, neb, ion) + skin + flow*ion + vec3(1.0,0.5,0.3)*fresh*0.35*ion;
          // the cluster
          col += vec3(1.0,0.95,0.85) * (0.05/(r*r*60.0 + 0.05)) + vec3(0.6,0.75,1.0)*0.12*exp(-r*3.0);
          col = 1.0 - exp(-col*1.6);
          gl_FragColor = vec4(pow(col, vec3(0.9)), 1.0);
        }`,
    });
    const dispScene = quadScene(dispMat);

    const toPc = (x, y) => [x / ctx.w * W, (1 - y / ctx.h) * H];
    P.drag(ctx.canvas, (dx, dy, x, y, phase) => {
      const [X, Y] = toPc(x, y);
      if (phase === 'start') {
        grab = -1; let best = 0.18;
        if (coresOn.value) cores.forEach((c, k) => { const r = Math.hypot(c.x - X, c.y - Y); if (r < best) { best = r; grab = k; } });
      } else if (phase === 'move' && grab >= 0) {
        cores[grab].x = P.clamp(X, 0.8, W - 0.1); cores[grab].y = P.clamp(Y, 0.1, H - 0.1); dirty = true;
      } else if (phase === 'end') grab = -1;
    });

    let simDt = 0;
    return {
      scale: 1,
      update(t, dt) {
        simDt = 0;
        if (!play.value && dt > 0) {
          if (tSim < 3) { simDt = Math.min(dt, 0.05) * 0.16; tSim = Math.min(3, tSim + simDt); }
          else if ((hold += dt) > 3) { tSim = 0; hold = 0; reset = 2; }
        }
      },
      renderTo(r, pw, ph) {
        if (dirty) {
          cores.forEach(c => { c.z = coresOn.value ? 1 : 0; });
          r.setRenderTarget(densRT); r.render(densScene, ortho); dirty = false;
        }
        simMat.uniforms.uPrev.value = simA.texture;
        simMat.uniforms.uT.value = tSim; simMat.uniforms.uDt.value = simDt;
        simMat.uniforms.uErode.value = ERODE * tSim; simMat.uniforms.uReset.value = reset > 0 ? 1 : 0;
        if (reset > 0) reset--;
        r.setRenderTarget(simB); r.render(simScene, ortho);
        [simA, simB] = [simB, simA];
        dispMat.uniforms.uSim.value = simA.texture; dispMat.uniforms.uT.value = tSim;
        dispMat.uniforms.uTime.value = performance.now() / 1000; dispMat.uniforms.uErode.value = ERODE * tSim;
        r.setRenderTarget(null); r.setViewport(0, 0, pw, ph); r.setScissor(0, 0, pw, ph);
        r.render(dispScene, ortho);
      },
      overlay(g, w, h) {
        const f = px(w);
        P.hud(g, 12, 12, [
          ['since the stars lit', P.fmt(tSim, 2) + ' Myr'],
          ['front has eaten N', P.sci(Math.max(ERODE * tSim * 1e22, 1), 2) + ' cm⁻²', '#ff5d52'],
          ['rate n_IF c_s', P.sci(NIF * CS, 2) + ' cm⁻² s⁻¹'],
          ['core centre column', '3.1 × 10²³ cm⁻²'],
        ], { px: f });
        const bw = Math.min(200, w * 0.36), y = h - 20;
        g.fillStyle = 'rgba(255,255,255,0.12)'; g.fillRect(12, y, bw, 4);
        g.fillStyle = '#ff5d52'; g.fillRect(12, y, bw * tSim / 3, 4);
        g.font = `${f}px "JetBrains Mono", monospace`; g.fillStyle = 'rgba(220,224,232,0.7)'; g.fillText('0 → 3 Myr', 12, y - 6);
        if (coresOn.value) {
          g.setLineDash([2, 4]); g.strokeStyle = 'rgba(230,226,218,0.45)';
          cores.forEach((c, k) => { g.lineWidth = k === grab ? 2 : 1; g.beginPath(); g.arc(c.x / W * w, (1 - c.y / H) * h, 14, 0, 7); g.stroke(); });
          g.setLineDash([]); g.lineWidth = 1;
        }
        g.textAlign = 'right'; g.fillStyle = 'rgba(220,224,232,0.45)'; g.fillText('drag the dashed cores', w - 12, h - 12); g.textAlign = 'left';
        // scale bar 0.5 pc
        const sb = 0.5 / W * w; g.fillStyle = 'rgba(220,224,232,0.6)'; g.fillRect(w - 12 - sb, h - 32, sb, 2);
        g.textAlign = 'right'; g.fillText('0.5 pc', w - 12, h - 38); g.textAlign = 'left';
      },
    };
  }, { aspect: 0.6 });
})();
