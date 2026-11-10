#version 420 core
#extension GL_ARB_compute_shader : require
#extension GL_OES_shader_image_atomic : require
layout(local_size_x = 16, local_size_y = 1, local_size_z = 1) in;

layout(r32ui) uniform uimage2D _group_0_binding_0_cs;

layout(r32i) uniform iimage2D _group_0_binding_1_cs;

layout(r32ui) uniform uimage2D _group_0_binding_2_cs;

uint gen_gl_LocalInvocationIndex_1 = 0u;


void main_1() {
    uint slot = 0u;
    int lowest = 0;
    uint mask = 0u;
    uint _e6 = imageAtomicAdd(_group_0_binding_0_cs, ivec2(0), 1u);
    slot = _e6;
    uint _e11 = gen_gl_LocalInvocationIndex_1;
    int _e13 = imageAtomicMin(_group_0_binding_1_cs, ivec2(0), int(_e11));
    lowest = _e13;
    uint _e19 = slot;
    uint _e21 = imageAtomicOr(_group_0_binding_0_cs, ivec2(1, 0), (1u << _e19));
    uint _e26 = imageAtomicAnd(_group_0_binding_0_cs, ivec2(2, 0), 4294967295u);
    uint _e31 = slot;
    uint _e32 = imageAtomicXor(_group_0_binding_0_cs, ivec2(3, 0), _e31);
    mask = ((_e21 & _e26) ^ _e32);
    int _e38 = lowest;
    int _e39 = imageAtomicMax(_group_0_binding_1_cs, ivec2(1, 0), _e38);
    uint _e40 = slot;
    uint _e44 = mask;
    int _e45 = lowest;
    imageStore(_group_0_binding_2_cs, ivec2(int(_e40), 0), uvec4((_e44 + uint(_e45))));
    return;
}

void main() {
    uint gen_gl_LocalInvocationIndex = gl_LocalInvocationIndex;
    gen_gl_LocalInvocationIndex_1 = gen_gl_LocalInvocationIndex;
    main_1();
    return;
}

