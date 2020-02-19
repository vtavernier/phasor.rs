// 0 = static angle for the whole image
// 1 = gaussian orientation map
layout(location = 20) uniform int u_AngleMode;
// Rotation of the angle parameter (value when u_AngleMode == AM_STATIC)
layout(location = 21) uniform float u_AngleOffset;
// Bandwidth of the Gaussian orientation field
layout(location = 22) uniform float u_AngleBandwidth;
// Range of Gaussian perturbations of the orientation map [0, pi] (0 =>
// AM_STATIC, pi => full range)
layout(location = 23) uniform float u_AngleRange;

// 0 = static frequency for the whole image
// 1 = gaussian frequency map (profile is a ramp between minimum and maximum
// frequency)
layout(location = 24) uniform int u_FrequencyMode;
// Minimum frequency
layout(location = 25) uniform float u_MinFrequency;
// Maximum frequency
layout(location = 26) uniform float u_MaxFrequency;
// Bandwidth of the Gaussian frequency field
layout(location = 27) uniform float u_FrequencyBandwidth;

// 0 = anisotropic (isotropy = u_MinIsotropy)
// 1 = gaussian isotropy map (profile is a ramp from u_MinIsotropy to
// u_MaxIsotropy) 2 = isotropic (isotropy = u_MaxIsotropy) 3 = ramp in x
// coordinate from min to max
layout(location = 28) uniform int u_IsotropyMode;
// Minimum isotropy
layout(location = 29) uniform float u_MinIsotropy;
// Maximum isotropy [0, 1]
layout(location = 30) uniform float u_MaxIsotropy;
// Bandwidth of the Gaussian isotropy field
layout(location = 31) uniform float u_IsotropyBandwidth;
// Exponent of the power profile for isotropy (if gaussian or ramp isotropy
// field)
layout(location = 32) uniform float u_IsotropyPower;

layout(location = 33) uniform int u_GlobalSeed;

///////////////////////////////////////////////
// prng
///////////////////////////////////////////////
uint x_;
uint N = 15487469u;  // seed max value should be a prime number;
uint hash(uint x) {
    x = ((x >> 16) ^ x) * 0x45d9f3bu;
    x = ((x >> 16) ^ x) * 0x45d9f3bu;
    x = ((x >> 16) ^ x);
    return x;
}
void seed(uint s) { x_ = hash(s) % N; }
uint next() {
    x_ *= 3039177861u;
    x_ = x_ % N;
    return x_;
}
float uni_0_1() { return float(next()) / float(N); }
float uni(float min, float max) { return min + (uni_0_1() * (max - min)); }
int poisson(float mean) {
    float g_ = exp(-mean);
    int em = 0;
    double t = uni_0_1();
    while (t > g_) {
        ++em;
        t *= uni_0_1();
    }
    return em;
}

uint morton(uint x, uint y) {
    uint z = 0;
    for (int i = 0; i < 32 * 4; i++) {
        z |= ((x & (1 << i)) << i) | ((y & (1 << i)) << (i + 1));
    }
    return z;
}

/// gaussian orientation field
int _impPerKernel = 20;
vec3 gaussian(vec2 x, float b) {
    float a = exp(-M_PI * (b * b) * ((x.x * x.x) + (x.y * x.y)));
    vec2 d = -2. * M_PI * b * b * x;
    // Gaussian value, X derivative, Y derivative
    return a * vec3(1., d.x, d.y);
}

vec2 cell(ivec2 ij, vec2 uv, float b, uint gseed, float cellsz,
          out vec4 dnoise) {
    uint s = morton(ij.x, ij.y) + 333;
    s = s == 0 ? 1 : s + gseed;
    seed(s);
    int impulse = 0;
    int nImpulse = _impPerKernel;
    vec2 noise = vec2(0.0);
    while (impulse <= nImpulse) {
        vec2 impulse_centre = vec2(uni_0_1(), uni_0_1());
        vec2 d = (uv - impulse_centre) * cellsz;
        float omega = uni(-2.4, 2.4);
        vec2 r = vec2(cos(omega), sin(omega));
        vec3 g = gaussian(d, b);
        noise += g.x * r;
        dnoise += vec4(g.y * r, g.z * r);
        impulse++;
    }
    return noise;
}

