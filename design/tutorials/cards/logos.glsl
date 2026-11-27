// App logo cards: the icon and the name, 3 s, opaque. The picture fades to the ground in the last 0.35 s
// so the cut to the intro title is clean.

// @card logo-01 | Assemble | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.27}]
// The two halves slide in from the sides and lock together; the white lenses light; the name rises.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  vec3 bg = mix(MP_INK, MP_DEEP, length(p) * 1.3);
  float k = eOut(ramp(0.1, 0.85));
  vec4 lg = mpLogo(p, vec2(0.0, 0.07), 0.15, (1.0 - k) * 4.0, ramp(0.75, 1.05));
  vec3 col = mix(bg, lg.rgb, lg.a);
  col += MP_CYAN * 0.25 * exp(-length(p - vec2(0.0, 0.07)) * 9.0) * (ramp(0.8, 0.9) - ramp(0.9, 1.6));
  col = mix(col, MP_WHITE, textFill(wordmark, uv + vec2(0.0, 0.025 * (1.0 - eOut(ramp(0.9, 1.6))))) * ramp(0.9, 1.4));
  fragColor = vec4(mix(col, bg, ramp(uDuration - 0.35, uDuration)), 1.0);
}

// @card logo-02 | Coin flip | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.27}]
// The icon spins in like a tossed coin, lands face up, and a glint runs across it.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  vec3 bg = mix(MP_INK, MP_DEEP, length(p) * 1.3);
  vec2 c = vec2(0.0, 0.07);
  float a = (1.0 - eOut(ramp(0.05, 1.2))) * 9.42478;
  float cs = cos(a);
  float sx = max(abs(cs), 0.02);
  vec2 q = vec2((p.x - c.x) / sx * sign(cs + 1e-4), p.y - c.y) + c;
  float sc = mix(0.6, 1.0, eOut(ramp(0.05, 1.2)));
  vec4 lg = mpLogo(q, c, 0.15 * sc, 0.0, 1.0);
  lg.rgb *= mix(0.55, 1.0, abs(cs));
  vec3 col = mix(bg, lg.rgb, lg.a * step(abs(p.x - c.x), 0.15 * sc * sx + 0.01) * ramp(0.05, 0.2));
  float g = (p.x + (p.y - c.y) * 0.6) - mix(-0.3, 0.3, ramp(1.25, 1.75));
  col += vec3(0.6) * exp(-g * g * 900.0) * lg.a * step(1.25, mpT()) * (1.0 - ramp(1.7, 1.8));
  col = mix(col, MP_WHITE, textFill(wordmark, uv) * ramp(1.2, 1.6));
  fragColor = vec4(mix(col, bg, ramp(uDuration - 0.35, uDuration)), 1.0);
}

// @card logo-03 | Draw on | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.27}]
// A pen traces each shape of the icon round from the top, then the shapes fill with colour.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  vec3 bg = mix(MP_INK, MP_DEEP, length(p) * 1.3);
  vec2 c = vec2(0.0, 0.07); float r = 0.15;
  vec2 q = (p - c) / r;
  float ang = (atan(q.x, q.y) + 3.14159) / 6.28318;
  float prog = eInOut(ramp(0.1, 1.2));
  float drawn = step(1.0 - ang, prog);
  float w = px() / r;
  float lw = 2.2 * w;
  vec3 col = bg;
  col = mix(col, MP_BLUE, (1.0 - smoothstep(lw * 0.5, lw, abs(sdBlue(q)))) * drawn);
  col = mix(col, MP_CYAN, (1.0 - smoothstep(lw * 0.5, lw, abs(sdCyan(q)))) * drawn);
  col = mix(col, MP_WHITE, (1.0 - smoothstep(lw * 0.5, lw, abs(min(sdLensL(q), sdLensR(q))))) * step(1.0 - ang, prog - 0.12));
  vec4 lg = mpLogo(p, c, r, 0.0, 1.0);
  col = mix(col, lg.rgb, lg.a * ramp(1.15, 1.55));
  col = mix(col, MP_WHITE, textFill(wordmark, uv) * step(uv.x, mix(wordmarkBox.x, wordmarkBox.z + 0.01, ramp(1.3, 1.9))));
  fragColor = vec4(mix(col, bg, ramp(uDuration - 0.35, uDuration)), 1.0);
}

