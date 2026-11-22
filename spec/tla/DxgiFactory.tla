---------------------------- MODULE DxgiFactory ----------------------------
(* The DXGI factory on the windows-2025 runner, from spec/dxgi.txt: CreateDXGIFactory2,
   IDXGIFactory4::EnumWarpAdapter, IDXGIFactory5::CheckFeatureSupport (ALLOW_TEARING),
   IDXGIFactory1::EnumAdapters1 and IDXGIFactory6::EnumAdapterByGpuPreference (DXGI.FACTORY,
   FACTORY.DEBUG, WARP, TEARING, ADAPTERS, ADAPTER.PREF). The state is whether a factory is made;
   each call answers with what the clause says the runner answers, so the log holds every answer;
   `kikizake tla` replays every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES factory, log, n
vars == <<factory, log, n>>

\* DXGI.FACTORY: the flags asked: 0, DXGI_CREATE_FACTORY_DEBUG (DXGI.FACTORY.DEBUG), the
\* measured other bits and all of them
FactoryFlags == {"0", "1", "2", "4", "0x10", "0x80000000", "0xffffffff"}
\* (the flags and preferences as the case language writes them: TLC's integers stop at 2^31 - 1)
\* DXGI.ADAPTER.PREF: the adapter indices and the preferences (in range and past it)
Indices == {0, 1, 2, 7}
Prefs == {"0", "1", "2", "3", "0x40", "0xffffffff"}

\* DXGI.WARP: the WARP adapter: vendor 0x1414, device 0x8c, DXGI_ADAPTER_FLAG_SOFTWARE (2)
Warp == "1414 8c 2"
\* DXGI.ADAPTERS, ADAPTER.PREF: the runner's adapters in order: one of WARP's ids without the
\* software flag (the render-only adapter), then WARP; past the last DXGI_ERROR_NOT_FOUND
Adapters == <<"1414 8c 0", Warp>>
NOT_FOUND == "887a0002"

Init ==
    /\ factory = FALSE
    /\ log = <<>>
    /\ n = 0

\* the calls that need a factory answer "nofactory" without one
Need(c, a) == <<"> " \o c, IF factory THEN a ELSE "nofactory">>

\* DXGI.FACTORY, FACTORY.DEBUG: every flags value makes the factory (the others are ignored; the
\* debug one taken on the runner)
Factory(f) ==
    /\ factory' = TRUE
    /\ log' = log \o <<"> dxgifactory " \o f, "factory ok">>

WarpAdapter ==
    /\ log' = log \o Need("dxgiwarp", "warp vendor 1414 device 8c flags 2")
    /\ UNCHANGED factory

\* DXGI.TEARING: the runner allows tearing
Tearing ==
    /\ log' = log \o Need("dxgitearing", "tearing 1")
    /\ UNCHANGED factory

\* DXGI.ADAPTERS: two adapters, the last WARP
EnumAdapters ==
    /\ log' = log \o Need("dxgiadapters", "adapters " \o ToString(Len(Adapters)) \o " last " \o Adapters[Len(Adapters)])
    /\ UNCHANGED factory

\* DXGI.ADAPTER.PREF: the same order for every preference, not range checked
Pref(i, p) ==
    /\ log' = log \o Need("dxgipref " \o ToString(i) \o " " \o p,
                  IF i < Len(Adapters) THEN "pref hr 00000000 " \o Adapters[i + 1] ELSE "pref hr " \o NOT_FOUND)
    /\ UNCHANGED factory

Next ==
    /\ n' = n + 1
    /\ \/ \E f \in FactoryFlags : Factory(f)
       \/ WarpAdapter
       \/ Tearing
       \/ EnumAdapters
       \/ \E i \in Indices, p \in Prefs : Pref(i, p)

Bound == n <= MaxOps

\* every call adds its line and its answer
LogPairs == Len(log) = 2 * n
\* nothing is answered but "nofactory" before a factory is made
NoFactoryFirst == ~factory => \A k \in 1..Len(log) : k % 2 = 0 => log[k] = "nofactory"

Spec == Init /\ [][Next]_vars
=============================================================================
