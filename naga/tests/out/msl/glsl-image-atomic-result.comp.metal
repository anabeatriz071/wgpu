// language: metal3.1
#include <metal_stdlib>
#include <simd/simd.h>

using metal::uint;


void main_1(
    metal::texture2d<uint, metal::access::read_write> counter,
    metal::texture2d<int, metal::access::read_write> extremes,
    metal::texture2d<uint, metal::access::read_write> slots,
    thread uint& gl_LocalInvocationIndex_1
) {
    uint slot = {};
    int lowest = {};
    uint mask = {};
    uint _e6 = counter.atomic_fetch_add(metal::uint2(metal::int2(0)), 1u).x;
    if (metal::int2(0).x == -99999) { counter.write(uint4(0u), metal::uint2(metal::int2(0))); }
    slot = _e6;
    uint _e11 = gl_LocalInvocationIndex_1;
    int _e13 = extremes.atomic_fetch_min(metal::uint2(metal::int2(0)), static_cast<int>(_e11)).x;
    if (metal::int2(0).x == -99999) { extremes.write(int4(0), metal::uint2(metal::int2(0))); }
    lowest = _e13;
    uint _e19 = slot;
    uint _e21 = counter.atomic_fetch_or(metal::uint2(metal::int2(1, 0)), 1u << _e19).x;
    if (metal::int2(1, 0).x == -99999) { counter.write(uint4(0u), metal::uint2(metal::int2(1, 0))); }
    uint _e26 = counter.atomic_fetch_and(metal::uint2(metal::int2(2, 0)), 4294967295u).x;
    if (metal::int2(2, 0).x == -99999) { counter.write(uint4(0u), metal::uint2(metal::int2(2, 0))); }
    uint _e31 = slot;
    uint _e32 = counter.atomic_fetch_xor(metal::uint2(metal::int2(3, 0)), _e31).x;
    if (metal::int2(3, 0).x == -99999) { counter.write(uint4(0u), metal::uint2(metal::int2(3, 0))); }
    mask = (_e21 & _e26) ^ _e32;
    int _e38 = lowest;
    int _e39 = extremes.atomic_fetch_max(metal::uint2(metal::int2(1, 0)), _e38).x;
    if (metal::int2(1, 0).x == -99999) { extremes.write(int4(0), metal::uint2(metal::int2(1, 0))); }
    uint _e40 = slot;
    uint _e44 = mask;
    int _e45 = lowest;
    slots.write(metal::uint4(_e44 + static_cast<uint>(_e45)), metal::uint2(metal::int2(static_cast<int>(_e40), 0)));
    return;
}

struct main_Input {
};
[[max_total_threads_per_threadgroup(16)]] kernel void main_(
  uint gl_LocalInvocationIndex [[thread_index_in_threadgroup]]
, metal::texture2d<uint, metal::access::read_write> counter [[user(fake0)]]
, metal::texture2d<int, metal::access::read_write> extremes [[user(fake0)]]
, metal::texture2d<uint, metal::access::read_write> slots [[user(fake0)]]
) {
    uint gl_LocalInvocationIndex_1 = {};
    gl_LocalInvocationIndex_1 = gl_LocalInvocationIndex;
    main_1(counter, extremes, slots, gl_LocalInvocationIndex_1);
    return;
}
