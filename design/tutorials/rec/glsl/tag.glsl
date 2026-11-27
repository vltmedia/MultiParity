// Tag: an outline on the target and a label that slides out beside it, joined by a short line.
// The label's text is the `label` text input; labelBox is where Kitlig drew it.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  float t = uProgress * uDuration;
  vec2 uv = fragCoord / uResolution;
  float e = envOut();
  // Outline on the target.
  float d = boxDist(fragCoord, 6.0, 9.0);
  float edge = (1.0 - smoothstep(1.2, 2.8, abs(d))) * envIn();
  // The label: a pill behind the text, wiping in from the side nearest the target.
  float k = clamp((t - 0.15) / 0.35, 0.0, 1.0); k = 1.0 - pow(1.0 - k, 3.0);
  vec2 p0 = labelBox.xy * uResolution - vec2(16.0, 11.0), p1 = labelBox.zw * uResolution + vec2(16.0, 11.0);
  float fromLeft = step((boxMin.x + boxMax.x) * 0.5, labelBox.x);
  float wipe = mix(p1.x - (p1.x - p0.x) * k, p0.x + (p1.x - p0.x) * k, fromLeft);
  float shown = mix(step(wipe, fragCoord.x), step(fragCoord.x, wipe), fromLeft);
  vec2 c = (p0 + p1) * 0.5, h = (p1 - p0) * 0.5;
  float pd = sdRoundBox(fragCoord - c, h, 10.0);
  float pill = (1.0 - smoothstep(-1.0, 1.0, pd)) * shown;
  float txt = textFill(label, uv) * shown;
  // A short connector from the target's edge to the pill.
  vec2 tc = vec2(mix(boxMax.x, boxMin.x, 1.0 - fromLeft), (boxMin.y + boxMax.y) * 0.5) * uResolution + vec2(mix(-6.0, 6.0, fromLeft), 0.0);
  vec2 pc = vec2(mix(p1.x, p0.x, fromLeft), c.y);
  vec2 ab = pc - tc; float hh = clamp(dot(fragCoord - tc, ab) / dot(ab, ab), 0.0, 1.0 * k);
  float conn = (1.0 - smoothstep(0.8, 2.0, length(fragCoord - tc - ab * hh))) * step(0.01, k);
  vec3 col = accent; float a = max(edge * 0.95, max(conn * 0.9, pill));
  col = mix(col, vec3(0.027, 0.118, 0.18), txt);
  fragColor = vec4(col, a * e);
}
