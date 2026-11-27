// End cards: what to watch next, 5 s. They start see-through over the video's last frames and cover it.

// @card end-01 | Doors close | 5
// @inputs [{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.6,"bold":true,"align":"center","x":0.5,"y":0.6},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":10,"bold":true,"align":"center","x":0.5,"y":0.49}]
// The icon's halves close over the video like doors, then draw back to the edges to show what is next.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution;
  float t = mpT();
  float edge = 0.5 * eInOut(ramp(0.0, 0.9));
  edge = mix(edge, 0.06, eInOut(ramp(1.1, 1.9)));
  float yy = (uv.y - 0.5) * 2.0;
  float bulge = 0.07 * sqrt(max(0.0, 1.0 - yy * yy)) * mix(1.0, 0.4, ramp(1.1, 1.9));
  float w = 1.5 / uResolution.x;
  float shut = step(0.9, t);
  vec4 col = vec4(mix(MP_DEEP, MP_INK, uv.y), shut);
  col.rgb = mix(col.rgb, MP_CYAN, textFill(kicker, uv) * ramp(1.5, 2.0));
  col.rgb = mix(col.rgb, MP_WHITE, textFill(next, uv + vec2(0.0, 0.02 * (1.0 - eOut(ramp(1.6, 2.4))))) * ramp(1.6, 2.1));
  float blue = 1.0 - smoothstep(edge - w, edge + w, uv.x);
  float lensL = (1.0 - smoothstep(edge + bulge - w, edge + bulge + w, uv.x)) * (1.0 - blue) * step(0.001, edge);
  float cyan = smoothstep(1.0 - edge - w, 1.0 - edge + w, uv.x);
  float lensR = smoothstep(1.0 - edge - bulge - w, 1.0 - edge - bulge + w, uv.x) * (1.0 - cyan) * step(0.001, edge);
  col = over(vec4(MP_WHITE, lensL + lensR), col);
  col = over(vec4(MP_BLUE, blue), col);
  col = over(vec4(MP_CYAN, cyan), col);
  fragColor = col;
}

// @card end-02 | Up next card | 5
// @inputs [{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"left","x":0.6,"y":0.59},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":7,"bold":true,"align":"left","x":0.6,"y":0.5}]
// The video dims, a card slides in with the next video's name, and a bar counts down the seconds.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float aspect = uResolution.x / uResolution.y;
  float dim = 0.6 * ramp(0.0, 0.6);
  vec4 col = vec4(MP_DEEP, dim);
  float k = eOut(ramp(0.3, 1.0));
  float sx = (1.0 - k) * 0.5;
  vec2 a = vec2(0.565, 0.33), b = vec2(0.93, 0.68);
  vec2 cc = (a + b) * 0.5 + vec2(sx, 0.0), hh = (b - a) * 0.5;
  float d = sdRBox((uv - cc) * vec2(aspect, 1.0), hh * vec2(aspect, 1.0), 0.025);
  float card = cover(d, px());
  col = over(vec4(MP_INK, card * 0.96), col);
  col = over(vec4(MP_CYAN, (1.0 - smoothstep(0.0, px() * 1.5, abs(d))) * 0.6 * card), col);
  col = over(vec4(MP_CYAN, textFill(kicker, uv - vec2(sx, 0.0))), col);
  col = over(vec4(MP_WHITE, textFill(next, uv - vec2(sx, 0.0))), col);
  float prog = ramp(1.0, uDuration - 0.4);
  float barY = 0.385, x0 = 0.6 + sx, x1 = 0.895 + sx;
  float bar = step(abs(uv.y - barY), 0.004) * step(x0, uv.x) * step(uv.x, x1);
  col = over(vec4(MP_WHITE, bar * 0.15 * k), col);
  col = over(vec4(MP_CYAN, bar * step(uv.x, mix(x0, x1, prog)) * k), col);
  vec4 lg = mpLogo(p, vec2(-0.78, 0.4), 0.035, 0.0, 1.0);
  col = over(vec4(lg.rgb, lg.a * ramp(0.4, 0.9)), col);
  // At the very end the whole frame goes to the ground colour.
  col = over(vec4(MP_INK, ramp(uDuration - 0.6, uDuration)), col);
  fragColor = col;
}

