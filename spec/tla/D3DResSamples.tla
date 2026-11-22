---------------------------- MODULE D3DResSamples ----------------------------
(* A multisampled render target's samples through CopyResource and CopySubresourceRegion
   (spec/d3dres.txt D3DRES.COPY.SAMPLES, COPYSUB.SAMPLES), on Direct3D 11 and 10: the triangle of
   d3d11draw probe-rt-ms4-copy drawn on a 16 by 16 R8G8B8A8_UNORM target of 4 samples (slot 0),
   its edge pixels' samples differing; slots 2 and 3 are textures of the same description,
   holding zeros. Then calls: CopyResource or a whole CopySubresourceRegion between two of the
   three, or a ResolveSubresource of one into the single-sampled slot 1 read back. Direct3D 11
   copies the samples with the resource, so a resolve of the copy gives the drawn picture's
   resolve (Picture: Windows' answer to probe-rt-ms4-copy); Direct3D 10 copies no multisampled
   resource and no subresource of one holding samples that differ, so the destination keeps
   what it had. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES dev, hold, log, n
vars == <<dev, hold, log, n>>

MS == {0, 2, 3}
S(i) == ToString(i)

\* the calls that draw the triangle, after the device line (d3d11draw probe-rt-ms4-copy)
Prelude == <<
    "tex 0 2 16 16 1 1 28 0 0x20 0 0 4 0",
    "rtv 0 0 nodesc",
    "clear 0 0 0 0 0",
    "target 0",
    "tex 1 2 16 16 1 1 28 0 0x20 0 0 1 0",
    "tex 2 2 16 16 1 1 28 0 0x20 0 0 4 0",
    "shader 0 vs 445842434841951959ca12a1efa2b53114f126850100000048010000030000002c00000080000000d80000004953474e4c000000020000000800000038000000000000000000000003000000000000000f0f000041000000010000000000000003000000010000000f0f0000544558434f4f524400544558434f4f524400abab4f53474e50000000020000000800000038000000000000000100000003000000000000000f00000044000000000000000000000003000000010000000f00000053565f506f736974696f6e00544558434f4f524400ababab5348445268000000400001001a0000005f000003f2101000000000005f000003f21010000100000067000004f2201000000000000100000065000003f22010000100000036000005f220100000000000461e10000000000036000005f220100001000000461e1000010000003e000001",
    "shader 1 ps 445842432f318ca05382c77d60d51844f0bd4d3d01000000f8000000030000002c00000084000000b80000004953474e50000000020000000800000038000000000000000100000003000000000000000f00000044000000000000000000000003000000010000000f0f000053565f506f736974696f6e00544558434f4f524400ababab4f53474e2c000000010000000800000020000000000000000000000003000000000000000f00000053565f54617267657400abab5348445238000000400000000e00000062100003f21010000100000065000003f22010000000000036000005f220100000000000461e1000010000003e000001",
    "layout 0 2 2",
    "rast 3 1 0 0 0 0 1 0 0 0",
    "viewport 0 0 0x41800000 0x41800000 0 0x3f800000",
    "vtx 0xbf800000 0x3f800000 0x3f000000 0x3f800000 0x3d30b0b1 0x3ca0a0a1 0x3f48c8c9 0x3f800000",
    "vtx 0x3f800000 0x3f300000 0x3f000000 0x3f800000 0x3e40c0c1 0x3ec0c0c1 0x3f7dfdfe 0x3f800000",
    "vtx 0xbf600000 0xbf400000 0x3f000000 0x3f800000 0x3eaaaaab 0x3f3bbbbc 0x3e48c8c9 0x3f800000",
    "draw 4 3 32"
>>

\* what each of them prints
TexWords == "tex = 10 10 1 1 1c 4 0 0 20 0 0"
Printed(l) ==
    CASE l = "tex 0 2 16 16 1 1 28 0 0x20 0 0 4 0" -> <<TexWords>>
      [] l = "tex 1 2 16 16 1 1 28 0 0x20 0 0 1 0" -> <<"tex = 10 10 1 1 1c 1 0 0 20 0 0">>
      [] l = "tex 2 2 16 16 1 1 28 0 0x20 0 0 4 0" -> <<TexWords>>
      [] l = "rtv 0 0 nodesc" -> <<"rtv = 1c 6">>
      [] OTHER -> <<>>

\* the drawn picture resolved (Windows, d3d11draw-32 probe-rt-ms4-copy)
Picture == <<
    "0e0dc5ff0c0d98bf090b68800a0c6a80050736400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "141abaff151ebfff1721c4ff1825c9ff1a29ceff1b2cd3ff1d30d8ff1727a6bf101c7180111e74801220768009113c4000000000000000000000000000000000",
    "131d83bf1a2ab4ff1c2eb9ff1d32beff1f35c3ff2039c8ff223dcdff2341d2ff2544d7ff2648dcff284ce1ff294fe6ff2b53ebff2c57f0ff2344b8bf0c183f40",
    "17267bbf1f37a9ff213baeff223fb3ff2442b8ff2546bdff274ac2ff284dc7ff2a51ccff2b55d1ff2d58d6ff2e5cdbff3060e0ff3163e5ff1a34758000000000",
    "1a3073bf25449eff2648a3ff284ba8ff294fadff2b53b2ff2c56b7ff2e5abcff2f5ec1ff3161c6ff3265cbff3469d0ff356dd5ff1c386d800000000000000000",
    "1e3a6bbf2a5193ff2b5498ff2d589dff2e5ca2ff305fa7ff3163acff3367b1ff346ab6ff366ebbff3772c0ff2b5994bf0f1e3340000000000000000000000000",
    "172d42802f5d88ff30618dff326592ff336897ff356c9cff3670a1ff3874a6ff3977abff3b7bb0ff2d5f88bf0000000000000000000000000000000000000000",
    "1a333c80346a7dff366e82ff377187ff39758cff3a7991ff3c7d96ff3d809bff3f84a0ff20445380000000000000000000000000000000000000000000000000",
    "0e1d1b40397771ff3b7a76ff3c7e7cff3e8281ff3f8686ff41898bff428d90ff1124254000000000000000000000000000000000000000000000000000000000",
    "0f2018403e8466ff40876bff418b70ff438f75ff44927bff357160bf000000000000000000000000000000000000000000000000000000000000000000000000",
    "1123164044905bff459460ff479865ff489b6aff2550388000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "12261340499d50ff4aa155ff4ca45aff132a18400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "000000004eaa45ff3b8238bf142c1440000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "000000002a5b1d800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
>>
Zeros == [i \in 1 .. 16 |-> "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"]

RECURSIVE Run(_)
Run(ls) == IF ls = <<>> THEN <<>> ELSE <<"> " \o Head(ls)>> \o Printed(Head(ls)) \o Run(Tail(ls))
Tex3 == "tex 3 2 16 16 1 1 28 0 0x20 0 0 4 0"

Init ==
    /\ dev \in {"device11", "device10"}
    /\ hold = [k \in MS |-> IF k = 0 THEN "picture" ELSE "zeros"]
    /\ log = <<"> " \o dev, dev \o " ok">> \o Run(Prelude) \o <<"> " \o Tex3, TexWords>>
    /\ n = 0

D11 == dev = "device11"

\* CopyResource: Direct3D 11 copies, 10 copies no multisampled resource
Copy(d, s) ==
    /\ hold' = IF D11 THEN [hold EXCEPT ![d] = hold[s]] ELSE hold
    /\ log' = log \o <<"> copy " \o S(d) \o " " \o S(s)>>
    /\ UNCHANGED dev /\ n' = n + 1

\* CopySubresourceRegion of the whole subresource to (0, 0, 0): Direct3D 11 copies the samples;
\* 10 nothing when either side holds samples that differ (else both hold zeros: no change)
CopySub(d, s) ==
    /\ hold' = IF D11 THEN [hold EXCEPT ![d] = hold[s]] ELSE hold
    /\ log' = log \o <<"> copysub " \o S(d) \o " 0 0 0 0 " \o S(s) \o " 0">>
    /\ UNCHANGED dev /\ n' = n + 1

\* ResolveSubresource of s into slot 1, then slot 1 read
Resolve(s) ==
    /\ log' = log \o <<"> resolve 1 0 " \o S(s) \o " 0 28", "> read 1 0 64 16 1", "read 16 1">>
                   \o (IF hold[s] = "picture" THEN Picture ELSE Zeros)
    /\ UNCHANGED <<dev, hold>> /\ n' = n + 1

Next ==
    /\ n < MaxOps
    /\ \/ \E d \in MS, s \in MS : d /= s /\ (Copy(d, s) \/ CopySub(d, s))
       \/ \E s \in MS : Resolve(s)

Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps
TypeOK == /\ dev \in {"device11", "device10"} /\ hold \in [MS -> {"picture", "zeros"}]
=============================================================================
