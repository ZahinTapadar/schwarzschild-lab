(function () {
  'use strict';
  // Hero: the volumetric pillars, slowly orbiting. Slider moves from visible light to 4.4 µm.
  P.demo3d('fig-intro-hero', (ctx) => {
    const V = P.pillarsVolume();
    const cam = new THREE.PerspectiveCamera(38, 1, 0.05, 50);
    const orb = P.orbit(ctx, cam, { target: new THREE.Vector3(0.05, -0.75, 0), dist: 4.3, yaw: 0, pitch: 0.02, auto: 0.05 });
    const lam = ctx.slider({ id: 'hero-lam', label: 'Wavelength', min: 0, max: 1, step: 0.001, value: 0,
      format: (u) => `${(0.55 * Math.pow(8, u)).toFixed(2)} µm` });
    let ir = 0;
    return {
      scale: 0.8,
      update(t, dt) { orb.step(dt); ir += (lam.value - ir) * Math.min(1, dt * 6); V.uniforms.uIR.value = ir; V.uniforms.uKr.value = Math.min(1, Math.pow(0.55 * Math.pow(8, ir) / 0.55, -1.6)); },
      renderTo(r, pw, ph) { V.renderTo(r, cam, performance.now() / 1000, pw, ph); },
      overlay(g, w, h) {
        const um = 0.55 * Math.pow(8, ir);
        P.hud(g, 12, 12, [['λ', `${um.toFixed(2)} µm`], ['dust A_λ/A_V', (Math.pow(um / 0.55, -1.6) > 1 ? 1 : Math.pow(um / 0.55, -1.6)).toFixed(2)], ['drag', 'to orbit']], { px: w < 500 ? 10 : 12 });
      },
    };
  }, { aspect: 0.62 });
})();
