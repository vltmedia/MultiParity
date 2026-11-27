// Sweep: a band of light passes across the target once, leaving a soft outline.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  float t = uProgress * uDuration;
  vec2 a = boxMin * uResolution, b = boxMax * uResolution;
  float d = boxDist(fragCoord, 4.0, 8.0);
  float inside = 1.0 - smoothstep(-1.0, 1.0, d);
  float x = (fragCoord.x - a.x) / max(b.x - a.x, 1.0);
  float pos = mix(-0.25, 1.25, clamp((t - 0.1) / 1.1, 0.0, 1.0));
  float band = exp(-pow((x - pos + (fragCoord.y - a.y) / max(b.x - a.x, 1.0) * 0.25) / 0.09, 2.0));
  float edge = 1.0 - smoothstep(1.0, 2.4, abs(d));
  float al = (inside * band * 0.55 + edge * 0.8) * envIn() * envOut();
  fragColor = vec4(mix(accent, vec3(1.0), inside * band * 0.4), al);
}
