// Ray-marched volumetric model of the three Pillars, shared by several scenes.
// Units: 1 scene unit = 1 parsec. Geometry is schematic but follows the measured layout:
// tallest pillar (P1) ~1.3 pc on the left and farthest from us, P2 in the middle, short P3 in front,
// tips leaning toward the NGC 6611 cluster ~2 pc away (McLeod et al. 2015; Karim et al. 2023).
// Physics in the shader:
//   dust extinction: dtau = kappa(lambda) * rho * ds, with kappa(2 µm)/kappa(V) ≈ 0.11 (Cardelli 1989)
//   ionised skin: emission where the cloud surface sees the cluster (one shadow probe toward it)
//   background stars are attenuated by exp(-tau) along the full ray, so infrared reveals them.
// API: const V = P.pillarsVolume(); V.uniforms.uIR.value = 0..1; V.renderTo(renderer, perspectiveCamera, t)
(function () {
  'use strict';
  const vert = `varying vec2 vUv; void main(){ vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }`;
  const frag = `
precision highp float;
varying vec2 vUv;
uniform mat4 uInvProj, uCamWorld;
uniform vec3 uCam, uCluster;
uniform float uKr, uTime, uIR, uExposure, uDust, uSteps, uStars, uSkin, uAmbient;
uniform vec2 uRes;

float hash1(vec3 p){ p = fract(p*0.3183099 + 0.1); p *= 17.0; return fract(p.x*p.y*p.z*(p.x+p.y+p.z)); }
float noise(vec3 x){ vec3 i = floor(x), f = fract(x); f = f*f*(3.0-2.0*f);
  return mix(mix(mix(hash1(i),hash1(i+vec3(1,0,0)),f.x), mix(hash1(i+vec3(0,1,0)),hash1(i+vec3(1,1,0)),f.x),f.y),
             mix(mix(hash1(i+vec3(0,0,1)),hash1(i+vec3(1,0,1)),f.x), mix(hash1(i+vec3(0,1,1)),hash1(i+vec3(1,1,1)),f.x),f.y), f.z); }
float fbm(vec3 p){ float a = 0.5, s = 0.0; for(int i=0;i<5;i++){ s += a*noise(p); p = p*2.03 + vec3(1.7,9.2,3.1); a *= 0.5; } return s; }
float fbm3(vec3 p){ float a = 0.5, s = 0.0; for(int i=0;i<3;i++){ s += a*noise(p); p = p*2.03 + vec3(1.7,9.2,3.1); a *= 0.5; } return s; }

float capsule(vec3 p, vec3 a, vec3 b, float ra, float rb){
  vec3 pa = p-a, ba = b-a; float h = clamp(dot(pa,ba)/dot(ba,ba), 0.0, 1.0);
  return length(pa - ba*h) - mix(ra, rb, h);
}
float smin(float a, float b, float k){ float h = clamp(0.5+0.5*(b-a)/k, 0.0, 1.0); return mix(b, a, h) - k*h*(1.0-h); }

// signed distance (pc) to the pillar cloud, before noise
float sdBase(vec3 p){
  float d1 = capsule(p, vec3(-0.75,-1.45,-0.55), vec3(-0.35, 0.05,-0.45), 0.36, 0.17);   // P1 lower
  d1 = smin(d1, capsule(p, vec3(-0.35, 0.05,-0.45), vec3(-0.22, 0.42,-0.42), 0.17, 0.2), 0.12); // P1 head
  float d2 = capsule(p, vec3( 0.12,-1.45,-0.05), vec3( 0.42,-0.25, 0.0), 0.3, 0.13);
  d2 = smin(d2, capsule(p, vec3(0.42,-0.25,0.0), vec3(0.5,-0.05,0.02), 0.13, 0.12), 0.08);
  float d3 = capsule(p, vec3( 0.75,-1.45, 0.45), vec3( 0.92,-0.75, 0.5), 0.24, 0.1);
  float base = length((p - vec3(0.1,-2.9,0.0)) / vec3(2.6, 1.45, 1.3)) - 1.0;
  float d = smin(smin(d1, d2, 0.18), d3, 0.18);
  return smin(d, base*0.9, 0.3);
}
float sdCloud(vec3 p){
  // striations stretched along the pillars (they are drawn out toward the cluster)
  float n = fbm3(p*vec3(3.2, 1.1, 3.2));
  return sdBase(p) + (n - 0.5)*0.24 + (noise(p*vec3(11.0,4.0,11.0))-0.5)*0.05;
}

// stars on the celestial sphere behind the cloud
vec3 starfield(vec3 dir){
  vec3 col = vec3(0.0);
  for (int k = 0; k < 2; k++){
    float sc = k == 0 ? 90.0 : 260.0;
    vec3 q = dir*sc; vec3 id = floor(q); vec3 f = fract(q) - 0.5;
    float h = hash1(id + float(k)*13.1);
    float thr = k == 0 ? 0.982 : mix(0.965, 0.93, uIR);
    if (h > thr) {
      vec3 off = vec3(hash1(id+1.3), hash1(id+2.7), hash1(id+5.1)) - 0.5;
      float r = length(f - off*0.6);
      float b = pow((h - thr)/(1.0-thr), 2.0) * (k == 0 ? 9.0 : 2.4);
      float t = hash1(id+9.9);
      vec3 c = mix(vec3(1.0,0.72,0.45), vec3(0.65,0.8,1.0), t);
      col += c * b * (exp(-r*r*(k==0 ? 90.0 : 260.0)) + 0.15*exp(-r*r*(k==0 ? 12.0 : 40.0)));
    }
  }
  return col;
}

void main(){
  vec2 ndc = vUv*2.0 - 1.0;
  vec4 vp = uInvProj * vec4(ndc, 1.0, 1.0); vp /= vp.w;
  vec3 rd = normalize((uCamWorld * vec4(vp.xyz, 0.0)).xyz);
  vec3 ro = uCam;

  // background: diffuse ionised nebula glow (teal [O III] near the cluster, red H-alpha farther out)
  vec3 sky = vec3(0.0);
  float glow = fbm(rd*3.0 + vec3(0.0, uTime*0.004, 0.0));
  vec3 toC = normalize(uCluster);
  float nearC = pow(max(dot(rd, toC), 0.0), 3.0);
  float lowH = smoothstep(0.3, -0.6, rd.y + 0.35*rd.x);
  vec3 neb = mix(vec3(0.16,0.46,0.58), vec3(0.62,0.42,0.30), 0.55*lowH);
  neb = mix(neb, vec3(0.30,0.70,0.72), nearC);
  sky += neb * uAmbient * (0.55 + 1.1*glow*glow) * mix(1.0, 0.25, uIR);
  sky += vec3(0.10,0.18,0.45) * uIR * (0.5 + glow);

  // volume bounds
  vec3 bmin = vec3(-2.6,-3.4,-1.6), bmax = vec3(2.6, 1.1, 1.6);
  vec3 t0 = (bmin - ro)/rd, t1 = (bmax - ro)/rd;
  vec3 tmin = min(t0,t1), tmax = max(t0,t1);
  float tn = max(max(tmin.x,tmin.y), max(tmin.z, 0.0)), tf = min(min(tmax.x,tmax.y), tmax.z);

  vec3 L = vec3(0.0); float T = 1.0;
  // dust opacity per unit density per pc: visible -> near-IR (A_2µm/A_V ≈ 0.11)
  vec3 kV = vec3(1.25, 1.0, 0.82) * uDust;           // redder light gets through more easily
  // uKr = A_lambda/A_V from the page (Cardelli-like power law); colour tint only in the visible
  vec3 kappa = mix(kV, kV.yyy, uIR) * uKr;
  vec3 skinCol = mix(vec3(1.0,0.72,0.48), vec3(1.0,0.6,0.32), uIR);
  vec3 dustCol = mix(vec3(0.45,0.20,0.09), vec3(1.0,0.55,0.22), uIR);  // IR: warm scattered/emitted glow

  if (tf > tn) {
    float steps = uSteps;
    float ds = (tf - tn)/steps;
    float jit = hash1(vec3(gl_FragCoord.xy, uTime));
    float t = tn + ds*jit;
    for (int i = 0; i < 160; i++){
      if (float(i) >= steps || T < 0.01) break;
      vec3 p = ro + rd*t;
      float d = sdCloud(p);
      if (d < 0.25) {
        // density: smooth inside the surface, clumpier toward the heads
        float inside = smoothstep(0.005, -0.035, d);
        float clump = 0.6 + 0.8*fbm3(p*7.0);
        float rho = inside * clump * (1.0 + 1.4*smoothstep(-0.6, 0.3, p.y));
        // lit by the cluster? probe a short way toward it
        vec3 Lc = normalize(uCluster - p);
        float lit = smoothstep(0.0, 0.1, min(sdCloud(p + Lc*0.12), sdCloud(p + Lc*0.45)));
        float shell = exp(-abs(d + 0.008)/0.018) * lit * mix(0.22, 1.0, smoothstep(-0.7, 0.35, p.y));
        // ionised skin emission (H-alpha + [S II] at the surface, fading inward)
        vec3 em = skinCol * shell * uSkin * 2.4;
        // photoevaporation flow: faint glow just outside lit surfaces
        // only on the cluster side: there must be cloud just behind this point, away from the stars
        float sun = 1.0 - smoothstep(-0.02, 0.06, sdCloud(p - Lc*0.16));
        em += vec3(0.45,0.8,0.82) * exp(-max(d,0.0)/0.07) * step(0.0, d) * lit * sun * 0.6 * uSkin * (1.0 - uIR*0.7);
        // dust: absorbs; in IR it also glows warm where lit
        em += dustCol * rho * (0.03 + 0.08*lit + 0.35*lit*uIR + 0.05*uIR);
        vec3 dtau = kappa * rho * ds * 70.0;
        vec3 Ts = exp(-dtau);
        L += T * em * ds * 7.0;
        T *= (Ts.x + Ts.y + Ts.z)/3.0;
        // colour-dependent transmission folded into the background later
        sky *= Ts;
      }
      t += ds;
    }
  }
  vec3 stars = starfield(rd) * uStars * mix(0.7, 2.6, uIR);
  vec3 col = L + T*sky + stars*T;
  col *= uExposure;
  col = 1.0 - exp(-col);                 // filmic-ish tone map
  col = pow(col, vec3(0.92));
  gl_FragColor = vec4(col, 1.0);
}`;

  P.pillarsVolume = function () {
    const uniforms = {
      uInvProj: { value: new THREE.Matrix4() }, uCamWorld: { value: new THREE.Matrix4() },
      uCam: { value: new THREE.Vector3() }, uCluster: { value: new THREE.Vector3(1.4, 2.4, 0.9) },
      uTime: { value: 0 }, uIR: { value: 0 }, uKr: { value: 1 }, uExposure: { value: 1.4 }, uDust: { value: 1 },
      uSteps: { value: 120 }, uStars: { value: 1 }, uSkin: { value: 1 }, uAmbient: { value: 0.55 },
      uRes: { value: new THREE.Vector2(1, 1) },
    };
    const mat = new THREE.ShaderMaterial({ vertexShader: vert, fragmentShader: frag, uniforms, depthTest: false, depthWrite: false });
    const quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), mat);
    quad.frustumCulled = false;
    const scene = new THREE.Scene(); scene.add(quad);
    const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    return {
      uniforms, scene, camera: ortho,
      // Copy a perspective camera's matrices into the ray generator, then render.
      renderTo(r, cam, t, pw, ph) {
        cam.updateMatrixWorld(); cam.updateProjectionMatrix();
        uniforms.uInvProj.value.copy(cam.projectionMatrixInverse);
        uniforms.uCamWorld.value.copy(cam.matrixWorld);
        uniforms.uCam.value.setFromMatrixPosition(cam.matrixWorld);
        uniforms.uTime.value = t;
        if (pw) uniforms.uRes.value.set(pw, ph);
        r.render(scene, ortho);
      },
    };
  };
})();
