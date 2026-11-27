// Ring: an outline that snaps onto the target, then breathes; a soft glow behind it.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  float t = uProgress * uDuration;
  float pop = easeOutBack(clamp(t / 0.35, 0.0, 1.0));
  float grow = mix(28.0, 6.0, pop) + 2.5 * sin(t * 5.0) * step(0.35, t);
  float d = boxDist(fragCoord, grow, 10.0);
  float line = 1.0 - smoothstep(1.6, 3.2, abs(d));
  float glow = exp(-max(d, 0.0) / 10.0) * 0.45 * (1.0 - step(0.0, -d));
  // A second ring that ripples outward once on arrival.
  float r2 = mix(6.0, 46.0, clamp((t - 0.25) / 0.7, 0.0, 1.0));
  float d2 = boxDist(fragCoord, r2, 12.0);
  float wave = (1.0 - smoothstep(1.0, 3.0, abs(d2))) * (1.0 - clamp((t - 0.25) / 0.7, 0.0, 1.0)) * step(0.25, t);
  float a = clamp(line + glow + wave * 0.8, 0.0, 1.0) * min(envIn() * 1.5, 1.0) * envOut();
  fragColor = vec4(accent, a);
}
