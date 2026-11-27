// Intro titles: the tutorial's name, 4 s, opaque, dissolving into the video over the last 0.5 s.
// Each card starts at an `@card` line; mp.glsl goes in front of it.

// @card intro-01 | Halves part | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":11,"bold":true,"align":"center","x":0.5,"y":0.5},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.6,"bold":true,"align":"center","x":0.5,"y":0.62}]
// The two halves of the icon fill the frame, then part like doors; the inner edges carry the white lens.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution;
  float k = eInOut(ramp(0.25, 1.3));
  float edge = 0.5 * (1.0 - k) - 0.1 * k;           // where each panel's flat side sits
  float yy = (uv.y - 0.5) * 2.0;
  float bulge = 0.07 * sqrt(max(0.0, 1.0 - yy * yy)); // the lens, bowed like the icon's
  float w = 1.5 / uResolution.x;
  vec3 col = mix(MP_DEEP, MP_INK, uv.y);
  float s = mix(1.08, 1.0, eOut(ramp(0.45, 1.7)));
  col = mix(col, MP_WHITE, textFill(title, (uv - 0.5) / s + 0.5) * ramp(0.5, 1.1));
  col = mix(col, MP_CYAN, textFill(kicker, uv + vec2(0.0, 0.03 * (1.0 - eOut(ramp(0.9, 1.6))))) * ramp(0.9, 1.5));
  float blue = 1.0 - smoothstep(edge - w, edge + w, uv.x);
  float lensL = (1.0 - smoothstep(edge + bulge - w, edge + bulge + w, uv.x)) * (1.0 - blue);
  float cyan = smoothstep(1.0 - edge - w, 1.0 - edge + w, uv.x);
  float lensR = smoothstep(1.0 - edge - bulge - w, 1.0 - edge - bulge + w, uv.x) * (1.0 - cyan);
  col = mix(col, MP_WHITE, lensL + lensR);
  col = mix(col, MP_BLUE, blue);
  col = mix(col, MP_CYAN, cyan);
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}

// @card intro-02 | Through the gap | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":10,"bold":true,"align":"center","x":0.5,"y":0.5},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.5,"y":0.61}]
// The camera dives into the gap between the icon's halves; the title waits on the far side.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  // Starts on the icon exactly as the Construction logo card leaves it, on the same dark ground.
  float z = eInOut(ramp(0.15, 1.45));
  float r = 0.15 * exp(z * 9.0);
  vec2 c = vec2(0.0, 0.07 * (1.0 - z));
  vec3 col = mix(MP_DEEP * 0.75, mix(MP_DEEP, MP_INK, 0.5 + 0.5 * uv.y), z);
  col += MP_CYAN * 0.10 * exp(-abs(p.x) * 9.0) * z;      // light spilling through the gap
  vec4 logo = mpLogo(p, c, r, 0.0, 1.0);
  col = mix(col, logo.rgb, logo.a);
  float a = ramp(1.15, 1.7);
  float s = mix(0.92, 1.0, eOut(ramp(1.15, 2.0)));
  col = mix(col, MP_WHITE, textFill(title, (uv - 0.5) / s + 0.5) * a);
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(1.4, 1.9));
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}

// @card intro-03 | Blueprint | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":11,"bold":true,"align":"left","x":0.1,"y":0.47},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"left","x":0.1,"y":0.6}]
// A drafting grid draws itself; the title is traced as an outline by a scan line, then filled.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  vec3 col = MP_INK;
  vec2 g = abs(fract(p * 16.0 + 0.5) - 0.5) / 16.0;
  vec2 g4 = abs(fract(p * 4.0 + 0.5) - 0.5) / 4.0;
  float grid = max(1.0 - smoothstep(0.0, px() * 1.2, min(g.x, g.y)), 0.0) * 0.10;
  grid += (1.0 - smoothstep(0.0, px() * 1.4, min(g4.x, g4.y))) * 0.18;
  float drawn = step(uv.x, eOut(ramp(0.0, 1.0)) * 1.05);
  col = mix(col, MP_BLUE, grid * drawn);
  float scan = mix(-0.05, 1.1, eInOut(ramp(0.35, 1.6)));
  float d = textDistance(title, uv);
  float outline = 1.0 - smoothstep(0.0012, 0.0028, abs(d));
  float traced = step(uv.x, scan);
  col = mix(col, MP_CYAN, outline * traced);
  col = mix(col, MP_WHITE, textFill(title, uv) * ramp(1.5, 2.0));
  col += MP_CYAN * 0.55 * exp(-abs(uv.x - scan) * 220.0) * step(scan, 1.05) * step(0.0, scan);
  col = mix(col, MP_CYAN, textFill(kicker, uv) * step(uv.x, scan) * ramp(0.6, 1.0));
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}

