// Spot: everything but the target dims, with a thin accent edge around it.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  float e = envIn() * envOut();
  float d = boxDist(fragCoord, 8.0, 12.0);
  float outside = smoothstep(-2.0, 10.0, d);
  float edge = 1.0 - smoothstep(1.0, 2.6, abs(d));
  vec3 col = mix(vec3(0.0, 0.03, 0.06), accent, edge);
  float a = max(outside * 0.62, edge * 0.95) * e;
  fragColor = vec4(col, a);
}
