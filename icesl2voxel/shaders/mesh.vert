#version 460 core

layout(location = 0) in vec3 vtxPosition;
layout(location = 0) uniform mat4 viewMatrix;

void main() {
    gl_Position = viewMatrix * vec4(vtxPosition, 1.);
}
