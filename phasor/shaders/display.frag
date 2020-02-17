#version 460

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 fragColor;

#include "test.glsl"

void main() {
    fragColor = vec4(length(uv), q(), 1.0 - uAlpha);
}
