---------------------------- MODULE DxgiSwapChain ----------------------------
(* A flip-model swap chain on Direct3D 12, from spec/dxgi.txt: CreateSwapChainForHwnd, GetBuffer,
   Present, ResizeBuffers, SetMaximumFrameLatency / GetMaximumFrameLatency and
   GetFrameLatencyWaitableObject (the DXGI clauses below). The chain is its description (the
   twelve words GetDesc1 gives), the back buffer index, whether it has presented since it was
   made or resized, and its frame latency. Each call answers with what the clause says, so the
   log holds every answer; `kikizake tla` replays every state's log through the Rust model
   (spec/tla/README.md). Every behavior makes one chain and takes at most MaxOps calls of it. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES desc, index, fresh, latency, log, n
vars == <<desc, index, fresh, latency, log, n>>

HD == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
RECURSIVE Hex(_)
Hex(v) == IF v < 16 THEN HD[v + 1] ELSE Hex(v \div 16) \o HD[(v % 16) + 1]
\* the words as GetDesc1's line prints them (hex), and as a call gives them (decimal)
Join(w, F(_)) == LET RECURSIVE j(_) j(i) == IF i = Len(w) THEN F(w[i]) ELSE F(w[i]) \o " " \o j(i + 1) IN j(1)
Words(w) == Join(w, Hex)
Args(w) == Join(w, ToString)
Bit(v, b) == (v \div (2 ^ b)) % 2 = 1

INVALID_CALL == "887a0001"
INVALIDARG == "80070057"

\* the description's words: 1 Width, 2 Height, 3 Format, 4 Stereo, 5 6 SampleDesc, 7 BufferUsage,
\* 8 BufferCount, 9 Scaling, 10 SwapEffect, 11 AlphaMode, 12 Flags
W == 1
H == 2
FMT == 3
COUNT == 8
FLAGS == 12
WAITABLE == 6
TEARING == 11
\* DXGI.CHAIN.FORMAT: the formats a flip chain takes
Flip == {10, 24, 28, 87}
\* DXGI.CHAIN.FLAGS: the flags a chain takes (ALLOW_MODE_SWITCH, RESTRICT_SHARED_RESOURCE_DRIVER,
\* FRAME_LATENCY_WAITABLE_OBJECT, ALLOW_TEARING, HW_PROTECTED, 0x2000), as bit numbers
ChainBits == {1, 5, 6, 11, 12, 13}
UnknownFlags(f) == \E b \in 0..30 : Bit(f, b) /\ b \notin ChainBits
\* DXGI.PRESENT: TEST, DO_NOT_SEQUENCE, RESTART, RESTRICT_TO_OUTPUT, ALLOW_TEARING
PresentBits == {0, 1, 2, 8, 9}
UnknownPresent(f) == \E b \in 0..30 : Bit(f, b) /\ b \notin PresentBits

\* the chains made: a default one (DXGI.CHAIN.SIZE: 0 by 0 is the window's 64 by 64), one with
\* three buffers, FLIP_SEQUENTIAL, FRAME_LATENCY_WAITABLE_OBJECT and ALLOW_TEARING, and one of one
\* buffer, refused (DXGI.CHAIN.BUFFERS)
Made == {<<0, 0, 87, 0, 1, 0, 32, 2, 0, 4, 0, 0>>, <<100, 50, 24, 0, 1, 0, 32, 3, 0, 3, 0, 2112>>, <<0, 0, 87, 0, 1, 0, 32, 1, 0, 4, 0, 0>>}
\* the calls asked: Present's sync intervals and flags (one unknown: 8), ResizeBuffers' count,
\* width, height, format, flags, GetBuffer's buffers, SetMaximumFrameLatency's values
Syncs == {0, 1, 5}
PFlags == {0, 1, 2, 4, 8, 256, 512, 513}
Resizes == {<<0, 0, 0, 0, 0>>, <<3, 100, 50, 0, 0>>, <<0, 0, 0, 28, 2112>>, <<16, 0, 7, 10, 64>>,
            <<1, 0, 0, 0, 0>>, <<17, 0, 0, 0, 0>>, <<0, 0, 0, 29, 0>>, <<0, 0, 0, 0, 4>>, <<0, 0, 0, 0, 2>>}
Buffers == {0, 2, 3}
Latencies == {0, 1, 40, 41}

Init ==
    \E d \in Made :
        /\ n = 0
        /\ latency = 1
        /\ fresh = TRUE
        /\ index = 0
        /\ IF d[COUNT] < 2 \/ d[COUNT] > 16
           THEN /\ desc = <<>>
                /\ log = <<"> device12", "device12 ok", "> dxgifactory 0", "factory ok", "> swapchain " \o Args(d), "swapchain hr " \o INVALID_CALL>>
           ELSE LET m == IF d[W] = 0 \/ d[H] = 0 THEN [d EXCEPT ![W] = 64, ![H] = 64] ELSE d
                IN /\ desc = m
                   /\ log = <<"> device12", "device12 ok", "> dxgifactory 0", "factory ok", "> swapchain " \o Args(d), "swapchain = " \o Words(m) \o " index 0">>

Made1 == desc # <<>>
Call(c, a) == <<"> " \o c, IF Made1 THEN a ELSE "nochain">>

\* DXGI.PRESENT, PRESENT.TEST, PRESENT.RESTART: sync intervals 0..4; unknown flags, TEST with any
\* other flag, ALLOW_TEARING without the chain's flag or with an interval, DO_NOT_SEQUENCE before
\* the chain has presented: refused, the index kept; TEST and DO_NOT_SEQUENCE present nothing;
\* any other present (RESTART too) moves the index on by one, modulo the count
Present(s, f) ==
    LET bad == s > 4 \/ UnknownPresent(f) \/ (Bit(f, 9) /\ (~Bit(desc[FLAGS], TEARING) \/ s # 0))
               \/ (Bit(f, 0) /\ f # 1) \/ (Bit(f, 1) /\ fresh)
        moves == ~bad /\ ~Bit(f, 0) /\ ~Bit(f, 1)
        i == IF moves THEN (index + 1) % desc[COUNT] ELSE index
    IN /\ index' = IF Made1 THEN i ELSE index
       /\ fresh' = IF Made1 /\ moves THEN FALSE ELSE fresh
       /\ log' = log \o Call("scpresent " \o ToString(s) \o " " \o ToString(f),
                     "present hr " \o (IF bad THEN INVALID_CALL ELSE "00000000") \o " index " \o Hex(i))
       /\ UNCHANGED <<desc, latency>>

\* DXGI.RESIZE: unknown flags DXGI_ERROR_INVALID_CALL; a count of 1 or past 16, a format no flip
\* chain takes, a change of FRAME_LATENCY_WAITABLE_OBJECT or ALLOW_TEARING E_INVALIDARG; else a
\* count or format of 0 keeps the chain's, a 0 width or height is the window's (DXGI.CHAIN.SIZE),
\* the flags are the new ones, the index starts at 0 again and the chain has not presented
Resize(r) ==
    LET c == r[1]
        w == r[2]
        h == r[3]
        f == r[4]
        fl == r[5]
        unknown == UnknownFlags(fl)
        refused == c = 1 \/ c > 16 \/ (f # 0 /\ f \notin Flip)
                   \/ Bit(fl, WAITABLE) # Bit(desc[FLAGS], WAITABLE) \/ Bit(fl, TEARING) # Bit(desc[FLAGS], TEARING)
        sized == IF w = 0 \/ h = 0 THEN <<64, 64>> ELSE <<w, h>>
        m == [desc EXCEPT ![W] = sized[1], ![H] = sized[2], ![COUNT] = IF c = 0 THEN @ ELSE c,
                          ![FMT] = IF f = 0 THEN @ ELSE f, ![FLAGS] = fl]
        ok == Made1 /\ ~unknown /\ ~refused
    IN /\ desc' = IF ok THEN m ELSE desc
       /\ index' = IF ok THEN 0 ELSE index
       /\ fresh' = IF ok THEN TRUE ELSE fresh
       /\ log' = log \o Call("scresize " \o Args(r),
                     IF unknown THEN "resize hr " \o INVALID_CALL ELSE IF refused THEN "resize hr " \o INVALIDARG
                     ELSE "resize = " \o Words(m) \o " index 0")
       /\ UNCHANGED latency

\* DXGI.BUFFER: buffers 0..count-1 are 2D render targets of the chain's size and format, one mip,
\* one sample; others DXGI_ERROR_INVALID_CALL
Buffer(b) ==
    /\ log' = log \o Call("scbuffer " \o ToString(b),
                  IF b < desc[COUNT] THEN "buffer 3 " \o Hex(desc[W]) \o " " \o Hex(desc[H]) \o " 1 1 " \o Hex(desc[FMT]) \o " 1 0 1"
                  ELSE "buffer hr " \o INVALID_CALL)
    /\ UNCHANGED <<desc, index, fresh, latency>>

\* DXGI.LATENCY: only a FRAME_LATENCY_WAITABLE_OBJECT chain (else both calls refused); 1..40 taken,
\* the rest refused; GetMaximumFrameLatency gives the latency, 1 at first
Latency(v) ==
    LET waitable == Made1 /\ Bit(desc[FLAGS], WAITABLE)
        taken == waitable /\ v >= 1 /\ v <= 40
        l == IF taken THEN v ELSE latency
    IN /\ latency' = l
       /\ log' = log \o Call("sclatency " \o ToString(v),
                     IF ~waitable THEN "latency hr " \o INVALID_CALL \o " get " \o INVALID_CALL \o " cccccccc"
                     ELSE "latency hr " \o (IF taken THEN "00000000" ELSE INVALID_CALL) \o " get 00000000 " \o Hex(l))
       /\ UNCHANGED <<desc, index, fresh>>

\* DXGI.LATENCY: the waitable object only for a FRAME_LATENCY_WAITABLE_OBJECT chain
Wait ==
    /\ log' = log \o Call("scwait", IF Made1 /\ Bit(desc[FLAGS], WAITABLE) THEN "wait handle" ELSE "wait null")
    /\ UNCHANGED <<desc, index, fresh, latency>>

Next ==
    /\ n' = n + 1
    /\ \/ \E s \in Syncs, f \in PFlags : Present(s, f)
       \/ \E r \in Resizes : Resize(r)
       \/ \E b \in Buffers : Buffer(b)
       \/ \E v \in Latencies : Latency(v)
       \/ Wait

Bound == n <= MaxOps

Spec == Init /\ [][Next]_vars

\* the index is one of the buffers, the count and the size as a chain takes them
IndexInChain == Made1 => index < desc[COUNT]
ChainHolds == Made1 => desc[COUNT] >= 2 /\ desc[COUNT] <= 16 /\ desc[FMT] \in Flip /\ desc[W] > 0 /\ desc[H] > 0
LatencyHolds == latency >= 1 /\ latency <= 40
=============================================================================
