// One shared THREE.WebGLRenderer for every 3D scene on the page.
// Browsers allow only ~16 live WebGL contexts, so each scene renders into a corner of the shared
// canvas and is copied onto its own figure's 2D canvas (the same approach ciechanow.ski uses).
// Usage:
//   P.demo3d('fig-x', (ctx, THREE) => {
//     const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(40, 1, 0.01, 100);
//     return { scene, camera, update(t, dt, pw, ph) {...}, overlay(g, w, h, t) {...}, scale: 1 };
//   }, { aspect: 0.56 });
// update() runs before each render; overlay() draws crisp 2D labels/HUD on top afterwards.
// Instead of scene/camera a scene may supply renderTo(renderer, pw, ph) for multi-pass work.
// P.orbit(ctx, camera, {target, dist, yaw, pitch, auto}) adds drag-to-rotate with inertia and slow auto-spin.
(function () {
  'use strict';
  const MAXW = 2048, MAXH = 1280;
  let R = null;
  P.renderer = function () {
    if (R) return R;
    R = new THREE.WebGLRenderer({ antialias: true, alpha: false, powerPreference: 'high-performance' });
    R.setPixelRatio(1);
    R.setSize(MAXW, MAXH, false);
    R.outputEncoding = THREE.sRGBEncoding;
    return R;
  };

  // Only the most visible 3D scene renders each frame (others keep their last image),
  // and each scene scales its resolution to hold ~50 fps.
  const vis = new Map();
  function mostVisible(id, fig) {
    const b = fig.getBoundingClientRect();
    const frac = Math.max(0, Math.min(b.bottom, innerHeight) - Math.max(b.top, 0)) / Math.max(1, b.height);
    const now = performance.now();
    vis.set(id, { frac, now });
    for (const [k, v] of vis) if (k !== id && now - v.now < 150 && v.frac > frac + 0.02) return false;
    return true;
  }

  P.demo3d = function (id, setup, opts = {}) {
    P.demo(id, (ctx) => {
      const r = P.renderer();
      const inst = setup(ctx, THREE, r) || {};
      let dyn = 1, ema = 1 / 60, drawn = false;
      return {
        animate: inst.animate !== false,
        resize: () => { drawn = false; inst.resize && inst.resize(); },
        draw(t, dt) {
          if (drawn && !mostVisible(id, ctx.fig)) return;
          if (dt > 0) {
            ema += (dt - ema) * 0.1;
            if (ema > 1 / 45) dyn = Math.max(0.45, dyn * 0.97);
            else if (ema < 1 / 57) dyn = Math.min(1, dyn * 1.01);
          }
          drawn = true;
          const scale = (inst.scale || opts.scale || 1) * dyn;
          const pw = Math.max(2, Math.min(MAXW, Math.round(ctx.w * ctx.dpr * scale)));
          const ph = Math.max(2, Math.min(MAXH, Math.round(ctx.h * ctx.dpr * scale)));
          if (inst.camera && inst.camera.isPerspectiveCamera) { inst.camera.aspect = ctx.w / ctx.h; inst.camera.updateProjectionMatrix(); }
          inst.update && inst.update(t, dt, pw, ph);
          r.setRenderTarget(null);
          r.setViewport(0, 0, pw, ph); r.setScissor(0, 0, pw, ph); r.setScissorTest(true);
          if (inst.renderTo) inst.renderTo(r, pw, ph); else r.render(inst.scene, inst.camera);
          r.setRenderTarget(null);
          r.setScissorTest(false);
          const g = ctx.g;
          g.save(); g.setTransform(1, 0, 0, 1, 0, 0);
          g.imageSmoothingEnabled = true; g.imageSmoothingQuality = 'high';
          // WebGL's viewport origin is bottom-left; the canvas element's is top-left.
          g.drawImage(r.domElement, 0, MAXH - ph, pw, ph, 0, 0, ctx.canvas.width, ctx.canvas.height);
          g.restore();
          inst.overlay && inst.overlay(g, ctx.w, ctx.h, t);
        },
      };
    }, opts);
  };

  // Orbit camera around a target. Drag rotates (with inertia); auto-spin resumes after 3 s idle.
  P.orbit = function (ctx, camera, o = {}) {
    const s = { yaw: o.yaw ?? 0, pitch: o.pitch ?? 0.15, dist: o.dist ?? 5, target: o.target || new THREE.Vector3(), vy: 0, vp: 0, idle: 99, auto: o.auto ?? 0.06, dragging: false };
    P.drag(ctx.canvas, (dx, dy, x, y, phase) => {
      if (phase === 'start') { s.vy = s.vp = 0; s.dragging = true; }
      if (phase === 'end') { s.dragging = false; return; }
      s.vy = -dx * 0.006; s.vp = dy * 0.006; s.idle = 0;
      s.yaw += s.vy; s.pitch = P.clamp(s.pitch + s.vp, -1.3, 1.3);
    });
    s.step = (dt) => {
      s.idle += dt;
      if (!s.dragging) { s.yaw += s.vy; s.pitch = P.clamp(s.pitch + s.vp, -1.3, 1.3); s.vy *= 0.92; s.vp *= 0.92; }
      if (s.idle > 3) s.yaw += s.auto * dt;
      const cp = Math.cos(s.pitch);
      camera.position.set(s.target.x + s.dist * cp * Math.sin(s.yaw), s.target.y + s.dist * Math.sin(s.pitch), s.target.z + s.dist * cp * Math.cos(s.yaw));
      camera.lookAt(s.target);
    };
    return s;
  };

  // Soft round sprite for additive particles (THREE.Points with PointsMaterial map / custom shader).
  let dot = null;
  P.dotTexture = function () {
    if (dot) return dot;
    const c = document.createElement('canvas'); c.width = c.height = 64;
    const g = c.getContext('2d'); const gr = g.createRadialGradient(32, 32, 0, 32, 32, 32);
    gr.addColorStop(0, 'rgba(255,255,255,1)'); gr.addColorStop(0.2, 'rgba(255,255,255,0.55)'); gr.addColorStop(1, 'rgba(255,255,255,0)');
    g.fillStyle = gr; g.fillRect(0, 0, 64, 64);
    dot = new THREE.CanvasTexture(c); return dot;
  };

  // HUD helper shared by 3D overlays: monospace label/value rows in a translucent panel.
  P.hud = function (g, x, y, rows, o = {}) {
    const px = o.px || 12;
    g.save();
    g.font = `${px}px "JetBrains Mono", ui-monospace, monospace`;
    const lw = Math.max(...rows.map(r => g.measureText(r[0]).width));
    const vw = Math.max(...rows.map(r => g.measureText(r[1]).width));
    const W = lw + vw + 26, H = rows.length * (px + 6) + 10;
    const ax = o.align === 'right' ? x - W : x;
    g.fillStyle = 'rgba(6,8,12,0.55)'; g.beginPath(); g.roundRect ? g.roundRect(ax, y, W, H, 6) : g.rect(ax, y, W, H); g.fill();
    rows.forEach((r, i) => {
      const yy = y + 8 + px + i * (px + 6) - 3;
      g.fillStyle = 'rgba(220,224,232,0.7)'; g.fillText(r[0], ax + 9, yy);
      g.fillStyle = r[2] || '#f2efe8'; g.fillText(r[1], ax + 17 + lw, yy);
    });
    g.restore();
    return y + H;
  };
})();