// @card logo-04 | Eclipse | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.27}]
// A dark disc sits over the icon with a ring of light round it, then slides away to uncover it.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  vec3 bg = MP_DEEP;
  bg += vec3(0.6) * step(0.997, hash12(floor(p * 220.0))) * 0.5;   // a few stars
  vec2 c = vec2(0.0, 0.07); float r = 0.15;
  vec4 lg = mpLogo(p, c, r, 0.0, 1.0);
  vec3 col = mix(bg, lg.rgb, lg.a);
  float m = eInOut(ramp(0.55, 1.5));
  vec2 oc = c + vec2(m * 0.42, m * 0.12);
  float od = length(p - oc) - r * 1.03;
  float corona = exp(-max(od, 0.0) / 0.018) * (0.4 + 0.6 * ramp(0.0, 0.5)) * (1.0 - m);
  float rays = 0.6 + 0.4 * fbm(vec2(atan(p.y - oc.y, p.x - oc.x) * 3.0, mpT() * 0.6));
  col += MP_CYAN * corona * rays * step(0.0, od);
  col = mix(col, bg * 0.6, cover(od, px()) * (1.0 - ramp(1.45, 1.5)));
  col = mix(col, MP_WHITE, textFill(wordmark, uv) * ramp(1.35, 1.8));
  fragColor = vec4(mix(col, MP_DEEP, ramp(uDuration - 0.35, uDuration)), 1.0);
}

// @card logo-05 | Fill up | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.27}]
// Each half fills like a glass, blue then cyan, with a wave on the surface; the lenses light last.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  float t = mpT();
  vec3 bg = mix(MP_INK, MP_DEEP, length(p) * 1.3);
  vec2 c = vec2(0.0, 0.07); float r = 0.15;
  vec2 q = (p - c) / r;
  float w = px() / r;
  float calm = 1.0 - ramp(1.4, 1.9);
  float lvL = mix(-1.15, 1.15, eInOut(ramp(0.1, 1.3))) + 0.07 * sin(q.x * 7.0 + t * 8.0) * calm;
  float lvR = mix(-1.15, 1.15, eInOut(ramp(0.3, 1.5))) + 0.07 * sin(q.x * 7.0 - t * 8.0 + 1.0) * calm;
  vec3 col = bg;
  float ring = 1.0 - smoothstep(w, w * 2.5, abs(sdDisc(q)));
  col = mix(col, MP_WHITE * 0.35, ring * 0.6 * (1.0 - ramp(1.3, 1.6)));
  col = mix(col, MP_BLUE, cover(sdBlue(q), w) * step(q.y, lvL));
  col = mix(col, MP_CYAN, cover(sdCyan(q), w) * step(q.y, lvR));
  col = mix(col, MP_WHITE, cover(min(sdLensL(q), sdLensR(q)), w) * ramp(1.45, 1.75));
  col = mix(col, MP_WHITE, textFill(wordmark, uv) * ramp(1.5, 1.9));
  fragColor = vec4(mix(col, bg, ramp(uDuration - 0.35, uDuration)), 1.0);
}

// @card logo-06 | App tile | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":9,"bold":true,"align":"left","x":0.46,"y":0.5}]
// The app's own icon tile pops up with a bounce and a soft shadow; the name slides out beside it.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  vec3 bg = mix(vec3(0.05, 0.16, 0.25), MP_DEEP, length(p - vec2(-0.3, 0.1)) * 1.1);
  vec2 c = vec2(-0.31, 0.0);
  float s = max(eBack(ramp(0.1, 0.65)), 1e-3);
  vec2 q = (p - c) / s;
  float half_ = 0.18;
  float td = sdRBox(q, vec2(half_), 0.04);
  float shadow = exp(-max(sdRBox(p - c - vec2(0.0, -0.025), vec2(half_ * s), 0.04 * s), 0.0) / 0.03) * 0.45 * ramp(0.1, 0.5);
  vec3 col = bg * (1.0 - shadow);
  col = mix(col, MP_INK, cover(td, px() / s));
  vec4 lg = mpLogo(q, vec2(0.0), 0.12, 0.0, 1.0);
  col = mix(col, lg.rgb, lg.a * cover(td, px()));
  float k = eOut(ramp(0.6, 1.3));
  float tm = smoothstep(0.43, 0.445, uv.x);
  col = mix(col, MP_WHITE, textFill(wordmark, uv + vec2(0.12 * (1.0 - k), 0.0)) * tm);
  fragColor = vec4(mix(col, bg, ramp(uDuration - 0.35, uDuration)), 1.0);
}