// @card end-03 | Iris out | 5
// @inputs [{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.5,"y":0.33},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":7,"bold":true,"align":"center","x":0.5,"y":0.25}]
// The old film iris: a circle closes on the video and the icon appears where it shut.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  vec2 c = vec2(0.0, 0.09);
  float R = mix(1.15, 0.0, eInOut(ramp(0.1, 1.2)));
  float d = length(p - c) - R;
  vec4 col = vec4(mix(MP_INK, MP_DEEP, length(p) * 1.2), smoothstep(-px(), px(), d));
  col = over(vec4(MP_CYAN, (1.0 - smoothstep(0.0, px() * 2.5, abs(d))) * step(0.01, R)), col);
  float s = eBack(ramp(1.15, 1.6));
  vec4 lg = mpLogo(p, c, 0.11 * max(s, 1e-3), 0.0, 1.0);
  col = over(vec4(lg.rgb, lg.a * step(1.15, mpT())), col);
  col = over(vec4(MP_CYAN, textFill(kicker, uv) * ramp(1.5, 1.9)), col);
  col = over(vec4(MP_WHITE, textFill(next, uv) * ramp(1.6, 2.1)), col);
  fragColor = col;
}

// @card end-04 | Aurora | 5
// @inputs [{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.6,"bold":true,"align":"left","x":0.1,"y":0.6},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":12,"bold":true,"align":"left","x":0.1,"y":0.48},{"name":"foot","type":"text","value":"MultiParity tutorials","fontId":"space-grotesk","sizePct":2,"bold":true,"align":"left","x":0.1,"y":0.12}]
// The video gives way to slow blue and cyan light; the next video's name is set large on the left.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float t = mpT();
  vec3 col = MP_DEEP;
  float n = fbm(vec2(p.x * 1.4 + t * 0.1, p.y * 2.0 - t * 0.04));
  float m = fbm(vec2(p.x * 2.1 - t * 0.08 + 4.0, p.y * 1.3 + 2.0));
  float band = exp(-pow((p.y - 0.05 - p.x * 0.25 - 0.2 * n) * 2.6, 2.0));
  col += MP_BLUE * band * 0.5 * smoothstep(0.3, 0.8, n);
  col += MP_CYAN * band * 0.4 * smoothstep(0.35, 0.85, m);
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(0.7, 1.1));
  col = mix(col, MP_WHITE, textFill(next, uv + vec2(0.03 * (1.0 - eOut(ramp(0.8, 1.6))), 0.0)) * ramp(0.8, 1.3));
  col = mix(col, MP_WHITE * 0.7, textFill(foot, uv) * ramp(1.2, 1.6));
  vec4 lg = mpLogo(p, vec2(0.76, -0.38), 0.045, 0.0, 1.0);
  col = mix(col, lg.rgb, lg.a * ramp(1.2, 1.6));
  fragColor = vec4(col, eOut(ramp(0.0, 0.8)));
}

// @card end-05 | Countdown ring | 5
// @inputs [{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.6,"bold":true,"align":"left","x":0.42,"y":0.57},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":9,"bold":true,"align":"left","x":0.42,"y":0.47}]
// The icon sits inside a ring that fills as the seconds run out, beside the next video's name.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  vec2 c = vec2(-0.43, 0.02);
  vec3 col = mix(MP_INK, MP_DEEP, length(p - c) * 1.2);
  float s = eBack(ramp(0.3, 0.8));
  vec4 lg = mpLogo(p, c, 0.12 * max(s, 1e-3), 0.0, 1.0);
  col = mix(col, lg.rgb, lg.a);
  float rr = length(p - c) - 0.19;
  float ring = 1.0 - smoothstep(0.003, 0.0045, abs(rr));
  float ang = (atan(p.x - c.x, p.y - c.y) + 3.14159) / 6.28318;
  float fill = ramp(0.9, uDuration - 0.2);
  col = mix(col, MP_WHITE, ring * 0.12 * ramp(0.5, 0.9));
  col = mix(col, MP_CYAN, ring * step(1.0 - ang, fill));
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(0.8, 1.2));
  col = mix(col, MP_WHITE, textFill(next, uv + vec2(0.02 * (1.0 - eOut(ramp(0.9, 1.6))), 0.0)) * ramp(0.9, 1.3));
  fragColor = vec4(col, eOut(ramp(0.0, 0.5)));
}

