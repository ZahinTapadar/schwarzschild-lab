// GLSL ES 3.0 shaders: Schwarzschild null-geodesic ray tracer + HDR post.
// Geometric units (G = c = 1); every length is in the same unit as uM.

export const FULLSCREEN_VS = `#version 300 es
in vec2 aPos;
out vec2 vUv;
void main() { vUv = aPos * 0.5 + 0.5; gl_Position = vec4(aPos, 0.0, 1.0); }`;

const NOISE = `
float hash13(vec3 p) {
  p = fract(p * 0.1031);
  p += dot(p, p.zyx + 31.32);
  return fract((p.x + p.y) * p.z);
}
vec3 hash33(vec3 p) {
  p = fract(p * vec3(0.1031, 0.1030, 0.0973));
  p += dot(p, p.yxz + 33.33);
  return fract((p.xxy + p.yxx) * p.zyx);
}
float vnoise(vec3 p) {
  vec3 i = floor(p), f = fract(p);
  vec3 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(hash13(i), hash13(i + vec3(1,0,0)), u.x),
                 mix(hash13(i + vec3(0,1,0)), hash13(i + vec3(1,1,0)), u.x), u.y),
             mix(mix(hash13(i + vec3(0,0,1)), hash13(i + vec3(1,0,1)), u.x),
                 mix(hash13(i + vec3(0,1,1)), hash13(i + vec3(1,1,1)), u.x), u.y), u.z);
}
float fbm(vec3 p, int oct) {
  float s = 0.0, a = 0.5;
  for (int i = 0; i < 6; i++) {
    if (i >= oct) break;
    s += a * vnoise(p);
    p = p * 2.03 + vec3(1.7, 9.2, 3.1);
    a *= 0.5;
  }
  return s;
}`;