vec3 eval_noise(vec2 uv, float b, uint gseed) {
    float kr = sqrt(-log(0.05) / M_PI) / b;
    float cellsz = 2.0 * kr;
    vec2 _ij = uv / cellsz;
    ivec2 ij = ivec2(_ij);
    vec2 fij = _ij - vec2(ij);
    vec2 noise = vec2(0.0);   // Complex noise value
    vec4 dnoise = vec4(0.0);  // Complex noise derivatives (ReX, ImX, ReY, ImY)
    for (int j = -2; j <= 2; j++) {
        for (int i = -2; i <= 2; i++) {
            ivec2 nij = ivec2(i, j);
            noise += cell(ij + nij, fij - vec2(nij), b, gseed, cellsz, dnoise);
        }
    }

    // Squared norm of complex norms gradient. Good enough approximation of
    // the angle gradient norm but with an analytical expression.
    vec2 d = vec2(dot(dnoise.xy, dnoise.xy), dot(dnoise.zw, dnoise.zw));

    return vec3(noise.xy, dot(d, d) / dot(noise.xy, noise.xy));
}

float frequency(vec2 x) {
    if (u_FrequencyMode == FM_STATIC) {
        return u_MinFrequency;
    } else /* if (u_FrequencyMode == FM_GAUSS) */ {
        vec3 q = eval_noise(x, u_FrequencyBandwidth, u_GlobalSeed + 10);
        return u_MinFrequency + (u_MaxFrequency - u_MinFrequency) *
                                    (.5 + .5 * sin(atan(q.y, q.x)));
    }
}

float isotropy(vec2 x) {
    if (u_IsotropyMode == IM_ANISOTROPIC) {
        return u_MinIsotropy;
    } else if (u_IsotropyMode == IM_ISOTROPIC) {
        return u_MaxIsotropy;
    } else if (u_IsotropyMode == IM_RAMP) {
        return clamp(u_MinIsotropy + (u_MaxIsotropy - u_MinIsotropy) *
                                         pow(x.x / 32.0, u_IsotropyPower),
                     0., 1.);
    } else /* if (u_FrequencyMode == IM_GAUSS) */ {
        vec3 q = eval_noise(x, u_IsotropyBandwidth, u_GlobalSeed + 15);
        return u_MinIsotropy +
               (u_MaxIsotropy - u_MinIsotropy) *
                   pow(.5 + .5 * sin(atan(q.y, q.x)), u_IsotropyPower);
    }
}

vec2 angle(vec2 x) {
    if (u_AngleMode == AM_STATIC) {
        return vec2(u_AngleOffset, 0.);
    } else if (u_AngleMode == AM_RANGLE) {
        float d = M_PI / 2.0 * pow(abs(x.x - x.y) / 32.0, 2.0);

        if (x.x > x.y) {
            return vec2(u_AngleOffset, d);
        } else {
            return vec2(u_AngleOffset + M_PI / 2.0, d);
        }
    } else if (u_AngleMode == AM_RADIAL) {
        vec2 u = x / vec2(32.0);
        return vec2(atan(2. * u.y, 2. * u.x - 1.), 0.);
    } else /* if (u_AngleMode == AM_GAUSS) */ {
        vec3 q = eval_noise(x, u_AngleBandwidth, u_GlobalSeed + 5);
        return vec2(u_AngleRange / M_PI * atan(q.y, q.x) + u_AngleOffset, q.z);
    }
}

// Converts an unsigned int to a float in [0,1]
float tofloat(uint u) {
    // Slower, but generates all dyadic rationals of the form k / 2^-24 equally
    return float(u >> 8) * (1. / float(1u << 24));

    // Faster, but only generates all dyadic rationals of the form k / 2^-23
    // equally return uintBitsToFloat(0x7Fu << 23 | u >> 9) - 1.;
}

float kernelangle(vec2 x, uint kernelId) {
    float is = isotropy(x);
    float a = angle(x).x;

    if (is > 0.) {
        // Hash the kernel ID into a random number
        float rn = 2. * (tofloat(hash(kernelId + u_GlobalSeed))) - 1.;
        return a + rn * (M_PI * is);  // is == 1 => -Pi, Pi orientation random
    } else {
        return a;
    }
}

// vim: ft=glsl:fdm=marker:ts=4:sw=4:et
