#version 460 core
#extension GL_ARB_shader_image_load_store : enable

#include "shared.h"

#define PREFILTERED
#include "gabor.glsl"

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 o_PixColor;
layout(location = 1) out vec4 o_PixExtra;

layout(location = 4) uniform int u_DisplayMode;

#include "fields.glsl"

layout(location = 6) uniform float u_FilterModulation;
layout(location = 7) uniform float u_FilterModPower;
layout(location = 8) uniform float u_IsotropyModulation;

void main() {
    vec2 gij = vec2(vec2(u_Grid.xy) * uv);
    vec2 gs = 32.0 / vec2(u_Grid.xy);

    int gi = int(gij.x);
    int gj = int(gij.y);

    vec2 kv = vec2(0.0);

    // Reference orientation at current pixel
    vec2 o = angle(gij * gs);
    vec2 w = vec2(cos(o.x), sin(o.x));
    // Reference isotropy
    float is = isotropy(gij * gs);
    // Filter modulation parameter (1. -> oscillator+gaussian, 0. ->
    // gaussian)
    float fm = smoothstep(
        0., 1., exp(-pow(o.y * u_FilterModulation + is * u_IsotropyModulation, u_FilterModPower)));

    // Reference frequency at current pixel
    float f = frequency(gij * gs);
    int cm = cell_margin();

    for (int nj = gj - cm; nj <= gj + cm; nj++) {
        for (int ni = gi - cm; ni <= gi + cm; ni++) {
            int ci = cell_idx(ni);
            int cj = cell_idx(nj);
            if (ci == -1 || cj == -1)
                continue;

            for (int k = 0; k < K; k++) {
                // fetch kernel
                int idx = (ci + cj * u_Grid.x) * K + k;
                Kernel n = load_at_idx(idx, vec2(ni, nj));
                n.frequency *= gs.x;
                // evaluate
                kv += phasor(gij - n.pos, n.phase, vec2(cos(n.angle), sin(n.angle)), n.frequency,
                             w, f, fm);
            }
        }
    }

    float ph = atan(kv.x, kv.y);
    float I = 0.5 * length(kv);

    if (u_DisplayMode == DM_NOISE) {
        o_PixColor = vec4(0.5 + 0.5 * sin(ph));
        o_PixColor = vec4(vec3(mod(ph + M_PI, M_2PI) / M_2PI), 1.0);
    } else if (u_DisplayMode == DM_COMPLEX) {
        // Complex conjugate
        o_PixColor = vec4(kv, atan(-w.y, w.x), f);
        o_PixExtra = vec4(is, fm, 0., 0.);
    } else if (u_DisplayMode == DM_STATE) {
        float state = 0.0;
        for (int k = 0; k < K; k++) {
            uint idx = ((gi + gj * u_Grid.x) * K + k) * NFLOATS;
            state = max(state, imageLoad(u_Kernels, int(idx) + 5).x);
        }

        o_PixColor = vec4(I) + 0.0 * vec4(state, 0.0, 0.0, 0.0);
    } else {
        o_PixColor = vec4(1.0, 0.0, 1.0, 1.0);
    }

    // o_PixColor = vec4( fract(ph / (2.0*M_PI)) );
    // o_PixColor = vec4( I * 0.5 );
}