// @card logo-07 | Column drop | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.27}]
// The icon falls into place in thin columns, each on its own beat, like a picture being stacked.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  vec3 bg = mix(MP_INK, MP_DEEP, length(p) * 1.3);
  vec2 c = vec2(0.0, 0.07); float r = 0.15;
  float cw = 0.012;
  float col_id = floor((p.x - c.x) / cw);
  float h = hash12(vec2(col_id, 3.0));
  float d0 = 0.1 + h * 0.6 + abs(col_id) * 0.012;
  float k = eBack(ramp(d0, d0 + 0.5));
  float off = (1.0 - k) * 0.9;
  vec4 lg = mpLogo(p - vec2(0.0, off), c, r, 0.0, 1.0);
  float edge = smoothstep(0.0, px() * 1.5, abs(fract((p.x - c.x) / cw) - 0.5) * cw - cw * 0.5 + px() * 1.5);
  lg.rgb *= mix(1.0, 0.82 + 0.18 * h, 1.0 - ramp(d0 + 0.4, d0 + 0.7));
  vec3 col = mix(bg, lg.rgb, lg.a);
  col = mix(col, MP_WHITE, textFill(wordmark, uv) * ramp(1.3, 1.7));
  fragColor = vec4(mix(col, bg, ramp(uDuration - 0.35, uDuration)), 1.0);
}

// @card logo-08 | Neon | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.27}]
// The icon is bent glass tubing that flickers on, one half then the other, and stays lit.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  float t = mpT();
  vec3 bg = MP_DEEP * (0.8 + 0.4 * fbm(p * 18.0));
  vec2 c = vec2(0.0, 0.07); float r = 0.15;
  vec2 q = (p - c) / r;
  float fl = step(0.45, hash12(vec2(floor(t * 22.0), 1.0)));
  float onL = t < 0.25 ? 0.0 : (t < 0.75 ? fl : 1.0);
  float onR = t < 0.5 ? 0.0 : (t < 1.0 ? step(0.45, hash12(vec2(floor(t * 19.0), 7.0))) : 1.0);
  float onW = t < 0.9 ? 0.0 : (t < 1.25 ? step(0.4, hash12(vec2(floor(t * 25.0), 3.0))) : 1.0);
  vec3 col = bg;
  float dB = abs(sdBlue(q)) * r, dC = abs(sdCyan(q)) * r, dW = abs(min(sdLensL(q), sdLensR(q))) * r;
  col += MP_BLUE * exp(-dB / 0.014) * 0.7 * onL;
  col += MP_CYAN * exp(-dC / 0.014) * 0.7 * onR;
  col += MP_WHITE * exp(-dW / 0.012) * 0.45 * onW;
  col = mix(col, mix(MP_BLUE, vec3(1.0), 0.55), (1.0 - smoothstep(0.0015, 0.003, dB)) * onL);
  col = mix(col, mix(MP_CYAN, vec3(1.0), 0.55), (1.0 - smoothstep(0.0015, 0.003, dC)) * onR);
  col = mix(col, vec3(1.0), (1.0 - smoothstep(0.0015, 0.003, dW)) * onW);
  float on2 = t < 1.3 ? 0.0 : (t < 1.6 ? step(0.4, hash12(vec2(floor(t * 23.0), 9.0))) : 1.0);
  float wd = textDistance(wordmark, uv);
  col += MP_CYAN * pow(textHalo(wordmark, uv), 4.0) * 0.5 * on2;
  col = mix(col, vec3(0.92, 0.98, 1.0), (1.0 - smoothstep(0.001, 0.0025, abs(wd))) * on2);
  fragColor = vec4(mix(col, bg, ramp(uDuration - 0.35, uDuration)), 1.0);
}

