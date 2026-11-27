// MultiParity card helpers, put in front of every card shader (like rec/glsl/common.glsl).
// Frame units: p = mpP(fragCoord), the frame 1 high with 0 at the centre and +y up.
const vec3 MP_INK = vec3(0.027, 0.118, 0.180);   // #071e2e, the icon's ground
const vec3 MP_DEEP = vec3(0.010, 0.050, 0.085);
const vec3 MP_BLUE = vec3(0.157, 0.529, 0.969);  // #2887f7
const vec3 MP_CYAN = vec3(0.149, 0.792, 0.953);  // #26caf3
const vec3 MP_WHITE = vec3(0.933, 0.965, 0.984); // #eef6fb

float mpT() { return uProgress * uDuration; }
float sat(float x) { return clamp(x, 0.0, 1.0); }
float ramp(float a, float b) { return sat((mpT() - a) / (b - a)); }
float eOut(float x) { x = sat(x); return 1.0 - pow(1.0 - x, 3.0); }
float eInOut(float x) { x = sat(x); return x < 0.5 ? 4.0 * x * x * x : 1.0 - pow(-2.0 * x + 2.0, 3.0) / 2.0; }
float eBack(float x) { x = sat(x); float c1 = 1.70158, c3 = c1 + 1.0; return 1.0 + c3 * pow(x - 1.0, 3.0) + c1 * pow(x - 1.0, 2.0); }
float hash12(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * 0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
float vnoise(vec2 p) {
  vec2 i = floor(p), f = fract(p); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash12(i), hash12(i + vec2(1, 0)), f.x), mix(hash12(i + vec2(0, 1)), hash12(i + vec2(1, 1)), f.x), f.y);
}
float fbm(vec2 p) { float s = 0.0, a = 0.5; for (int i = 0; i < 5; i++) { s += a * vnoise(p); p = p * 2.03 + 17.1; a *= 0.5; } return s; }
vec2 mpP(vec2 fc) { return (fc - 0.5 * uResolution) / uResolution.y; }
vec2 mpUV(vec2 p) { return vec2(p.x * uResolution.y / uResolution.x + 0.5, p.y + 0.5); }
float px() { return 1.0 / uResolution.y; }
// Coverage of a signed distance (negative inside) with an edge `w` wide.
float cover(float d, float w) { return 1.0 - smoothstep(-w, w, d); }
float sdRBox(vec2 p, vec2 b, float r) { vec2 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r; }
mat2 rot(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }
// Straight-alpha `a` over `b`.
vec4 over(vec4 a, vec4 b) { float o = a.a + b.a * (1.0 - a.a); return vec4((a.rgb * a.a + b.rgb * b.a * (1.0 - a.a)) / max(o, 1e-5), o); }

// The icon, in logo units (the disc is radius 1). Signed distances, negative inside.
// Two half discs (blue, cyan) with a white lens on each inner side and a dark gap between.
float sdDisc(vec2 q) { return length(q) - 1.0; }
float sdBlue(vec2 q) { return max(sdDisc(q), q.x + 0.278); }
float sdLensL(vec2 q) { return max(max(length(q - vec2(-0.637, 0.0)) - 0.634, -(q.x + 0.278)), sdDisc(q)); }
float sdCyan(vec2 q) { return max(sdDisc(q), -(q.x - 0.278)); }
float sdLensR(vec2 q) { return max(max(length(q - vec2(0.637, 0.0)) - 0.634, q.x - 0.278), sdDisc(q)); }
float sdLeft(vec2 q) { return min(sdBlue(q), sdLensL(q)); }
float sdRight(vec2 q) { return min(sdCyan(q), sdLensR(q)); }
float sdLogo(vec2 q) { return min(sdLeft(q), sdRight(q)); }
// The icon drawn at centre c, radius r (frame units). `sep` pushes the halves apart (logo units),
// `lens` fades the white lenses in.
vec4 mpLogo(vec2 p, vec2 c, float r, float sep, float lens) {
  float w = px() / r * 0.8;
  vec2 q = (p - c) / r;
  vec2 ql = q + vec2(sep, 0.0), qr = q - vec2(sep, 0.0);
  vec4 col = vec4(0.0);
  col = over(vec4(MP_BLUE, cover(sdBlue(ql), w)), col);
  col = over(vec4(MP_WHITE, cover(sdLensL(ql), w) * lens), col);
  col = over(vec4(MP_CYAN, cover(sdCyan(qr), w)), col);
  col = over(vec4(MP_WHITE, cover(sdLensR(qr), w) * lens), col);
  return col;
}