// @card intro-04 | Letter drop | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":12,"bold":true,"align":"center","x":0.5,"y":0.48},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.5,"y":0.63}]
// Each letter drops in on its own beat, flashing blue or cyan as it lands, then settles white.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution;
  vec3 col = mix(MP_DEEP, MP_INK, smoothstep(0.0, 1.0, uv.y));
  float shadow = 0.0;
  for (int i = 0; i < 64; i++) {
    if (i >= titleCount) break;
    vec4 b = titleGlyphs[i];
    float d0 = 0.3 + float(i) * 0.07;
    float k = eBack(ramp(d0, d0 + 0.55));
    float off = (1.0 - k) * 0.22;
    vec2 sp = uv - vec2(0.0, off);
    if (sp.x < b.x - 0.004 || sp.x > b.z + 0.004 || sp.y < b.y - 0.02 || sp.y > b.w + 0.02) continue;
    float a = textFill(title, sp) * ramp(d0, d0 + 0.12);
    vec3 flash = mod(float(i), 2.0) < 0.5 ? MP_BLUE : MP_CYAN;
    vec3 c = mix(flash, MP_WHITE, ramp(d0 + 0.35, d0 + 0.8));
    col = mix(col, c, a);
  }
  // A soft floor shadow the letters land on.
  float floorY = titleBox.y - 0.01;
  col *= 1.0 - 0.25 * exp(-pow((uv.y - floorY) * 60.0, 2.0)) * step(titleBox.x, uv.x) * step(uv.x, titleBox.z) * ramp(0.4, 1.2);
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(1.2, 1.7));
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}

// @card intro-05 | Parity slices | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":12,"bold":true,"align":"center","x":0.5,"y":0.5},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.5,"y":0.645}]
// The title arrives in horizontal slices, alternate ones from the left and the right, meeting in the middle.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution;
  vec3 col = MP_INK;
  // The ground is split down the middle too: a hair darker on the left.
  col = mix(col * 0.82, col, smoothstep(0.495, 0.505, uv.x));
  float bands = 9.0;
  float rel = (uv.y - titleBox.y) / max(titleBox.w - titleBox.y, 1e-3);
  float band = floor(clamp(rel, 0.0, 0.999) * bands);
  float side = mod(band, 2.0) < 0.5 ? -1.0 : 1.0;
  float d0 = 0.25 + band * 0.045;
  float k = eOut(ramp(d0, d0 + 0.8));
  vec2 sp = uv + vec2(side * (1.0 - k) * 0.7, 0.0);
  vec3 tint = side < 0.0 ? MP_BLUE : MP_CYAN;
  col = mix(col, mix(tint, MP_WHITE, ramp(d0 + 0.5, d0 + 1.0)), textFill(title, sp) * ramp(d0, d0 + 0.2));
  // The kicker types itself out.
  float typed = mix(kickerBox.x, kickerBox.z + 0.01, ramp(1.2, 1.9));
  col = mix(col, MP_CYAN, textFill(kicker, uv) * step(uv.x, typed));
  float caret = step(abs(uv.x - typed - 0.004), 0.0015) * step(kickerBox.y, uv.y) * step(uv.y, kickerBox.w) * step(1.2, mpT()) * step(fract(mpT() * 2.5), 0.6);
  col = mix(col, MP_CYAN, caret);
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}

// @card intro-06 | Aurora | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":11,"bold":true,"align":"center","x":0.5,"y":0.52},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.5,"y":0.65}]
// Blue and cyan light drifts below; the letters grow from hairlines to full weight inside a soft halo.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float t = mpT();
  vec3 col = MP_DEEP;
  float n = fbm(vec2(p.x * 1.6 + t * 0.12, p.y * 2.2 - t * 0.05));
  float m = fbm(vec2(p.x * 2.3 - t * 0.09 + 4.0, p.y * 1.4 + 2.0));
  float band = exp(-pow((p.y + 0.22 - 0.18 * n) * 3.2, 2.0));
  col += MP_BLUE * band * 0.55 * smoothstep(0.3, 0.8, n);
  col += MP_CYAN * band * 0.45 * smoothstep(0.35, 0.85, m);
  col *= ramp(0.0, 0.9);
  float grow = mix(0.009, 0.0, eOut(ramp(0.35, 1.5)));
  float d = textDistance(title, uv);
  float letters = smoothstep(grow - 0.0015, grow + 0.0015, d) * ramp(0.35, 0.6);
  col += MP_CYAN * 0.35 * pow(textHalo(title, uv), 3.0) * ramp(0.4, 1.4);
  col = mix(col, MP_WHITE, letters);
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(1.2, 1.7));
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}

