#define NFLOATS 6
#define MAX_K 64

#define DM_NOISE 0
#define DM_COMPLEX 1
#define DM_STATE 2

#define AM_STATIC 0
#define AM_GAUSS 1
#define AM_RANGLE 2
#define AM_RADIAL 3

#define FM_STATIC 0
#define FM_GAUSS 1

#define IM_ANISOTROPIC 0
#define IM_GAUSS 1
#define IM_ISOTROPIC 2
#define IM_RAMP 3

#define CM_CLAMP 0
#define CM_MOD 1

#define OM_OPTIMIZE 0
#define OM_AVERAGE 1
#define OM_HYBRID 2

#define M_PI 3.14159265358979323846
#define M_2PI (2.0 * M_PI)
#define M_PI2 (M_PI * M_PI)

struct Kernel {
#ifdef TINYGL
    vec2 pos;
#else
    float x;
    float y;
#endif
    float frequency;
    float phase;
    float angle;
    float state;
};

#ifdef TINYGL
#define K int(u_KernelCount)

layout(location = 0) uniform ivec3 u_Grid;
layout(location = 1) uniform int u_CellMode;
layout(location = 2) uniform uint u_KernelCount;
layout(location = 3, binding = 0, r32f) coherent uniform imageBuffer u_Kernels;

vec3 gaussian(vec2 x, float b) {
    float a = exp(-M_PI * (b * b) * ((x.x * x.x) + (x.y * x.y)));
    vec2 d = -2. * M_PI * b * b * x;
    // Gaussian value, X derivative, Y derivative
    return a * vec3(1., d.x, d.y);
}

Kernel invalid_kernel() { return Kernel(vec2(-10.0), 0., 0., 0., 0.); }

Kernel load_at_idx(int idx, vec2 pos_offset) {
    idx *= NFLOATS;

    return Kernel(
        pos_offset + vec2(
            imageLoad(u_Kernels, idx + 0).x,
            imageLoad(u_Kernels, idx + 1).x
        ),
        imageLoad(u_Kernels, idx + 2).x,
        imageLoad(u_Kernels, idx + 3).x,
        imageLoad(u_Kernels, idx + 4).x,
        imageLoad(u_Kernels, idx + 5).x
    );
}

void save_phase_at_idx(int idx, float phase) {
    imageStore(u_Kernels, idx * NFLOATS + 3, vec4(phase));
}

void save_at_idx(int idx, Kernel k) {
    idx *= NFLOATS;

    imageStore(u_Kernels, idx + 0, vec4(k.pos.x));
    imageStore(u_Kernels, idx + 1, vec4(k.pos.y));
    imageStore(u_Kernels, idx + 2, vec4(k.frequency));
    imageStore(u_Kernels, idx + 3, vec4(k.phase));
    imageStore(u_Kernels, idx + 4, vec4(k.angle));
    imageStore(u_Kernels, idx + 5, vec4(k.state));
}
#endif