// @card end-06 | Stripe wipe | 5
// @inputs [{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.5,"y":0.4},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":8,"bold":true,"align":"center","x":0.5,"y":0.31}]
// The brand stripes sweep across on a slant and wipe the video away, leaving the icon and what's next.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float slant = p.x + p.y * 0.45;
  float front = mix(-1.5, 1.9, eInOut(ramp(0.0, 1.1)));
  float band = step(front - 0.55, slant) * step(slant, front);
  float behind = step(slant, front - 0.55);
  vec4 col = vec4(MP_INK, behind);
  vec4 lg = mpLogo(p, vec2(0.0, 0.14), 0.1, 0.0, 1.0);
  col = over(vec4(lg.rgb, lg.a * behind * ramp(0.8, 1.2)), col);
  col = over(vec4(MP_CYAN, textFill(kicker, uv) * behind * ramp(1.0, 1.4)), col);
  col = over(vec4(MP_WHITE, textFill(next, uv) * behind * ramp(1.1, 1.5)), col);
  float s = fract(slant * 9.0);
  vec3 stripe = s < 0.45 ? MP_BLUE : (s < 0.9 ? MP_CYAN : MP_WHITE);
  col = over(vec4(stripe, band), col);
  fragColor = col;
}

// @card end-07 | Drafting table | 5
// @inputs [{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"left","x":0.1,"y":0.6},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":11,"bold":true,"align":"left","x":0.1,"y":0.48}]
// A drafting grid fades in over the video; the next title is outlined, then inked.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  vec3 col = MP_INK;
  vec2 g = abs(fract(p * 16.0 + 0.5) - 0.5) / 16.0;
  vec2 g4 = abs(fract(p * 4.0 + 0.5) - 0.5) / 4.0;
  float grid = (1.0 - smoothstep(0.0, px() * 1.2, min(g.x, g.y))) * 0.10 + (1.0 - smoothstep(0.0, px() * 1.4, min(g4.x, g4.y))) * 0.18;
  col = mix(col, MP_BLUE, grid);
  float d = textDistance(next, uv);
  float trace = step(uv.x, mix(nextBox.x - 0.01, nextBox.z + 0.01, eInOut(ramp(0.6, 1.6))));
  col = mix(col, MP_CYAN, (1.0 - smoothstep(0.0012, 0.0028, abs(d))) * trace);
  col = mix(col, MP_WHITE, textFill(next, uv) * ramp(1.6, 2.0));
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(0.5, 0.9));
  vec4 lg = mpLogo(p, vec2(0.76, -0.38), 0.045, 0.0, 1.0);
  col = mix(col, lg.rgb, lg.a * ramp(1.2, 1.6));
  fragColor = vec4(col, eOut(ramp(0.0, 0.6)));
}

