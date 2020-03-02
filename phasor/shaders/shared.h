#define NFLOATS 6u
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

#define M_PI 3.14159265358979323846
#define M_2PI (2.0 * M_PI)
#define M_PI2 (M_PI * M_PI)

struct Kernel {
    float x;
    float y;
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
#endif
