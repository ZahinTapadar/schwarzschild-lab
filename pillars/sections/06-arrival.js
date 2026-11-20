// Section 6, "Arrival" (round 2): three self-playing 3D scenes on the shared volumetric pillars.
//   fig-arrival-3d       the 3D arrangement (McLeod et al. 2015 MUSE geometry) with the O stars as sprites
//   fig-arrival-approach automatic flight from Earth's distance into and through a pillar head
//   fig-arrival-eye      the same volume as a camera records it and as a dark-adapted eye would see it
// Units: scene units are parsecs; physics in cgs (P.C).
(function () {
  'use strict';
  const C = P.C;
  const D0 = 1740;                        // pc, Gaia distance (facts.md canonical)
  const PC_LY = C.pc / C.ly;              // 3.26 ly per pc
  // Cluster placed between P2 (in front) and P1 (behind) along our line of sight (McLeod 2015; facts.md C).
  const CLUSTER = [0.25, 2.2, -0.22];
  const MONO = '"JetBrains Mono", ui-monospace, Menlo, monospace';
  const small = (w) => (w < 520 ? 10 : 12);

  // ---------- JS copy of the volume's base distance field (lib/pillars-volume.js sdBase, without noise) ----------
  function capsule(p, a, b, ra, rb) {
    const pa = [p[0] - a[0], p[1] - a[1], p[2] - a[2]], ba = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    const h = P.clamp((pa[0] * ba[0] + pa[1] * ba[1] + pa[2] * ba[2]) / (ba[0] * ba[0] + ba[1] * ba[1] + ba[2] * ba[2]), 0, 1);
    return Math.hypot(pa[0] - ba[0] * h, pa[1] - ba[1] * h, pa[2] - ba[2] * h) - P.lerp(ra, rb, h);
  }
  function smin(a, b, k) { const h = P.clamp(0.5 + 0.5 * (b - a) / k, 0, 1); return P.lerp(b, a, h) - k * h * (1 - h); }
  function sdBase(p) {
    let d1 = capsule(p, [-0.75, -1.45, -0.55], [-0.35, 0.05, -0.45], 0.36, 0.17);
    d1 = smin(d1, capsule(p, [-0.35, 0.05, -0.45], [-0.22, 0.42, -0.42], 0.17, 0.2), 0.12);
    let d2 = capsule(p, [0.12, -1.45, -0.05], [0.42, -0.25, 0.0], 0.3, 0.13);
    d2 = smin(d2, capsule(p, [0.42, -0.25, 0.0], [0.5, -0.05, 0.02], 0.13, 0.12), 0.08);
    const d3 = capsule(p, [0.75, -1.45, 0.45], [0.92, -0.75, 0.5], 0.24, 0.1);
    const q = [(p[0] - 0.1) / 2.6, (p[1] + 2.9) / 1.45, p[2] / 1.3];
    const base = Math.hypot(q[0], q[1], q[2]) - 1;
    return smin(smin(smin(d1, d2, 0.18), d3, 0.18), base * 0.9, 0.3);
  }
  // Gas layers by distance to the surface (pc): [particles per cm^3, H nuclei per cm^3, T in K, name].
  // Interior n(H2) = 1.3e5 (Karim 2023); ionised skin n_e ~ 2e3, ~0.02 pc thick; H II cavity ~100 (facts.md C, F).
  function gasAt(p) {
    const d = sdBase(p);
    if (d < 0) return [1.3e5, 2.6e5, 15, 'molecular interior'];
    if (d < 0.02) return [4e3, 2e3, 1e4, 'ionised skin'];
    return [200, 100, 1e4, 'H II cavity'];
  }
  const LHC = 2.5e6;                        // molecules per cm^3 in the LHC beam pipe (1e-13 atm at room temperature, CERN)
  const AIR = 2.5e19;                       // molecules per cm^3 at sea level


  // Variant of the shared volume for far views: a larger bounding box and a soft fade of the cloud's base,
  // so the box edges never show when the whole cloud is in frame. Patched from the shared shader's source.
  function softenVolume(V) {
    const mat = V.scene.children[0].material;
    let f = mat.fragmentShader;
    const swaps = [
      ['vec3 bmin = vec3(-2.6,-3.4,-1.6), bmax = vec3(2.6, 1.1, 1.6);', 'vec3 bmin = vec3(-3.4,-4.6,-2.1), bmax = vec3(3.4, 1.2, 2.1);'],
      ['vec3 dtau = kappa * rho * ds * 70.0;', 'float fadeY = smoothstep(-4.2, -2.4, p.y) * smoothstep(3.2, 2.0, abs(p.x)) * smoothstep(2.0, 1.2, abs(p.z));\n        em *= fadeY;\n        vec3 dtau = kappa * rho * fadeY * ds * 70.0;'],
    ];
    for (const [a, b] of swaps) { if (f.indexOf(a) < 0) return V; f = f.replace(a, b); }   // shared shader changed: keep it as is
    mat.fragmentShader = f; mat.needsUpdate = true;
    return V;
  }

  function bbRGB(T) { return P.bbColor(T).map(v => v / 255); }
  function toScreen(v3, cam, w, h) {
    const q = new THREE.Vector3(v3[0], v3[1], v3[2]).project(cam);
    return { x: (q.x + 1) / 2 * w, y: (1 - q.y) / 2 * h, front: q.z < 1 };
  }
  function label(g, x, y, text, col, px) {
    g.save(); g.font = `500 ${px || 12}px ${MONO}`; g.textBaseline = 'middle';
    g.fillStyle = 'rgba(6,8,12,0.55)'; const w = g.measureText(text).width + 10;
    g.fillRect(x + 8, y - 9, w, 18);
    g.fillStyle = col; g.fillText(text, x + 13, y);
    g.strokeStyle = col; g.globalAlpha = 0.7; g.beginPath(); g.moveTo(x, y); g.lineTo(x + 8, y); g.stroke();
    g.restore();
  }
  function buttons(ctx, names, onPick) {
    const seg = document.createElement('div'); seg.className = 'seg';
    const bs = names.map(([text, key], i) => {
      const b = document.createElement('button'); b.className = 'btn'; b.type = 'button'; b.textContent = text;
      b.setAttribute('aria-pressed', String(i === 0));
      b.addEventListener('click', () => { bs.forEach(x => x.setAttribute('aria-pressed', String(x === b))); onPick(key); });
      seg.appendChild(b); return b;
    });
    ctx.controls.appendChild(seg);
  }

  // ---------- the 19 O stars of NGC 6611 as additive sprites ----------
  // HD 168076 (O3.5 V + O7.5 V, ~44,000 K) is the brightest; the rest span O4-O9 (Teff 33,000-43,000 K; facts.md B).
  function oStars() {
    const rnd = P.rng(6611), pos = [], col = [], size = [];
    for (let i = 0; i < 19; i++) {
      const r = 0.45 * Math.cbrt(rnd()), th = Math.acos(2 * rnd() - 1), ph = rnd() * 2 * Math.PI;
      const p = i === 0 ? [CLUSTER[0] - 0.1, CLUSTER[1] - 0.15, CLUSTER[2] + 0.05]
        : [CLUSTER[0] + r * Math.sin(th) * Math.cos(ph), CLUSTER[1] + r * Math.cos(th) * 0.8, CLUSTER[2] + r * Math.sin(th) * Math.sin(ph)];
      const T = i === 0 ? 43900 : 33000 + 10000 * rnd();
      pos.push(...p); col.push(...bbRGB(T)); size.push(i === 0 ? 1.0 : 0.35 + 0.35 * rnd());
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
    geo.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
    geo.setAttribute('size', new THREE.Float32BufferAttribute(size, 1));
    geo.setAttribute('vis', new THREE.Float32BufferAttribute(new Array(19).fill(1), 1));
    const mat = new THREE.ShaderMaterial({
      uniforms: { uTex: { value: P.dotTexture() }, uScale: { value: 300 }, uTime: { value: 0 } },
      vertexShader: `attribute float size; attribute float vis; attribute vec3 color; varying vec3 vC; varying float vV; uniform float uScale, uTime;
        void main(){ vec4 mv = modelViewMatrix * vec4(position,1.0); vC = color; vV = vis;
          float tw = 1.0 + 0.06*sin(uTime*3.0 + position.x*40.0);
          gl_PointSize = clamp(size * uScale * tw / -mv.z, 3.0, 220.0); gl_Position = projectionMatrix * mv; }`,
      fragmentShader: `uniform sampler2D uTex; varying vec3 vC; varying float vV;
        void main(){ float a = texture2D(uTex, gl_PointCoord).a; vec2 d = gl_PointCoord - 0.5; float core = exp(-dot(d,d)*180.0);
          gl_FragColor = vec4((vC*a*0.9 + vec3(core)) * vV, 1.0); }`,
      transparent: true, blending: THREE.AdditiveBlending, depthTest: false, depthWrite: false,
    });
    const pts = new THREE.Points(geo, mat); pts.frustumCulled = false;
    const scene = new THREE.Scene(); scene.add(pts);
    // Dim stars hidden behind cloud: march the camera-star segment through the JS distance field.
    function occlude(camPos) {
      const v = geo.attributes.vis;
      for (let i = 0; i < 19; i++) {
        const s = [pos[3 * i], pos[3 * i + 1], pos[3 * i + 2]];
        let hidden = 0;
        for (let k = 1; k < 48; k++) {
          const f = k / 48, p = [P.lerp(camPos.x, s[0], f), P.lerp(camPos.y, s[1], f), P.lerp(camPos.z, s[2], f)];
          if (sdBase(p) < -0.02) { hidden = 1; break; }
        }
        v.array[i] = hidden ? 0.04 : 1;
      }
      v.needsUpdate = true;
    }
    return { scene, mat, occlude, hd: [pos[0], pos[1], pos[2]] };
  }
  // Apparent V magnitude of HD 168076 at distance d (pc), no dust in between:
  // M_V = V - 5 log10(1740/10) - A_V = 8.24 - 11.20 - 3.0 = -5.96 (facts.md A, B), m = M_V + 5 log10(d/10).
  const MV_HD = 8.24 - 5 * Math.log10(D0 / 10) - 3.0;
  const magHD = (dpc) => MV_HD + 5 * Math.log10(dpc / 10);

  // =====================================================================
  // 1. The 3D layout.
  // =====================================================================
  P.demo3d('fig-arrival-3d', (ctx) => {
    const V = P.pillarsVolume();
    V.uniforms.uCluster.value.set(...CLUSTER);
    V.uniforms.uSteps.value = 96;
    const cam = new THREE.PerspectiveCamera(40, 1, 0.02, 200);
    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(0.1, 0.4, 0), dist: 6.4, yaw: 0, pitch: 0.05, auto: 0.035 });
    const S = oStars();
    // Views: Earth looks along -z; the side view has Earth to the right; "from the cluster" looks down the stars' line of sight.
    const views = {
      earth: { yaw: 0, pitch: 0.05, dist: 6.4, ty: 0.4, name: 'from Earth' },
      side: { yaw: -Math.PI / 2, pitch: 0.12, dist: 6.4, ty: 0.4, name: 'from the side (Earth to the right)' },
      cluster: { yaw: Math.atan2(CLUSTER[0] - 0.1, CLUSTER[2]) , pitch: 1.05, dist: 4.2, ty: -0.5, name: 'from the cluster' },
    };
    let viewName = views.earth.name;
    buttons(ctx, [['View from Earth', 'earth'], ['Side', 'side'], ['From the cluster', 'cluster']], (k) => {
      const v = views[k]; viewName = v.name; orb.idle = -4; orb.vy = orb.vp = 0;
      let dy = v.yaw - orb.yaw; dy = Math.atan2(Math.sin(dy), Math.cos(dy));   // shortest way round
      gsap.to(orb, { yaw: orb.yaw + dy, pitch: v.pitch, dist: v.dist, duration: 2.4, ease: 'power2.inOut', overwrite: true });
      gsap.to(orb.target, { y: v.ty, duration: 2.4, ease: 'power2.inOut', overwrite: true });
    });
    const heads = [[[-0.22, 0.5, -0.42], 'P1', '#ffb08a'], [[0.5, 0.03, 0.02], 'P2', '#ffb08a'], [[0.92, -0.67, 0.5], 'P3', '#ffb08a']];
    return {
      scale: 0.7,
      update(t, dt) {
        orb.step(dt); cam.updateMatrixWorld();
        S.mat.uniforms.uTime.value = t; S.mat.uniforms.uScale.value = ctx.h * ctx.dpr * 0.7 * 0.55;
        S.occlude(cam.position);
      },
      renderTo(r, pw, ph) {
        V.renderTo(r, cam, performance.now() / 1000, pw, ph);
        r.autoClear = false; r.render(S.scene, cam); r.autoClear = true;
      },
      overlay(g, w, h) {
        const px = small(w);
        for (const [p, name, col] of heads) { const s = toScreen(p, cam, w, h); if (s.front) label(g, s.x, s.y, name, col, px); }
        const hs = toScreen(S.hd, cam, w, h); if (hs.front) label(g, hs.x, hs.y, 'HD 168076', '#cfe0ff', px);
        const dHD = cam.position.distanceTo(new THREE.Vector3(...S.hd));
        const rows = [
          ['view', viewName],
          ['order from Earth', 'P3 · P2 · stars · P1'],
          ['camera → HD 168076', `${(dHD * PC_LY).toFixed(1)} ly`],
          ['HD 168076 would shine at', `mag ${magHD(dHD).toFixed(1)}`],
        ];
        P.hud(g, 12, h - 12 - rows.length * (px + 6) - 10, rows, { px });   // bottom-left keeps the cluster clear
        g.save(); g.font = `${px}px ${MONO}`; g.fillStyle = 'rgba(220,224,232,0.6)'; g.textAlign = 'right'; g.textBaseline = 'top';
        g.fillText('schematic · drag to orbit', w - 12, 12); g.restore();
      },
    };
  }, { aspect: 0.66 });

  // =====================================================================
  // 2. The approach: surface brightness is conserved; then through a pillar head.
  // =====================================================================
  P.demo3d('fig-arrival-approach', (ctx) => {
    const V = softenVolume(P.pillarsVolume());
    V.uniforms.uCluster.value.set(...CLUSTER);
    V.uniforms.uSteps.value = 110;
    const cam = new THREE.PerspectiveCamera(40, 1, 0.005, 5000);
    const AIM = [0.47, -0.12, 0.0];                 // P2's head
    const MU0 = 20.5;                               // observed H-alpha surface brightness from Earth, R-mag/arcsec^2 (facts.md G: 20-22)
    const A_HA = 0.818 * 3.0;                       // A_Halpha = 0.82 A_V (Cardelli 1989), A_V ~ 3 to the cluster
    const T_APPROACH = 16, T_THROUGH = 14, T_HOLD = 3, LOOP = T_APPROACH + T_THROUGH + T_HOLD;
    let t0 = performance.now(), clock = 0, prevZ = null, column = 0;
    // Camera distance along the line of sight: log approach 1740 pc -> 2.5 pc, then linear through the head to z = -1.4 pc.
    function zAt(s) {
      if (s < T_APPROACH) { const u = s / T_APPROACH, e = 1 - (1 - u) * (1 - u); return D0 * Math.pow(2.5 / D0, e); }   // exponential zoom, easing out
      const u = P.clamp((s - T_APPROACH) / T_THROUGH, 0, 1);
      return P.lerp(2.5, -1.4, u * u * (3 - 2 * u) * 0.7 + u * 0.3);
    }
    const replay = document.createElement('button'); replay.className = 'btn'; replay.type = 'button'; replay.textContent = 'Fly again';
    replay.addEventListener('click', () => { t0 = performance.now(); column = 0; prevZ = null; });
    ctx.controls.appendChild(replay);
    let state = {};
    return {
      scale: 0.7,
      update(t, dt) {
        // wall-clock timeline, so the flight keeps its pace even when frames drop
        clock = (performance.now() - t0) / 1000; if (clock > LOOP) { t0 = performance.now(); clock = 0; column = 0; prevZ = null; }
        const z = zAt(clock);
        const drift = 0.02 * Math.sin(t * 0.4);
        cam.position.set(AIM[0] + drift, AIM[1] + 0.01 * Math.cos(t * 0.3), z);
        cam.lookAt(AIM[0] + drift * 0.5, AIM[1], z - 1);
        cam.updateMatrixWorld();
        // Column of gas passed: N_H += n_H ds (cm^-2); only counts inside the volume (|z| < 3 pc).
        const p = [cam.position.x, cam.position.y, z];
        const gas = Math.abs(z) < 3 ? gasAt(p) : [1, 1, 8000, 'interstellar space'];
        if (prevZ !== null && prevZ > z) column += gas[1] * (prevZ - z) * C.pc;
        prevZ = z;
        // Remaining foreground dust (toy: spread evenly along the path) dims the picture; surface brightness otherwise constant.
        const dz = Math.max(z, 0);
        const mu = MU0 - A_HA * (1 - dz / D0);
        V.uniforms.uExposure.value = 1.6 * Math.pow(10, -0.4 * (mu - (MU0 - A_HA)));
        state = { z, gas, mu };
      },
      renderTo(r, pw, ph) { V.renderTo(r, cam, performance.now() / 1000, pw, ph); },
      overlay(g, w, h) {
        const { z, gas, mu } = state; if (z === undefined) return;
        const px = small(w), d = Math.abs(z - AIM[2]);
        const rows = [];
        if (z > 1.2) {
          // angular size of the tallest pillar (1.3 pc) and total light relative to the view from Earth (inverse square)
          const ang = 2 * Math.atan(0.65 / d) * 180 / Math.PI;
          const angS = ang >= 1 ? `${ang.toFixed(1)}°` : `${(ang * 60).toFixed(1)}′`;
          const flux = Math.pow(D0 / d, 2) * Math.pow(10, 0.4 * A_HA * (1 - d / D0));
          rows.push(['distance', d > 30 ? `${Math.round(d * PC_LY).toLocaleString('en-US')} ly` : `${(d * PC_LY).toFixed(1)} ly`],
            ['tallest pillar spans', angS], ['total light vs Earth', flux < 1e4 ? `×${P.fmt(flux, 2)}` : `×${P.sci(flux, 2)}`],
            ['surface brightness', `${mu.toFixed(1)} mag/arcsec²`, '#ff9d8f']);
          if (d > 25) {   // the pillars are a few pixels: mark them
            const s = toScreen(AIM, cam, w, h);
            g.save(); g.strokeStyle = 'rgba(255,190,170,0.75)'; g.lineWidth = 1;
            const r = 14 + 6 * Math.sin(performance.now() / 400);
            g.beginPath(); g.arc(s.x, s.y, r, 0, 7); g.stroke();
            g.font = `${px}px ${MONO}`; g.fillStyle = 'rgba(255,210,195,0.85)'; g.fillText('the Pillars', s.x + r + 6, s.y + 4); g.restore();
          }
        } else {
          rows.push(['position', `${gas[3]}`], ['gas here', `${P.sci(gas[0], 2)} per cm³`],
            ['vs LHC beam pipe', gas[0] < LHC ? `1/${Math.round(LHC / gas[0]).toLocaleString('en-US')} as dense` : `${P.fmt(gas[0] / LHC, 2)}× as dense`], ['vs sea-level air', `1/${P.sci(AIR / gas[0], 2)}`],
            ['gas passed', `${P.fmt(1.4 * C.mH * column, 2)} g/cm² = ${P.fmt(1.4 * C.mH * column / 1.225e-3 / 100, 2)} m of air`],
            ['dust behind you', `A_V ${(column / 2.2e21).toFixed(1)} mag`]);   // N_H = 2.2e21 A_V (Guver & Ozel 2009)
        }
        const y = P.hud(g, 12, 12, rows, { px });
        // density bar: log10 n from 1 to 20 with the LHC and air marked
        const bx = 12, bw = Math.min(300, w - 24), by = y + 10;
        const X = (n) => bx + (Math.log10(n) - 0) / 20 * bw;
        g.save(); g.fillStyle = 'rgba(6,8,12,0.55)'; g.fillRect(bx, by, bw, 26);
        g.fillStyle = '#5fd0c4'; g.fillRect(bx, by + 8, Math.max(2, X(gas[0]) - bx), 6);
        g.font = `${px - 2}px ${MONO}`; g.fillStyle = 'rgba(220,224,232,0.75)';
        g.fillText('gas density (log)', X(1e10), by + 25);
        for (const [n, t] of [[LHC, 'LHC'], [AIR, 'air']]) { g.fillRect(X(n), by + 4, 1, 14); g.fillText(t, X(n) - 8, by + 25); }
        g.restore();
      },
    };
  }, { aspect: 0.6 });

  // =====================================================================
  // 3. Camera versus dark-adapted eye, applied live to the volume render.
  // =====================================================================
  P.demo3d('fig-arrival-eye', (ctx) => {
    const V = P.pillarsVolume();
    V.uniforms.uCluster.value.set(...CLUSTER);
    V.uniforms.uSteps.value = 96;
    const cam = new THREE.PerspectiveCamera(40, 1, 0.02, 200);
    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(0.1, -0.45, 0), dist: 3.4, yaw: 0.25, pitch: 0.08, auto: 0.05 });
    let rt = null;
    // Post pass. The camera half shows the render as is. The eye half approximates rod vision:
    // scotopic luminance weights (CIE 1951 V'): red H-alpha/[S II] ~5e-4, teal [O III]/H-beta ~0.9-1, so we weight
    // the render's channels as R*0.0005 + G*0.62 + B*0.38; then low acuity (blur), no colour, rod grain.
    // uMeso blends toward full colour as luminance rises through the mesopic range (CIE 191:2010).
    const post = new THREE.ShaderMaterial({
      uniforms: { uTex: { value: null }, uRes: { value: new THREE.Vector2(1, 1) }, uSplit: { value: 0.5 }, uMeso: { value: 0 }, uGain: { value: 0.95 }, uTime: { value: 0 } },
      vertexShader: `varying vec2 vUv; void main(){ vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }`,
      fragmentShader: `precision highp float; varying vec2 vUv; uniform sampler2D uTex; uniform vec2 uRes; uniform float uSplit, uMeso, uGain, uTime;
        float h(vec2 p){ return fract(sin(dot(p, vec2(12.9898,78.233)) + uTime*7.0) * 43758.5453); }
        void main(){
          vec3 cam = texture2D(uTex, vUv).rgb;
          if (vUv.x < uSplit) { gl_FragColor = vec4(cam, 1.0); }
          else {
            vec3 b = vec3(0.0); float wsum = 0.0;
            float rad = mix(4.0, 0.0, uMeso);
            for (int i = -2; i <= 2; i++) for (int j = -2; j <= 2; j++) {
              float w = exp(-float(i*i + j*j) / 3.0); b += texture2D(uTex, vUv + vec2(i, j) * rad / uRes).rgb * w; wsum += w; }
            b /= wsum;
            float ys = dot(b, vec3(0.0005, 0.62, 0.38));
            vec3 rods = vec3(0.80, 0.90, 0.88) * pow(ys, 0.95) * uGain;
            vec3 col = mix(rods, cam * mix(uGain, 1.0, uMeso), uMeso);
            col += (h(vUv * uRes) - 0.5) * 0.06 * (1.0 - uMeso) * smoothstep(0.0, 0.08, ys);
            gl_FragColor = vec4(max(col, 0.0), 1.0);
          }
          float edge = abs(vUv.x - uSplit) * uRes.x;
          gl_FragColor.rgb = mix(gl_FragColor.rgb, vec3(0.95), smoothstep(1.5, 0.0, edge) * 0.8);
        }`,
      depthTest: false, depthWrite: false,
    });
    const pq = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), post); pq.frustumCulled = false;
    const pScene = new THREE.Scene(); pScene.add(pq);
    const pCam = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

    // Brightness steps (photopic luminance of the bright gas): arrival mu ~ 18 -> ~1.5e-3 cd/m^2 (facts_check.py);
    // mesopic weight = (log10 L - log10 0.005) / 3, clamped to [0, 1].
    const levels = {
      arrival: { L: 1.5e-3, gain: 0.95, label: 'on arrival' },
      x100: { L: 0.15, gain: 1.0, label: '100× brighter' },
      x1e4: { L: 15, gain: 1.0, label: '10,000× brighter' },
    };
    const meso = (L) => P.clamp((Math.log10(L) - Math.log10(0.005)) / 3, 0, 1);
    const st = { meso: 0, gain: 0.95, L: 1.5e-3, label: 'on arrival', split: 0.5, hold: 0 };
    buttons(ctx, [['On arrival', 'arrival'], ['100× brighter', 'x100'], ['10,000× brighter', 'x1e4']], (k) => {
      const v = levels[k]; st.label = v.label;
      gsap.to(st, { meso: meso(v.L), gain: v.gain, L: v.L, duration: 1.6, ease: 'power2.inOut', overwrite: true });
    });
    // Dragging near the divider moves it; elsewhere it orbits (P.orbit handles that).
    P.drag(ctx.canvas, (dx, dy, x, y, phase) => { if (phase !== 'end' && Math.abs(x / ctx.w - st.split) < 0.08) { st.split = P.clamp(x / ctx.w, 0.05, 0.95); st.hold = 4; orb.idle = 0; orb.vy = orb.vp = 0; orb.yaw -= -dx * 0.006; orb.pitch -= dy * 0.006; } });

    return {
      scale: 0.7,
      update(t, dt) {
        orb.step(dt);
        st.hold -= dt; if (st.hold <= 0) st.split += (0.5 + 0.18 * Math.sin(t * 0.35) - st.split) * Math.min(1, dt * 2);
        post.uniforms.uSplit.value = st.split; post.uniforms.uMeso.value = st.meso; post.uniforms.uGain.value = st.gain; post.uniforms.uTime.value = t % 10;
      },
      renderTo(r, pw, ph) {
        if (!rt || rt.width !== pw || rt.height !== ph) { rt && rt.dispose(); rt = new THREE.WebGLRenderTarget(pw, ph); }
        r.setRenderTarget(rt); r.setViewport(0, 0, pw, ph); r.setScissor(0, 0, pw, ph);
        V.renderTo(r, cam, performance.now() / 1000, pw, ph);
        r.setRenderTarget(null); r.setViewport(0, 0, pw, ph); r.setScissor(0, 0, pw, ph);
        post.uniforms.uTex.value = rt.texture; post.uniforms.uRes.value.set(pw, ph);
        r.render(pScene, pCam);
      },
      overlay(g, w, h) {
        const px = small(w), m = st.meso;
        const regime = m <= 0.01 ? 'rods only, no colour' : m >= 0.99 ? 'cones, full colour' : 'mesopic, colour returning';
        P.hud(g, 12, 12, [['camera', 'long exposure'], ['line colours', 'Hα red, [O III] teal']], { px });
        P.hud(g, w - 12, 12, [['your eye', st.label], ['luminance', `${P.sci(st.L, 2)} cd/m²`], ['vision', regime], ['Hα share of rod signal', '0.07%']], { px, align: 'right' });
      },
    };
  }, { aspect: 0.6 });
})();
