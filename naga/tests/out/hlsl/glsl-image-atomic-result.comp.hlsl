struct NagaConstants {
    int first_vertex;
    int first_instance;
    uint other;
};
ConstantBuffer<NagaConstants> _NagaConstants: register(b0, space1);

RWTexture2D<uint> counter : register(u0);
RWTexture2D<int> extremes : register(u1);
RWTexture2D<uint> slots : register(u2);
static uint gl_LocalInvocationIndex_1 = (uint)0;

void main_1()
{
    uint slot = (uint)0;
    int lowest = (int)0;
    uint mask = (uint)0;

    uint _e6; InterlockedAdd(counter[(int(0)).xx],1u, _e6);
    slot = _e6;
    uint _e11 = gl_LocalInvocationIndex_1;
    int _e13; InterlockedMin(extremes[(int(0)).xx],int(_e11), _e13);
    lowest = _e13;
    uint _e19 = slot;
    uint _e21; InterlockedOr(counter[int2(int(1), int(0))],(1u << _e19), _e21);
    uint _e26; InterlockedAnd(counter[int2(int(2), int(0))],4294967295u, _e26);
    uint _e31 = slot;
    uint _e32; InterlockedXor(counter[int2(int(3), int(0))],_e31, _e32);
    mask = ((_e21 & _e26) ^ _e32);
    int _e38 = lowest;
    int _e39; InterlockedMax(extremes[int2(int(1), int(0))],_e38, _e39);
    uint _e40 = slot;
    uint _e44 = mask;
    int _e45 = lowest;
    slots[int2(int(_e40), int(0))] = ((_e44 + uint(_e45))).xxxx;
    return;
}

[numthreads(16, 1, 1)]
void main(uint gl_LocalInvocationIndex : SV_GroupIndex)
{
    gl_LocalInvocationIndex_1 = gl_LocalInvocationIndex;
    main_1();
    return;
}