export const RAYTRACE_FS = `#version 300 es
precision highp float;
precision highp int;
in vec2 vUv;
out vec4 fragColor;

uniform vec2  uRes;
uniform vec3  uCamPos;
uniform mat3  uCamRot;
uniform float uTanFov;
uniform float uM;            // black-hole mass
uniform float uTime;         // distant-observer time, units of M
uniform int   uSteps;
uniform float uStepScale;

uniform float uDiskIn, uDiskOut;  // in units of M
uniform float uDiskTemp;          // peak local temperature (K)
uniform float uDiskBright;
uniform float uDiskOpacity;
uniform float uDoppler;           // 0 = Interstellar look, 1 = physical
uniform float uBeaming;           // exponent on g (4 = bolometric)
uniform float uDiskSpeed;
uniform float uShowDisk;
uniform float uCamLapse;          // sqrt(1 - 2M/r_cam)
uniform float uStars, uNebula;

uniform sampler2D uBlackbody;

uniform int  uNB;
uniform vec4 uBody[16];     // xyz, radius
uniform vec4 uBodyCol[16];  // rgb, emissive
uniform vec4 uBodyAux[16];  // clock rate dtau/dt, id, -, -

${NOISE}

vec3 blackbody(float T) {
  float u = clamp(log(T / 800.0) / log(40000.0 / 800.0), 0.0, 1.0);
  return texture(uBlackbody, vec2(u, 0.5)).rgb;
}

// ---------------------------------------------------------------- sky ---
vec3 starLayer(vec3 d, float scale, float density, float fw) {
  vec3 p = d * scale;
  vec3 c = floor(p);
  float h = hash13(c);
  if (h > density) return vec3(0.0);
  vec3 sp = c + 0.25 + 0.5 * hash33(c + 7.0);
  float dist = length(p - sp);
  float w = max(0.045, fw * scale * 0.9);
  float mag = pow(hash13(c + 3.1), 6.0) * 3.0 + 0.08;
  float T = mix(2800.0, 14000.0, pow(hash13(c + 11.7), 1.6));
  float g = exp(-dist * dist / (w * w)) * (0.045 * 0.045) / (w * w);
  return blackbody(T) * mag * g;
}

vec3 sky(vec3 d, float fw) {
  vec3 col = vec3(0.0);
  // Galactic band (tilted great circle) with dust lanes
  vec3 n = normalize(vec3(0.18, 1.0, -0.42));
  float b = dot(d, n);
  float band = exp(-b * b * 9.0);
  float core = exp(-b * b * 40.0);
  float neb = fbm(d * 3.2 + 4.0, 5);
  float wisps = fbm(d * 9.0 - 2.0, 4);
  float dust = smoothstep(0.42, 0.75, fbm(d * 5.5 + 13.0, 5));
  vec3 gas = vec3(0.62, 0.58, 0.56) * band * (0.35 + neb * 0.9) + vec3(0.75, 0.62, 0.5) * core * wisps;
  gas *= (1.0 - 0.85 * dust * band);
  col += gas * 0.045 * uNebula;
  // faint diffuse glow everywhere
  col += vec3(0.003, 0.0033, 0.004) * fbm(d * 2.0, 3) * uNebula;
  // stars: denser in the band
  float sd = 0.012 + 0.04 * band;
  col += uStars * (starLayer(d, 90.0, 0.03, fw) * 1.6
                 + starLayer(d, 220.0, sd, fw)
                 + starLayer(d, 520.0, sd * 1.4, fw) * 0.6);
  return col;
}

// ---------------------------------------------------------- accretion ---
// Thin Keplerian disk, Novikov-Thorne temperature profile, relativistic
// Doppler + gravitational shift computed from the photon's conserved L_z.
vec4 diskSample(vec3 p, vec3 v) {
  float r = length(p.xz);
  float x = r / uM;
  if (x < uDiskIn || x > uDiskOut) return vec4(0.0);

  // Novikov-Thorne flux F ~ (1 - sqrt(rin/r)) / r^3, peak at r = 49/36 rin
  float xp = uDiskIn * 49.0 / 36.0;
  float Fmax = (1.0 - sqrt(uDiskIn / xp)) / (xp * xp * xp);
  float F = (1.0 - sqrt(uDiskIn / x)) / (x * x * x) / Fmax;
  float Tloc = uDiskTemp * pow(max(F, 1e-4), 0.25);

  // Keplerian angular velocity, prograde about +y
  float omega = sqrt(uM / (r * r * r));
  // photon momentum toward the camera (E normalised to 1 at infinity)
  vec3 n = -v / uCamLapse;
  float Lz = p.z * n.x - p.x * n.z;
  float dop = 1.0 / max(1.0 - omega * Lz, 0.05);
  float grav = sqrt(max(1.0 - 3.0 * uM / r, 1e-4)) / uCamLapse;
  float g = grav * mix(1.0, dop, uDoppler);

  // turbulent, differentially rotating texture
  float phi = atan(p.z, p.x);
  float ang = phi + mod(omega * uTime * uDiskSpeed, 6.2831853);
  vec3 q = vec3(cos(ang) * 2.2, sin(ang) * 2.2, x * 0.9);
  float turb = fbm(q * vec3(1.0, 1.0, 2.6), 5);
  float fine = fbm(vec3(cos(ang) * 6.0, sin(ang) * 6.0, x * 7.0), 4);
  float rings = 0.8 + 0.2 * sin(x * 7.0 + turb * 9.0);
  float dens = clamp(0.25 + 1.1 * turb * rings + 0.35 * fine - 0.2, 0.0, 1.4);

  float edge = smoothstep(uDiskIn, uDiskIn * 1.06, x) * (1.0 - smoothstep(uDiskOut * 0.55, uDiskOut, x));
  float alpha = clamp(dens * edge * uDiskOpacity, 0.0, 1.0);

  float Tobs = Tloc * g;
  float I = uDiskBright * pow(g, uBeaming) * F * (0.5 + 0.8 * dens);
  vec3 c = blackbody(Tobs) * I * edge;
  return vec4(c * alpha, alpha);
}

vec3 accel(vec3 x, float h2) {
  float r2 = dot(x, x);
  float r = sqrt(r2);
  return -3.0 * uM * h2 * x / (r2 * r2 * r);
}

void main() {
  vec2 uv = (gl_FragCoord.xy / uRes) * 2.0 - 1.0;
  float aspect = uRes.x / uRes.y;
  vec3 dir = normalize(uCamRot * vec3(uv.x * uTanFov * aspect, uv.y * uTanFov, 1.0));

  vec3 x = uCamPos;
  vec3 v = dir;
  vec3 hv = cross(x, v);
  float h2 = dot(hv, hv);

  vec3 col = vec3(0.0);
  float alpha = 0.0;
  bool escaped = false;
  float rh = 2.0 * uM;
  float rEsc = max(90.0 * uM, length(uCamPos) * 1.25);

  for (int i = 0; i < 2000; i++) {
    if (i >= uSteps || alpha > 0.995) break;
    float r = length(x);
    if (r < rh * 1.0002) break;
    if (r > rEsc && dot(x, v) > 0.0) { escaped = true; break; }

    // adaptive affine step: coarse far away, fine near the photon sphere
    float dl = uStepScale * 0.07 * r * clamp((r - rh) / (1.5 * uM), 0.08, 1.0);

    // RK4
    vec3 a1 = accel(x, h2);
    vec3 x2 = x + 0.5 * dl * v,  v2 = v + 0.5 * dl * a1;
    vec3 a2 = accel(x2, h2);
    vec3 x3 = x + 0.5 * dl * v2, v3 = v + 0.5 * dl * a2;
    vec3 a3 = accel(x3, h2);
    vec3 x4 = x + dl * v3,       v4 = v + dl * a3;
    vec3 a4 = accel(x4, h2);
    vec3 xn = x + dl / 6.0 * (v + 2.0 * v2 + 2.0 * v3 + v4);
    vec3 vn = v + dl / 6.0 * (a1 + 2.0 * a2 + 2.0 * a3 + a4);

    // body intersection along the segment
    vec3 seg = xn - x;
    float segL = length(seg);
    vec3 sd = seg / segL;
    float tBody = 2.0; int hit = -1;
    for (int b = 0; b < 16; b++) {
      if (b >= uNB) break;
      vec3 oc = x - uBody[b].xyz;
      float bq = dot(oc, sd);
      float cq = dot(oc, oc) - uBody[b].w * uBody[b].w;
      float disc = bq * bq - cq;
      if (disc < 0.0) continue;
      float t = (-bq - sqrt(disc)) / segL;
      if (t >= 0.0 && t <= 1.0 && t < tBody) { tBody = t; hit = b; }
    }

    // disk plane crossing
    float tDisk = 2.0;
    if (uShowDisk > 0.5 && x.y * xn.y < 0.0) tDisk = x.y / (x.y - xn.y);

    if (tDisk < tBody) {
      vec3 p = mix(x, xn, tDisk);
      vec4 d = diskSample(p, normalize(mix(v, vn, tDisk)) * length(v));
      col += (1.0 - alpha) * d.rgb;
      alpha += (1.0 - alpha) * d.a;
    }
    if (hit >= 0) {
      vec3 p = x + seg * tBody;
      vec3 c = uBody[hit].xyz;
      vec3 nrm = normalize(p - c);
      vec3 L = normalize(-c);
      float diff = max(dot(nrm, L), 0.0);
      float tex = 0.75 + 0.25 * fbm(nrm * 3.0 + uBodyAux[hit].y, 4);
      float bands = 0.85 + 0.15 * sin(nrm.y * 14.0 + tex * 6.0);
      vec3 base = uBodyCol[hit].rgb * tex * bands;
      float rate = uBodyAux[hit].x;                  // gravitational dimming
      vec3 lit = base * (diff * 1.4 + 0.05) * 40.0 / max(dot(c, c), 16.0);
      vec3 emi = base * uBodyCol[hit].a * (0.6 + 0.4 * pow(1.0 - abs(dot(nrm, -sd)), 0.5) );
      vec3 bc = (lit + emi * 3.0) * pow(rate, 3.0);
      col += (1.0 - alpha) * bc;
      alpha = 1.0;
      break;
    }
    x = xn; v = vn;
  }

  vec3 bg = escaped ? sky(normalize(v), 2.0 * uTanFov / uRes.y) : vec3(0.0);
  col += (1.0 - alpha) * bg;
  fragColor = vec4(col, 1.0);
}`;