// @card intro-07 | Halftone | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":13,"bold":true,"align":"center","x":0.5,"y":0.5},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.5,"y":0.66}]
// The title is printed in dots that swell until they join into solid letters.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float cell = 0.016;
  vec2 id = floor(p / cell), c = (id + 0.5) * cell;
  vec2 cuv = mpUV(c);
  float cov = textFill(title, cuv);
  float bg = 0.12 * smoothstep(0.55, 0.0, length(p - vec2(0.75, -0.4)));   // a faint dot field in the corner
  float h = hash12(id);
  float grow = eOut(ramp(0.2 + h * 0.4, 1.4 + h * 0.3));
  float r = cell * 0.5 * sqrt(max(cov, bg)) * grow * 1.15;
  float dt = cover(length(p - c) - r, px() * 0.8);
  vec3 col = MP_INK;
  vec3 ink = cov > 0.5 ? mix(MP_CYAN, MP_WHITE, ramp(1.0, 1.6)) : MP_BLUE;
  col = mix(col, ink, dt * (cov > 0.5 ? 1.0 : 0.7));
  float solid = ramp(1.6, 2.1);
  col = mix(col, MP_WHITE, textFill(title, uv) * solid);
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(1.6, 2.0));
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}

// @card intro-08 | Ripple and register | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":11,"bold":true,"align":"center","x":0.5,"y":0.5},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.5,"y":0.63}]
// Rings spread from the centre; a blue copy and a cyan copy of the title slide into register and turn white.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float t = mpT();
  vec3 col = mix(MP_INK, MP_DEEP, length(p));
  for (int i = 0; i < 4; i++) {
    float st = 0.1 + float(i) * 0.22;
    float rr = (t - st) * 0.55;
    float fade = (1.0 - sat((t - st) / 1.8)) * step(st, t);
    col = mix(col, MP_CYAN, (1.0 - smoothstep(0.0, px() * 2.5, abs(length(p) - rr))) * fade * 0.6);
  }
  float d = 0.035 * (1.0 - eOut(ramp(0.3, 1.35)));
  float a = ramp(0.25, 0.6);
  float b = textFill(title, uv + vec2(d, d * 0.3)) * a;
  float c = textFill(title, uv - vec2(d, d * 0.3)) * a;
  vec3 ink = MP_BLUE * b * (1.0 - c) + MP_CYAN * c * (1.0 - b) + MP_WHITE * b * c;
  col = mix(col, ink, max(b, c));
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(1.2, 1.7));
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}

// @card intro-09 | Stripe sweep | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":11,"bold":true,"align":"left","x":0.1,"y":0.46},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.3,"bold":true,"align":"left","x":0.113,"y":0.6}]
// A band of brand stripes sweeps across on a slant and leaves the title behind it; the kicker sits in a tag.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float aspect = uResolution.x / uResolution.y;
  float slant = p.x + p.y * 0.45;
  float front = mix(-1.5, 1.9, eInOut(ramp(0.1, 1.4)));
  float band = step(front - 0.55, slant) * step(slant, front);
  vec3 col = MP_INK;
  float behind = step(slant, front - 0.55);
  col = mix(col, MP_WHITE, textFill(title, uv) * behind);
  // The tag: a cyan pill with dark words.
  vec2 k0 = kickerBox.xy - vec2(0.012, 0.018), k1 = kickerBox.zw + vec2(0.012, 0.018);
  vec2 kc = (k0 + k1) * 0.5, kh = (k1 - k0) * 0.5;
  float pd = sdRBox((uv - kc) * vec2(aspect, 1.0), kh * vec2(aspect, 1.0), 0.02);
  float pill = cover(pd, px()) * behind * ramp(1.2, 1.5);
  col = mix(col, MP_CYAN, pill);
  col = mix(col, MP_INK, textFill(kicker, uv) * pill);
  float s = fract(slant * 9.0);
  vec3 stripe = s < 0.45 ? MP_BLUE : (s < 0.9 ? MP_CYAN : MP_WHITE);
  col = mix(col, stripe, band);
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}

// @card intro-10 | Mosaic resolve | 4
// @inputs [{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":12,"bold":true,"align":"center","x":0.5,"y":0.5},{"name":"kicker","type":"text","value":"MultiParity tutorial","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.5,"y":0.645}]
// Big coloured blocks refine step by step into the title, like a picture coming in over a slow line.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float k = ramp(0.2, 1.5);
  float stepN = floor(k * 6.0);
  float cell = 0.12 * pow(0.5, stepN);
  vec2 id = floor(p / cell), c = (id + 0.5) * cell;
  float cov = textFill(title, mpUV(c));
  // Wider blocks average more of the letters, so sample a few points in the block.
  cov = (cov + textFill(title, mpUV(c + cell * vec2(0.3, 0.3))) + textFill(title, mpUV(c - cell * vec2(0.3, 0.3)))
        + textFill(title, mpUV(c + cell * vec2(0.3, -0.3))) + textFill(title, mpUV(c + cell * vec2(-0.3, 0.3)))) / 5.0;
  float h = hash12(id + stepN * 7.0);
  vec3 tint = h < 0.5 ? MP_BLUE : MP_CYAN;
  vec3 col = MP_INK + (h - 0.5) * 0.03;
  float sharp = step(5.5, k * 6.0);
  vec3 ink = mix(tint, MP_WHITE, ramp(1.1, 1.6));
  col = mix(col, ink, mix(cov, textFill(title, uv), sharp) * step(0.01, k));
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(1.5, 1.9));
  fragColor = vec4(col, 1.0 - ramp(uDuration - 0.5, uDuration));
}
