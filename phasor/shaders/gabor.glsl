// Noise bandwidth
layout(location = 10) uniform float u_NoiseBandwidth;
#ifdef PREFILTERED
layout(location = 11) uniform float u_FilterBandwidth;
#endif

vec2 phasor(vec2 x, float phi, vec2 wi, float fi
#ifdef PREFILTERED
            ,
            vec2 w, float f, float fm
#endif
) {
    float gaus, osc;
    float b = u_NoiseBandwidth * u_NoiseBandwidth * M_PI;

#ifdef PREFILTERED
    if (u_FilterBandwidth > 0.0) {
        // Pre-filtered kernel
        float a = u_FilterBandwidth * u_FilterBandwidth * M_PI;
        vec2 dfw = fi * wi - f * w;
        dfw *= fm;

        gaus = exp(-b / (1. + fm * b / a) * dot(x, x)) *
               exp(-M_PI2 * dot(dfw, dfw) / (a + b));
        osc = 2. * M_PI * dot(x, fi * wi + dfw / (1. + b / a)) + phi;

    } else
#endif
    {
        // Regular kernel
        gaus = exp(-b * dot(x, x));
        osc = 2. * M_PI * dot(x, fi * wi) + phi;
    }

    return gaus * vec2(cos(osc), sin(osc));
}

int cell_margin() {
#ifdef PREFILTERED
    if (u_FilterBandwidth > 0.0) {
        return 1 + int(ceil(sqrt(u_NoiseBandwidth * u_NoiseBandwidth +
                                 u_FilterBandwidth * u_FilterBandwidth) /
                            u_FilterBandwidth));
    }
#endif
    return 3;
}

// 0 = clamp
// 1 = modulo
layout(location = 12) uniform int u_CellMode;

int cell_id(int n, int m) {
    if (u_CellMode == CM_CLAMP) {
        if (n < 0 || n >= m) {
            return -1;
        }

        return n;
    } else if (u_CellMode == CM_MOD) {
        while (n < 0) {
            n += m;
        }

        return n % m;
    } else {
        return n;
    }
}

int cell_idx(int ni) { return cell_id(ni, u_GridX); }

int cell_idy(int nj) { return cell_id(nj, u_GridY); }
