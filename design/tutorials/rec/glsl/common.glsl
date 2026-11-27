// Shared by the callouts. Inputs (uniforms): boxMin, boxMax (vec2, 0..1 from bottom left),
// accent (vec3). The callout eases in over 0.3 s and out over the last 0.35 s.
float sdRoundBox(vec2 p, vec2 b, float r) { vec2 q = abs(p) - b + r; return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r; }
float envIn() { float t = uProgress * uDuration; return smoothstep(0.0, 0.3, t); }
float envOut() { float t = uProgress * uDuration; return 1.0 - smoothstep(uDuration - 0.35, uDuration, t); }
float easeOutBack(float x) { float c1 = 1.70158, c3 = c1 + 1.0; return 1.0 + c3 * pow(x - 1.0, 3.0) + c1 * pow(x - 1.0, 2.0); }
// Distance in pixels to the box outline, the box grown by `grow` pixels.
float boxDist(vec2 frag, float grow, float radius) {
  vec2 a = boxMin * uResolution, b = boxMax * uResolution;
  vec2 c = (a + b) * 0.5, h = (b - a) * 0.5 + grow;
  return sdRoundBox(frag - c, h, radius);
}