// @card logo-09 | Converge | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.27}]
// A blue ghost and a cyan ghost of the icon swing in from opposite corners and land as one, with a flash.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  float t = mpT();
  vec3 bg = mix(MP_INK, MP_DEEP, length(p) * 1.3);
  float hit = 1.0;
  float shake = (1.0 - ramp(hit, hit + 0.3)) * step(hit, t);
  p += vec2(sin(t * 90.0), cos(t * 77.0)) * 0.006 * shake;
  vec2 c = vec2(0.0, 0.07); float r = 0.15;
  float k = eInOut(ramp(0.05, hit));
  float u = 1.0 - k;
  vec2 pa = c + rot(-0.9 * u) * (p - c - vec2(-0.55, 0.22) * u);
  vec2 pb = c + rot(0.9 * u) * (p - c - vec2(0.55, -0.22) * u);
  float aa = mpLogo(pa, c, r, 0.0, 1.0).a, ab = mpLogo(pb, c, r, 0.0, 1.0).a;
  vec3 col = bg;
  col += MP_BLUE * aa * 0.75 * (1.0 - step(hit, t));
  col += MP_CYAN * ab * 0.75 * (1.0 - step(hit, t));
  vec4 lg = mpLogo(p, c, r, 0.0, 1.0);
  col = mix(col, lg.rgb, lg.a * step(hit, t));
  float rr = (t - hit) * 0.8;
  col += MP_WHITE * (1.0 - smoothstep(0.0, 0.006, abs(length(p - c) - r - rr))) * (1.0 - ramp(hit, hit + 0.6)) * step(hit, t);
  col += MP_WHITE * 0.5 * exp(-length(p - c) * 6.0) * (1.0 - ramp(hit, hit + 0.25)) * step(hit, t);
  col = mix(col, MP_WHITE, textFill(wordmark, uv) * ramp(1.3, 1.7));
  fragColor = vec4(mix(col, bg, ramp(uDuration - 0.35, uDuration)), 1.0);
}

// @card logo-10 | Construction | 3
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.27}]
// The geometry behind the icon is drafted first: its circle, the two cuts, the lens arcs. Then it is inked.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  // Dark ground. At the end the grid, the drafting and the name fade but the icon stays, so the
  // "Through the gap" intro can pick it up where it is (centre 0, 0.07, radius 0.15).
  float leave = 1.0 - ramp(uDuration - 0.4, uDuration);
  vec3 ground = MP_DEEP * 0.75;
  vec3 bg = ground;
  vec2 g = abs(fract(p * 20.0 + 0.5) - 0.5) / 20.0;
  bg = mix(bg, MP_BLUE, (1.0 - smoothstep(0.0, px() * 1.2, min(g.x, g.y))) * 0.05 * leave);
  vec2 c = vec2(0.0, 0.07); float r = 0.15;
  vec2 q = (p - c) / r;
  float lw = 1.3 * px() / r;
  float ang = (atan(q.y, q.x) + 3.14159) / 6.28318;
  float dash = step(0.5, fract(ang * 48.0));
  vec3 col = bg;
  float c1 = (1.0 - smoothstep(0.0, lw, abs(sdDisc(q)))) * step(ang, eOut(ramp(0.05, 0.6)));
  float v = (1.0 - smoothstep(0.0, lw, abs(abs(q.x) - 0.278))) * step(abs(q.y), 1.25 * eOut(ramp(0.35, 0.75)));
  float a1 = length(q - vec2(-0.637, 0.0)), a2 = length(q - vec2(0.637, 0.0));
  float arcs = (1.0 - smoothstep(0.0, lw, min(abs(a1 - 0.634), abs(a2 - 0.634)))) * ramp(0.6, 0.9) * dash;
  float cross_ = (1.0 - smoothstep(0.0, lw, min(abs(q.x), abs(q.y)))) * step(length(q), 1.35) * ramp(0.1, 0.4) * 0.5;
  float marks = max(max(c1, v), max(arcs, cross_));
  col = mix(col, MP_CYAN, marks * (1.0 - 0.6 * ramp(1.2, 1.6)) * leave);
  vec4 lg = mpLogo(p, c, r, 0.0, 1.0);
  float ink = step(q.x, mix(-1.1, 1.1, eInOut(ramp(0.95, 1.45))));
  col = mix(col, lg.rgb, lg.a * ink);
  col = mix(col, MP_WHITE, textFill(wordmark, uv) * ramp(1.4, 1.8) * leave);
  fragColor = vec4(col, 1.0);
}