// @card end-08 | Drifting halves | 5
// @inputs [{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.6,"bold":true,"align":"center","x":0.5,"y":0.6},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":10,"bold":true,"align":"center","x":0.5,"y":0.49}]
// Half discs in blue and cyan float slowly upward behind the next video's name.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float t = mpT();
  vec3 col = mix(MP_DEEP, MP_INK, uv.y);
  for (int layer = 0; layer < 2; layer++) {
    float L = float(layer);
    float cell = 0.3 - L * 0.1;
    vec2 q = p + vec2(L * 0.37, -t * (0.035 + L * 0.03));
    vec2 id = floor(q / cell);
    float h = hash12(id + L * 19.0);
    if (h < 0.62) continue;
    vec2 cc = (id + 0.5 + (vec2(hash12(id + 3.1), hash12(id + 7.7)) - 0.5) * 0.3) * cell;
    float r = cell * (0.22 + 0.12 * hash12(id + 1.3));
    cc.x += r * (step(0.78, h) * 2.0 - 1.0) * 0.6;   // each half sits over its cell's middle
    vec2 lq = (q - cc) / r;
    float leftHalf = step(0.78, h);
    float sd = leftHalf > 0.5 ? sdBlue(lq) : sdCyan(lq);
    vec3 tint = leftHalf > 0.5 ? MP_BLUE : MP_CYAN;
    col = mix(col, tint, cover(sd, px() / r) * (0.18 + L * 0.12));
  }
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(0.6, 1.0));
  col = mix(col, MP_WHITE, textFill(next, uv) * ramp(0.7, 1.2));
  vec4 lg = mpLogo(p, vec2(0.0, -0.27), 0.045, 0.0, 1.0);
  col = mix(col, lg.rgb, lg.a * ramp(1.0, 1.4));
  fragColor = vec4(col, eOut(ramp(0.0, 0.7)));
}

// @card end-09 | Was and next | 5
// @inputs [{"name":"was","type":"text","value":"You watched","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.25,"y":0.56},{"name":"title","type":"text","value":"Branches","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.25,"y":0.47},{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.4,"bold":true,"align":"center","x":0.75,"y":0.56},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.75,"y":0.47}]
// The frame splits in two: the video you watched on the blue side, the next one on the cyan side, the icon on the seam.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  float kl = eInOut(ramp(0.0, 0.8)), kr = eInOut(ramp(0.15, 0.95));
  float left = step(uv.x, 0.5) * step(1.0 - kl, uv.y);
  float right = step(0.5, uv.x) * step(uv.y, kr);
  vec4 col = vec4(0.0);
  col = over(vec4(MP_BLUE, left), col);
  col = over(vec4(MP_CYAN, right), col);
  col = over(vec4(MP_INK * 1.2, textFill(was, uv) * left * ramp(0.8, 1.2) * 0.85), col);
  col = over(vec4(MP_INK, textFill(title, uv) * left * ramp(0.85, 1.25)), col);
  col = over(vec4(MP_INK * 1.2, textFill(kicker, uv) * right * ramp(1.0, 1.4) * 0.85), col);
  col = over(vec4(MP_INK, textFill(next, uv) * right * ramp(1.05, 1.45)), col);
  float s = eBack(ramp(0.9, 1.35));
  vec2 c = vec2(0.0, 0.0);
  float disc = cover(length(p - c) - 0.1 * s, px());
  col = over(vec4(MP_INK, disc * step(0.9, mpT())), col);
  vec4 lg = mpLogo(p, c, 0.075 * max(s, 1e-3), 0.0, 1.0);
  col = over(vec4(lg.rgb, lg.a * step(0.9, mpT())), col);
  fragColor = col;
}

// @card end-10 | Quiet close | 5
// @inputs [{"name":"kicker","type":"text","value":"Up next","fontId":"space-grotesk","sizePct":2.2,"bold":true,"align":"center","x":0.5,"y":0.42},{"name":"next","type":"text","value":"Drafts","fontId":"space-grotesk","sizePct":5,"bold":true,"align":"center","x":0.5,"y":0.35}]
// A calm fade to the icon and one line of type; a thin rule draws out under it; then all goes dark.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / uResolution, p = mpP(fragCoord);
  vec3 col = mix(MP_INK, MP_DEEP, length(p) * 1.2);
  vec4 lg = mpLogo(p, vec2(0.0, 0.13), 0.07, 0.0, 1.0);
  col = mix(col, lg.rgb, lg.a * ramp(0.5, 1.1));
  col = mix(col, MP_CYAN, textFill(kicker, uv) * ramp(0.9, 1.3));
  col = mix(col, MP_WHITE, textFill(next, uv) * ramp(1.0, 1.5));
  float ruleW = 0.09 * eInOut(ramp(1.3, 2.1));
  float rule = step(abs(uv.y - (nextBox.y - 0.03)), 0.0016) * step(abs(uv.x - 0.5), ruleW);
  col = mix(col, MP_CYAN, rule);
  col *= 1.0 - ramp(uDuration - 0.9, uDuration);
  fragColor = vec4(col, eOut(ramp(0.0, 0.6)));
}