// ------------------------------------------------------------- post FX ---
export const DOWNSAMPLE_FS = `#version 300 es
precision highp float;
in vec2 vUv; out vec4 o;
uniform sampler2D uTex; uniform vec2 uTexel; uniform float uThreshold; uniform int uFirst;
void main() {
  vec2 t = uTexel;
  vec3 a = texture(uTex, vUv + t * vec2(-2, 2)).rgb, b = texture(uTex, vUv + t * vec2(0, 2)).rgb, c = texture(uTex, vUv + t * vec2(2, 2)).rgb;
  vec3 d = texture(uTex, vUv + t * vec2(-2, 0)).rgb, e = texture(uTex, vUv).rgb, f = texture(uTex, vUv + t * vec2(2, 0)).rgb;
  vec3 g = texture(uTex, vUv + t * vec2(-2, -2)).rgb, h = texture(uTex, vUv + t * vec2(0, -2)).rgb, i = texture(uTex, vUv + t * vec2(2, -2)).rgb;
  vec3 j = texture(uTex, vUv + t * vec2(-1, 1)).rgb, k = texture(uTex, vUv + t * vec2(1, 1)).rgb;
  vec3 l = texture(uTex, vUv + t * vec2(-1, -1)).rgb, m = texture(uTex, vUv + t * vec2(1, -1)).rgb;
  vec3 s = e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
  if (uFirst == 1) {
    float br = max(s.r, max(s.g, s.b));
    float soft = clamp(br - uThreshold + 0.5, 0.0, 1.0);
    soft = soft * soft * 0.5;
    float w = max(soft, br - uThreshold) / max(br, 1e-4);
    s *= w;
  }
  o = vec4(min(s, vec3(500.0)), 1.0);
}`;

