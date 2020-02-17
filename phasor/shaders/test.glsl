layout(location = 0) uniform float uAlpha;
layout(location = 1) uniform ivec3 uStuff;
layout(location = 2) uniform ivec3 uAry[3];

vec2 q() {
    return vec2(uStuff.x, uAry[2].x);
}
