---------------------------- MODULE D3DResCalls ----------------------------
(* The Direct3D 10 and 11 resource lines around the device, from spec/d3dres.txt (runner/d3dres.c,
   goldens d3d11res and d3d10res probe-removed, probe-removed-calls, x86-usageff-0): before a
   device (none made, or device11 at feature level 0 refused) every resource and drawing line
   prints nodev (D3DRES.NODEV); a NULL out pointer only checks, S_FALSE for a description taken
   (D3DRES.CHECK); on the x86 executor a texture of usage 0xffffffff ends the program, so nothing
   after it is printed, while x86-64 makes it as staging (D3DRES.X86CRASH); an update or a region
   copy past the destination removes the device, after which reading fails with
   DXGI_ERROR_DEVICE_REMOVED (11: the staging copy; 10: its Map) (D3DRES.REMOVED), Direct3D 11
   refuses new resources and views with it (a checked one too), 10 makes them, and every other
   call runs unseen; an empty slot is still nores (the executor's own answer) (D3DRES.REMOVED.OPS).
   A device line starts afresh. The resources: a 1 by 1 R8G8B8A8_UNORM texture (shader resource
   and render target), the same of usage 0xffffffff with CPU read access, one of width 0, a 16-byte
   vertex buffer; none has initial data, clears clear to 0, updates are empty or past the
   resource, so every read gives zeros. Not here: a texture of usage 0xffffffff on a removed device
   on x86 (each x86 run ends at its first such texture, so x86-usageff-0 is the one measured). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES arch, dev, removed, gone, res, views, log, n
vars == <<arch, dev, removed, gone, res, views, log, n>>

Slots == 0 .. 1
REMOVED == "887a0005"
INVALIDARG == "80070057"
S_FALSE == "00000001"
None == "none"
S(i) == ToString(i)

\* the descriptions: tex <slot> then these; GetDesc's words of each made one
Tex == [t |-> "2 1 1 1 1 0x1c 0 0x28 0 0 1 0", ff |-> "2 1 1 1 1 0x1c 0xffffffff 0 0x20000 0 1 0",
        bad |-> "2 0 1 1 1 0x1c 0 0x28 0 0 1 0"]
TexWords == [t |-> "tex = 1 1 1 1 1c 1 0 0 28 0 0", ff |-> "tex = 1 1 1 1 1c 1 0 ffffffff 0 20000 0"]
BUF == "16 0 1 0 0 0"
BufWords(d) == IF d = "d11" THEN "buf = 10 0 1 0 0 0" ELSE "buf = 10 0 1 0 0"
\* a view of the whole texture (nodesc): its format and dimension (TEXTURE2D) and the mip words
ViewWords == [srv |-> "srv = 1c 4 0 1", rtv |-> "rtv = 1c 4 0"]
\* a read of subresource 0: the texture's one row, the buffer's 16 bytes
ReadOf(r) == IF res[r] = "b" THEN <<"read 1 1", "00000000000000000000000000000000">>
             ELSE <<"read 1 1", "00000000">>
ReadArgs(r) == IF res[r] = "b" THEN " 0 16 1 1" ELSE " 0 4 1 1"

D11 == dev = "d11"

\* the device line a behavior may start with (beside starting with none)
Devices == [d11 |-> <<"> device11", "device11 ok">>, d10 |-> <<"> device10", "device10 ok">>,
            d101 |-> <<"> device10_1 0xa100", "device10_1 ok">>]

Init ==
    /\ arch \in {32, 64}
    /\ dev \in {None, "d11", "d10", "d101"} /\ removed = FALSE /\ gone = FALSE
    /\ res = [r \in Slots |-> None] /\ views = [v \in Slots |-> None]
    /\ n = 0
    /\ log = <<"# arch " \o S(arch)>> \o (IF dev = None THEN <<>> ELSE Devices[dev])

Call(c, out) == log' = log \o <<"> " \o c>> \o out
Keep == UNCHANGED <<dev, removed, gone, res, views>>
\* a line before a device: nodev, nothing else changes
NoDev(c) == /\ dev = None /\ Keep /\ Call(c, <<"nodev">>)

Device(line, d, out) ==
    /\ dev' = d /\ removed' = FALSE /\ UNCHANGED gone
    /\ res' = [r \in Slots |-> None] /\ views' = [v \in Slots |-> None]
    /\ Call(line, <<out>>)

\* CreateTexture2D: the slot is emptied first; D3DRES.X86CRASH ends the x86 executor
MakeTex(s, k) ==
    LET c == "tex " \o S(s) \o " " \o Tex[k] IN
    IF dev = None THEN NoDev(c)
    ELSE IF k = "ff" /\ arch = 32 THEN
        /\ ~removed /\ gone' = TRUE /\ UNCHANGED <<dev, removed, res, views>> /\ Call(c, <<>>)
    ELSE IF removed /\ D11 THEN
        /\ res' = [res EXCEPT ![s] = None] /\ UNCHANGED <<dev, removed, gone, views>>
        /\ Call(c, <<"tex hr " \o REMOVED>>)
    ELSE IF k = "bad" THEN
        /\ res' = [res EXCEPT ![s] = None] /\ UNCHANGED <<dev, removed, gone, views>>
        /\ Call(c, <<"tex hr " \o INVALIDARG>>)
    ELSE /\ res' = [res EXCEPT ![s] = k] /\ UNCHANGED <<dev, removed, gone, views>>
         /\ Call(c, <<TexWords[k]>>)
CheckTex(k) ==
    LET c == "tex check " \o Tex[k] IN
    IF dev = None THEN NoDev(c)
    ELSE /\ Keep
         /\ Call(c, <<"tex hr " \o (IF removed /\ D11 THEN REMOVED ELSE IF k = "bad" THEN INVALIDARG ELSE S_FALSE)>>)
MakeBuf(s) ==
    LET c == "buf " \o S(s) \o " " \o BUF IN
    IF dev = None THEN NoDev(c)
    ELSE IF removed /\ D11 THEN
        /\ res' = [res EXCEPT ![s] = None] /\ UNCHANGED <<dev, removed, gone, views>>
        /\ Call(c, <<"buf hr " \o REMOVED>>)
    ELSE /\ res' = [res EXCEPT ![s] = "b"] /\ UNCHANGED <<dev, removed, gone, views>>
         /\ Call(c, <<BufWords(dev)>>)
CheckBuf ==
    LET c == "buf check " \o BUF IN
    IF dev = None THEN NoDev(c)
    ELSE /\ Keep /\ Call(c, <<"buf hr " \o (IF removed /\ D11 THEN REMOVED ELSE S_FALSE)>>)

\* a view of the resource in slot r into view slot r, or only checked: of the texture made (one of
\* the staging texture or of the buffer, which bind neither, or a UAV, refused); the view slot is
\* emptied first unless checked, on Direct3D 10 before "uav none" too
View(k, r, check) ==
    LET c == k \o " " \o S(r) \o " " \o S(r) \o (IF check THEN " check" ELSE "") \o " nodesc"
        emptied == IF check THEN views ELSE [views EXCEPT ![r] = None]
        answer(a) == /\ UNCHANGED <<dev, removed, gone, res>> /\ views' = emptied /\ Call(c, <<a>>)
    IN IF dev = None THEN NoDev(c)
    ELSE IF res[r] = None THEN /\ Keep /\ Call(c, <<k \o " nores">>)
    ELSE IF k = "uav" /\ ~D11 THEN answer("uav none")
    ELSE IF removed /\ D11 THEN answer(k \o " hr " \o REMOVED)
    ELSE IF res[r] = "t" /\ k /= "uav" THEN
        IF check THEN /\ Keep /\ Call(c, <<k \o " hr " \o S_FALSE>>)
        ELSE /\ UNCHANGED <<dev, removed, gone, res>> /\ views' = [views EXCEPT ![r] = k] /\ Call(c, <<ViewWords[k]>>)
    ELSE answer(k \o " hr " \o INVALIDARG)

\* ClearRenderTargetView to 0 (the textures hold zeros already), ClearDepthStencilView and
\* ClearUnorderedAccessViewUint of slots that hold no such view
Clear(v) ==
    LET c == "clear " \o S(v) \o " 0 0 0 0" IN
    IF dev = None THEN NoDev(c)
    ELSE /\ Keep /\ Call(c, IF views[v] = "rtv" THEN <<>> ELSE <<"clear noview">>)
ClearDs(v) ==
    LET c == "cleards " \o S(v) \o " 1 0 0" IN
    IF dev = None THEN NoDev(c) ELSE /\ Keep /\ Call(c, <<"cleards noview">>)
ClearUav(v) ==
    LET c == "clearuav " \o S(v) \o " u 0 0 0 0" IN
    IF dev = None THEN NoDev(c)
    ELSE /\ Keep /\ Call(c, <<IF D11 THEN "clearuav noview" ELSE "clearuav none">>)

\* UpdateSubresource of a box 17 elements wide: past the texture and the buffer (the staging
\* texture takes no update); of an empty box: nothing
Update(r, past) ==
    LET c == "update " \o S(r) \o " 0 5 4 0 0 0 0 " \o (IF past THEN "17" ELSE "0") \o " 1 1" IN
    IF dev = None THEN NoDev(c)
    ELSE IF res[r] = None THEN /\ Keep /\ Call(c, <<"update nores">>)
    ELSE /\ removed' = (removed \/ (past /\ res[r] \in {"t", "b"}))
         /\ UNCHANGED <<dev, gone, res, views>> /\ Call(c, <<>>)
\* CopyResource (of zeros); CopySubresourceRegion of the whole source to x = 1, past the
\* destination: the device removed, unless one is a buffer and the other a texture
Copy(d, s) ==
    LET c == "copy " \o S(d) \o " " \o S(s) IN
    IF dev = None THEN NoDev(c)
    ELSE /\ Keep /\ Call(c, IF res[d] = None \/ res[s] = None THEN <<"copy nores">> ELSE <<>>)
CopySub(d, s) ==
    LET c == "copysub " \o S(d) \o " 0 1 0 0 " \o S(s) \o " 0" IN
    IF dev = None THEN NoDev(c)
    ELSE IF res[d] = None \/ res[s] = None THEN /\ Keep /\ Call(c, <<"copysub nores">>)
    ELSE /\ removed' = (removed \/ ((res[d] = "b") = (res[s] = "b")))
         /\ UNCHANGED <<dev, gone, res, views>> /\ Call(c, <<>>)
\* ResolveSubresource of a single-sampled source: nothing
Resolve(d, s) ==
    LET c == "resolve " \o S(d) \o " 0 " \o S(s) \o " 0 28" IN
    IF dev = None THEN NoDev(c)
    ELSE /\ Keep /\ Call(c, IF res[d] = None \/ res[s] = None THEN <<"resolve nores">> ELSE <<>>)
Read(r) ==
    LET c == "read " \o S(r) \o ReadArgs(r) IN
    IF dev = None THEN NoDev(c)
    ELSE /\ Keep
         /\ Call(c, IF res[r] = None THEN <<"read nores">>
                    ELSE IF removed THEN <<IF D11 THEN "read hr " \o REMOVED ELSE "read map hr " \o REMOVED>>
                    ELSE ReadOf(r))
\* drawing lines before a device
Draws == \/ NoDev("draw 4 3 32") \/ NoDev("target 0")

Next ==
    /\ ~gone /\ n' = n + 1 /\ UNCHANGED arch
    /\ \/ Device("device11", "d11", "device11 ok")
       \/ Device("device11 0", None, "device11 failed " \o INVALIDARG)
       \/ Device("device10", "d10", "device10 ok")
       \/ Device("device10_1 0xa100", "d101", "device10_1 ok")
       \/ \E s \in Slots, k \in {"t", "ff", "bad"} : MakeTex(s, k)
       \/ \E k \in {"t", "bad"} : CheckTex(k)
       \/ \E s \in Slots : MakeBuf(s)
       \/ CheckBuf
       \/ \E k \in {"srv", "rtv", "uav"}, r \in Slots : View(k, r, FALSE)
       \/ \E k \in {"srv", "rtv"} : View(k, 0, TRUE)
       \/ \E v \in Slots : Clear(v)
       \/ ClearDs(0) \/ ClearUav(0)
       \/ \E r \in Slots : Update(r, TRUE)
       \/ Update(0, FALSE)
       \/ Copy(0, 1) \/ Copy(1, 0) \/ CopySub(0, 1) \/ CopySub(1, 0) \/ Resolve(0, 1)
       \/ \E r \in Slots : Read(r)
       \/ Draws

Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps
TypeOK == /\ dev \in {None, "d11", "d10", "d101"} /\ removed \in BOOLEAN /\ gone \in BOOLEAN
          /\ res \in [Slots -> {None, "t", "ff", "b"}] /\ views \in [Slots -> {None, "srv", "rtv"}]
\* the x86 executor ends at a texture of usage 0xffffffff on a live device and on no other line
Crash == gone => (arch = 32 /\ Len(log) > 1)
=============================================================================