export const UPSAMPLE_FS = `#version 300 es
precision highp float;
in vec2 vUv; out vec4 o;
uniform sampler2D uTex; uniform vec2 uTexel; uniform float uRadius;
void main() {
  vec2 t = uTexel * uRadius;
  vec3 s = texture(uTex, vUv).rgb * 4.0;
  s += (texture(uTex, vUv + vec2(t.x, 0)).rgb + texture(uTex, vUv - vec2(t.x, 0)).rgb
      + texture(uTex, vUv + vec2(0, t.y)).rgb + texture(uTex, vUv - vec2(0, t.y)).rgb) * 2.0;
  s += texture(uTex, vUv + t).rgb + texture(uTex, vUv - t).rgb
     + texture(uTex, vUv + vec2(t.x, -t.y)).rgb + texture(uTex, vUv + vec2(-t.x, t.y)).rgb;
  o = vec4(s / 16.0, 1.0);
}`;

export const COMPOSITE_FS = `#version 300 es
precision highp float;
in vec2 vUv; out vec4 o;
uniform sampler2D uHdr, uBloom;
uniform float uExposure, uBloomStrength, uTime, uDim;
vec3 aces(vec3 x) {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}
float h12(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * .1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
void main() {
  vec3 hdr = texture(uHdr, vUv).rgb;
  vec3 bloom = texture(uBloom, vUv).rgb;
  vec3 c = (hdr + bloom * uBloomStrength) * uExposure * uDim;
  c = aces(c);
  vec2 q = vUv - 0.5;
  c *= 1.0 - dot(q, q) * 0.55;
  c = pow(c, vec3(1.0 / 2.2));
  c += (h12(gl_FragCoord.xy + fract(uTime) * 917.0) - 0.5) / 255.0 * 1.5;
  o = vec4(c, 1.0);
}`;

// Debris particles, drawn additively into the HDR buffer.
export const POINTS_VS = `#version 300 es
in vec3 aPos; in float aRadius; in float aKind; in float aRate; in float aColor;
uniform mat4 uViewProj; uniform vec3 uCam; uniform float uM; uniform float uPx; uniform float uShadowSin;
out vec3 vCol; out float vA;
void main() {
  vec3 d = aPos - uCam;
  float dist = length(d);
  // cull debris hidden behind the shadow
  vec3 toBH = -uCam; float camR = length(uCam);
  float s = length(cross(normalize(d), toBH / camR));
  bool hidden = dot(d, toBH) > 0.0 && dist > camR && s < uShadowSin;
  if (aKind < 0.5 || hidden || length(aPos) < 2.0 * uM) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); gl_PointSize = 0.0; return; }
  gl_Position = uViewProj * vec4(aPos, 1.0);
  gl_PointSize = clamp(uPx * 0.1 / dist, 1.2, 10.0);
  float rate = clamp(aRate, 0.0, 1.0);
  float hot = clamp(4.0 * uM / length(aPos), 0.0, 1.0);
  vec3 base = aColor < 0.5 ? vec3(1.0, 0.55, 0.25) : vec3(0.55, 0.75, 1.0);
  vCol = mix(base, vec3(1.0, 0.92, 0.8), hot) * (1.4 + 4.0 * hot) * rate * rate;
  vA = 1.0;
}`;
export const POINTS_FS = `#version 300 es
precision highp float;
in vec3 vCol; in float vA; out vec4 o;
void main() {
  vec2 p = gl_PointCoord * 2.0 - 1.0;
  float f = exp(-dot(p, p) * 3.0);
  o = vec4(vCol * f, 1.0);
}`;

// Lines for trails and the spacetime-fabric embedding grid (screen pass).
export const LINES_VS = `#version 300 es
in vec3 aPos; in vec4 aCol;
uniform mat4 uViewProj;
out vec4 vCol;
void main() { gl_Position = uViewProj * vec4(aPos, 1.0); vCol = aCol; }`;
export const LINES_FS = `#version 300 es
precision highp float;
in vec4 vCol; out vec4 o;
void main() { o = vCol; }`;
