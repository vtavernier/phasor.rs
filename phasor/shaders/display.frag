#version 460
#extension GL_ARB_shader_image_load_store : enable
#extension GL_ARB_compute_variable_group_size : enable

#define K u_KernelCount
#include "shared.h"

#define M_PI 3.14159265358979323846
#define M_PI2 (M_PI * M_PI)

layout(r32f) coherent uniform imageBuffer u_Kernels;

layout(location = 0) uniform int u_GridX;
layout(location = 1) uniform int u_GridY;
layout(location = 2) uniform int u_ScreenX;
layout(location = 3) uniform int u_ScreenY;
layout(location = 4) uniform int u_DisplayMode;
layout(location = 5) uniform int u_KernelCount;

#define PREFILTERED
#include "gabor.glsl"

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 o_PixColor;
layout(location = 1) out vec4 o_PixExtra;

#include "fields.glsl"

layout(location = 6) uniform float u_FilterModulation;
layout(location = 7) uniform float u_FilterModPower;
layout(location = 8) uniform float u_IsotropyModulation;

void main() {
    if (gl_SampleMaskIn[0] == 0) {
        // do nothing

    } else {
        vec2 gij = vec2(vec2(u_GridX, u_GridY) * uv);
        vec2 gs = 32.0 / vec2(u_GridX, u_GridY);

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
            0., 1.,
            exp(-pow(o.y * u_FilterModulation + is * u_IsotropyModulation,
                     u_FilterModPower)));

        // Reference frequency at current pixel
        float f = frequency(gij * gs);
        int cm = cell_margin();

        for (int nj = gj - cm; nj <= gj + cm; nj++) {
            for (int ni = gi - cm; ni <= gi + cm; ni++) {
                int ci = cell_idx(ni);
                int cj = cell_idx(nj);
                if (ci == -1 || cj == -1) continue;

                for (int k = 0; k < K; k++) {
                    // fetch kernel
                    vec2 npos;
                    float nphase, nangle, nfrequ;
                    int idx = ((ci + cj * u_GridX) * K + k) * NFLOATS;
                    npos.x = float(ni) + imageLoad(u_Kernels, int(idx) + 0).x;
                    npos.y = float(nj) + imageLoad(u_Kernels, int(idx) + 1).x;
                    nfrequ = imageLoad(u_Kernels, int(idx) + 2).x * gs.x;
                    nphase = imageLoad(u_Kernels, int(idx) + 3).x;
                    nangle = imageLoad(u_Kernels, int(idx) + 4).x;
                    // evaluate
                    kv += phasor(gij - npos, nphase,
                                 vec2(cos(nangle), sin(nangle)), nfrequ, w, f,
                                 fm);
                }
            }
        }

        float ph = atan(kv.x, kv.y);
        float I = 0.5 * length(kv);

        if (u_DisplayMode == DM_NOISE) {
            o_PixColor = vec4(0.5 + 0.5 * sin(ph));
        } else if (u_DisplayMode == DM_COMPLEX) {
            // Complex conjugate
            o_PixColor = vec4(kv, atan(-w.y, w.x), f);
            o_PixExtra = vec4(is, fm, 0., 0.);
        } else if (u_DisplayMode == DM_STATE) {
            float state = 0.0;
            for (int k = 0; k < K; k++) {
                int idx = ((gi + gj * u_GridX) * K + k) * NFLOATS;
                state = max(state, imageLoad(u_Kernels, int(idx) + 5).x);
            }

            o_PixColor = vec4(I) + 0.0 * vec4(state, 0.0, 0.0, 0.0);
        } else {
            o_PixColor = vec4(1.0, 0.0, 1.0, 1.0);
        }

        // o_PixColor = vec4( fract(ph / (2.0*M_PI)) );
        // o_PixColor = vec4( I * 0.5 );
    }
}

// vim: ft=glsl:fdm=marker:ts=4:sw=4:et
