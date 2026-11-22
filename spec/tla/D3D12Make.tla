---------------------------- MODULE D3D12Make ----------------------------
(* The Direct3D 12 objects a device makes beside resources, from spec/d3d12.txt:
   CreateDescriptorHeap, CreateCommandQueue, CreateCommandAllocator / CreateCommandList,
   CreateQueryHeap, CreateFence, GetDescriptorHandleIncrementSize, and RemoveDevice before a
   clear's CreateCommittedResource (D3D12.MAKE.DESCHEAP, MAKE.QUEUE, MAKE.LIST, MAKE.QUERYHEAP,
   MAKE.FENCE, FEATURE.INCREMENT, CLEAR.REMOVED). Each behavior is one executor, x86 or x86-64
   (the log's "# arch" entry: `kikizake tla` replays the state as that executor's model), one
   device, and up to MaxOps calls of it, each answered as its clause says. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES arch, removed, log, n
vars == <<arch, removed, log, n>>

OK == "00000000"
INVALIDARG == "80070057"
OUTOFMEMORY == "8007000e"
NOT_CURRENTLY_AVAILABLE == "887a002b"
DEVICE_REMOVED == "887a0005"

\* the arguments asked: descriptor heap types (CBV/SRV/UAV, sampler, RTV, DSV), counts and
\* flags; queue and list types (direct, bundle, compute, copy, the video ones and past them),
\* priorities (NORMAL 0, HIGH 100, GLOBAL_REALTIME 10000, others) and flags; query heap types
\* 0..8 and counts around the limits; fence values and flags
DescTypes == 0..3
Counts == {0, 1, 1000001}
DescFlags == {0, 1, 2, 3}
QueueTypes == {0, 1, 2, 3, 4, 5, 6, 7}
Priorities == {0, 1, 100, 10000}
QueueFlags == {0, 1, 2}
QueryTypes == 0..8
QueryCounts == {0, 1, 7864320, 8126463, 8126464, 8126465, 16777216}
\* the first count an x86 process runs out of memory for (31 x 2^18): at it types 0 and 3 were
\* taken on one run and out of memory on another
QHEAP32_OOM == 8126464
FenceValues == {0, 5}
FenceFlags == {0, 1, 2, 3, 4}

Init ==
    /\ arch \in {32, 64}
    /\ removed = FALSE
    /\ n = 0
    /\ log = <<"# arch " \o ToString(arch), "> device12", "device12 ok">>

Ans(c, a) == <<"> " \o c, a>>
Hr(name, ok) == name \o " hr " \o (IF ok THEN OK ELSE INVALIDARG)
Words(s) == LET RECURSIVE j(_) j(i) == IF i = Len(s) THEN ToString(s[i]) ELSE ToString(s[i]) \o " " \o j(i + 1) IN j(1)

\* D3D12.MAKE.DESCHEAP: no count checked; SHADER_VISIBLE (1) only for CBV/SRV/UAV (0) and
\* samplers (1); no other flag
DescHeap(t, c, f) ==
    log' = log \o Ans("dheap12 " \o Words(<<t, c, f>>), Hr("dheap12", f \in {0, 1} /\ (f = 0 \/ t <= 1)))

\* D3D12.MAKE.QUEUE: direct, compute and copy queues; NORMAL or HIGH; GLOBAL_REALTIME not
\* available to the process (a direct queue without flags asked); DISABLE_GPU_TIMEOUT only
Queue(t, p, f) ==
    log' = log \o Ans("queue12 " \o Words(<<t, p, f>>),
        IF p = 10000 /\ t = 0 /\ f = 0 THEN "queue12 hr " \o NOT_CURRENTLY_AVAILABLE
        ELSE Hr("queue12", t \in {0, 2, 3} /\ p \in {0, 100} /\ f \in {0, 1}))

\* D3D12.MAKE.LIST: direct, bundle, compute and copy lists; the allocator refuses the others
List(t) ==
    log' = log \o Ans("cmdlist12 " \o ToString(t),
        IF t <= 3 THEN "cmdlist12 hr " \o OK ELSE "cmdlist12 allocator hr " \o INVALIDARG)

\* D3D12.MAKE.QUERYHEAP: every type 0..7 but VIDEO_DECODE_STATISTICS (4), at least one query;
\* x86-64 takes 16777216; x86 runs out of memory from QHEAP32_OOM, where types 0 and 3 answered
\* either
QueryHeap(t, c) ==
    LET valid == t <= 7 /\ t # 4 /\ c > 0
    IN log' = log \o
        IF arch = 32 /\ c = QHEAP32_OOM /\ t \in {0, 3}
        THEN <<"> qheap12 " \o Words(<<t, c>>), "oneof qheap12 hr " \o OK \o " | qheap12 hr " \o OUTOFMEMORY>>
        ELSE Ans("qheap12 " \o Words(<<t, c>>),
            IF valid /\ arch = 32 /\ c >= QHEAP32_OOM THEN "qheap12 hr " \o OUTOFMEMORY ELSE Hr("qheap12", valid))

\* D3D12.MAKE.FENCE: NONE, SHARED (1), SHARED with SHARED_CROSS_ADAPTER (3); a new fence's
\* completed value is its initial one
Fence(v, f) ==
    log' = log \o Ans("fence12 " \o Words(<<v, f>>),
        IF f \in {0, 1, 3} THEN "fence12 hr " \o OK \o " value " \o ToString(v) ELSE "fence12 hr " \o INVALIDARG)

\* D3D12.FEATURE.INCREMENT: CBV/SRV/UAV and samplers 64 bytes apart; RTV 32 and DSV 8 on x86-64,
\* 16 and 4 on x86 (hex)
Incr ==
    log' = log \o Ans("incr12", IF arch = 64 THEN "incr 40 40 20 8" ELSE "incr 40 40 10 4")

\* D3D12.CLEAR.REMOVED: after RemoveDevice the clear's resource is not made
Remove ==
    /\ removed' = TRUE
    /\ log' = log \o Ans("remove12", "remove12 ok")
ClearRemoved ==
    /\ removed
    /\ log' = log \o Ans("clear12 28 0 0 0 0", "clear12 hr " \o DEVICE_REMOVED)

Next ==
    /\ n' = n + 1
    /\ UNCHANGED arch
    /\ \/ /\ \/ \E t \in DescTypes, c \in Counts, f \in DescFlags : DescHeap(t, c, f)
             \/ \E t \in QueueTypes, p \in Priorities, f \in QueueFlags : Queue(t, p, f)
             \/ \E t \in QueueTypes : List(t)
             \/ \E t \in QueryTypes, c \in QueryCounts : QueryHeap(t, c)
             \/ \E v \in FenceValues, f \in FenceFlags : Fence(v, f)
             \/ Incr
             \/ ClearRemoved
          /\ UNCHANGED removed
       \/ Remove

Bound == n <= MaxOps

\* every call answers once
LogPairs == Len(log) = 3 + 2 * n

Spec == Init /\ [][Next]_vars
=============================================================================