// @card end-11 | Logo and link | 5
// @inputs [{"name":"wordmark","type":"text","value":"MultiParity","fontId":"space-grotesk","sizePct":6.5,"bold":true,"align":"center","x":0.5,"y":0.37},{"name":"cta","type":"text","value":"Get it on GitHub","fontId":"space-grotesk","sizePct":2.6,"bold":true,"align":"center","x":0.5,"y":0.265},{"name":"url","type":"text","value":"vltmedia.github.io/MultiParity","fontId":"space-grotesk","sizePct":2.3,"bold":false,"align":"center","x":0.5,"y":0.19}]
// The Construction logo card as the ending: the video goes dark, the icon is drafted and inked, then where to get it.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 p = mpP(fragCoord), uv = fragCoord / uResolution;
  float aspect = uResolution.x / uResolution.y;
  vec3 bg = MP_DEEP * 0.75;
  vec2 g = abs(fract(p * 20.0 + 0.5) - 0.5) / 20.0;
  bg = mix(bg, MP_BLUE, (1.0 - smoothstep(0.0, px() * 1.2, min(g.x, g.y))) * 0.05);
  vec2 c = vec2(0.0, 0.13); float r = 0.13;
  vec2 q = (p - c) / r;
  float lw = 1.3 * px() / r;
  float ang = (atan(q.y, q.x) + 3.14159) / 6.28318;
  float dash = step(0.5, fract(ang * 48.0));
  vec3 col = bg;
  float c1 = (1.0 - smoothstep(0.0, lw, abs(sdDisc(q)))) * step(ang, eOut(ramp(0.4, 0.95)));
  float v = (1.0 - smoothstep(0.0, lw, abs(abs(q.x) - 0.278))) * step(abs(q.y), 1.25 * eOut(ramp(0.6, 1.0)));
  float a1 = length(q - vec2(-0.637, 0.0)), a2 = length(q - vec2(0.637, 0.0));
  float arcs = (1.0 - smoothstep(0.0, lw, min(abs(a1 - 0.634), abs(a2 - 0.634)))) * ramp(0.85, 1.15) * dash;
  float cross_ = (1.0 - smoothstep(0.0, lw, min(abs(q.x), abs(q.y)))) * step(length(q), 1.35) * ramp(0.45, 0.75) * 0.5;
  col = mix(col, MP_CYAN, max(max(c1, v), max(arcs, cross_)) * (1.0 - 0.6 * ramp(1.5, 1.9)));
  vec4 lg = mpLogo(p, c, r, 0.0, 1.0);
  col = mix(col, lg.rgb, lg.a * step(q.x, mix(-1.1, 1.1, eInOut(ramp(1.2, 1.7)))));
  col = mix(col, MP_WHITE, textFill(wordmark, uv) * ramp(1.6, 2.0));
  // "Get it on GitHub" in a cyan tag, the address under it.
  vec2 k0 = ctaBox.xy - vec2(0.014, 0.02), k1 = ctaBox.zw + vec2(0.014, 0.02);
  vec2 kc = (k0 + k1) * 0.5, kh = (k1 - k0) * 0.5;
  float tagA = eOut(ramp(2.0, 2.4));
  float pill = cover(sdRBox((uv - kc) * vec2(aspect, 1.0), kh * vec2(aspect, 1.0) * vec2(mix(0.6, 1.0, tagA), 1.0), 0.03), px()) * tagA;
  col = mix(col, MP_CYAN, pill);
  col = mix(col, MP_DEEP, textFill(cta, uv) * pill);
  col = mix(col, MP_WHITE * 0.8, textFill(url, uv) * ramp(2.3, 2.7));
  col *= 1.0 - ramp(uDuration - 0.6, uDuration);
  fragColor = vec4(col, eOut(ramp(0.0, 0.6)));
}
