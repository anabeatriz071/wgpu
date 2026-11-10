#version 460
layout(local_size_x = 16, local_size_y = 1, local_size_z = 1) in;

layout(set = 0, binding = 0, r32ui) uniform uimage2D _8;
layout(set = 0, binding = 1, r32i) uniform iimage2D _10;
layout(set = 0, binding = 2, r32ui) uniform writeonly uimage2D _12;

uint _13 = 0u;

void _17()
{
    uint _32 = 0u;
    int _35 = 0;
    uint _38 = 0u;
    uint _44 = imageAtomicAdd(_8, ivec2(0), 1u);
    _32 = _44;
    int _49 = imageAtomicMin(_10, ivec2(0), int(_13));
    _35 = _49;
    uint _53 = imageAtomicOr(_8, ivec2(1, 0), 1u << _32);
    uint _55 = imageAtomicAnd(_8, ivec2(2, 0), 4294967295u);
    uint _59 = imageAtomicXor(_8, ivec2(3, 0), _32);
    _38 = (_53 & _55) ^ _59;
    int _63 = imageAtomicMax(_10, ivec2(1, 0), _35);
    imageStore(_12, ivec2(int(_32), 0), uvec4(_38 + uint(_35)));
}

void main()
{
    _13 = gl_LocalInvocationIndex;
    _17();
}

