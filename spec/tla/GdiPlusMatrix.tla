---------------------------- MODULE GdiPlusMatrix ----------------------------
(* GDI+ matrices, from spec/gdiplus.txt: Matrix::Matrix, Multiply, Invert, TransformPoints
   (GP.MATRIX, GP.MATRIX.MUL, GP.MATRIX.ORDER, GP.MATRIX.POINT, GP.MATRIX.INVERT). The matrices
   here have small entries on the 1/64 grid, so every product, sum and reciprocal is exact in single
   precision and the answers (each element's float bits) follow from the clauses' formulas; what
   rounding leaves is the sign of a zero: a product's the exclusive or of its factors' signs, a
   sum -0 only of two -0 (x86 rounds to nearest, the executor `kikizake tla` answers for). Every
   behavior takes at most MaxOps calls on two empty matrix slots; after each, points are sent
   through the matrix it made or changed. `kikizake tla` replays every state's log through the
   Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES mat, log, n
vars == <<mat, log, n>>

S(i) == ToString(i)
D == 64
RECURSIVE Hex(_, _)
Hex(v, k) == IF k = 0 THEN "" ELSE Hex(v \div 16, k - 1) \o SubSeq("0123456789abcdef", (v % 16) + 1, (v % 16) + 1)
RECURSIVE Log2(_)
Log2(v) == IF v < 2 THEN 0 ELSE 1 + Log2(v \div 2)

\* a float: v / 64, and for v = 0 whether it is -0
F(k) == [v |-> k * D, nz |-> FALSE]
Neg0 == [v |-> 0, nz |-> TRUE]
Minus(a) == a.v < 0 \/ (a.v = 0 /\ a.nz)
Mul(a, b) == LET p == a.v * b.v IN [v |-> p \div D, nz |-> p = 0 /\ Minus(a) # Minus(b)]
Add(a, b) == LET s == a.v + b.v IN [v |-> s, nz |-> s = 0 /\ Minus(a) /\ Minus(b)]
Neg(a) == [v |-> -a.v, nz |-> a.v = 0 /\ ~a.nz]
Sub(a, b) == Add(a, Neg(b))
\* 1 / d (d not 0)
Recip(d) == [v |-> (D * D) \div d.v, nz |-> FALSE]
\* the float's bits: sign, exponent, mantissa (|v| < 2^24 on the grid)
Bits(a) == IF a.v = 0 THEN (IF a.nz THEN "80000000" ELSE "00000000")
           ELSE LET m == IF a.v < 0 THEN -a.v ELSE a.v
                    e == Log2(m)
                    b == (127 + e - 6) * 2 ^ 23 + (m - 2 ^ e) * 2 ^ (23 - e)
                IN IF a.v < 0 THEN Hex((b \div 2 ^ 28) + 8, 1) \o Hex(b % 2 ^ 28, 7) ELSE Hex(b, 8)
Elements(m) == "elements 0 " \o Bits(m[1]) \o " " \o Bits(m[2]) \o " " \o Bits(m[3]) \o " " \o Bits(m[4])
               \o " " \o Bits(m[5]) \o " " \o Bits(m[6])

\* GP.MATRIX.MUL: a b with points as row vectors, each element a sum of products left to right
Prod(a, b) == <<Add(Mul(a[1], b[1]), Mul(a[2], b[3])), Add(Mul(a[1], b[2]), Mul(a[2], b[4])),
                Add(Mul(a[3], b[1]), Mul(a[4], b[3])), Add(Mul(a[3], b[2]), Mul(a[4], b[4])),
                Add(Add(Mul(a[5], b[1]), Mul(a[6], b[3])), b[5]), Add(Add(Mul(a[5], b[2]), Mul(a[6], b[4])), b[6])>>
\* GP.MATRIX.INVERT: det = m11 m22 - m12 m21, r = 1 / det; m22 r, -m12 r, -m21 r, m11 r,
\* (m21 dy - m22 dx) r, (m12 dx - m11 dy) r
Det(m) == Sub(Mul(m[1], m[4]), Mul(m[2], m[3]))
Inverse(m) == LET r == Recip(Det(m)) IN
  <<Mul(m[4], r), Mul(Neg(m[2]), r), Mul(Neg(m[3]), r), Mul(m[1], r),
    Mul(Sub(Mul(m[3], m[6]), Mul(m[4], m[5])), r), Mul(Sub(Mul(m[2], m[5]), Mul(m[1], m[6])), r)>>
\* GP.MATRIX.POINT: (m11 x + m21 y + dx, m12 x + m22 y + dy)
Point(m, p) == <<Add(Add(Mul(m[1], p[1]), Mul(m[3], p[2])), m[5]), Add(Add(Mul(m[2], p[1]), Mul(m[4], p[2])), m[6])>>

Points == <<<<1, 2>>, <<0, 0>>, <<3, 1>>>>
Ask(k, m) ==
  LET call == "> gpmatpts " \o S(k) \o " 0x" \o Bits(F(1)) \o " 0x" \o Bits(F(2)) \o " 0x" \o Bits(F(0)) \o " 0x" \o Bits(F(0))
           \o " 0x" \o Bits(F(3)) \o " 0x" \o Bits(F(1))
      pt(i) == LET q == Point(m, <<F(Points[i][1]), F(Points[i][2])>>) IN " " \o Bits(q[1]) \o " " \o Bits(q[2])
  IN IF m = <<>> THEN <<call, "gpmatpts nomatrix">>
     ELSE <<call, "pts 0" \o pt(1) \o pt(2) \o pt(3)>>

Call(k, lines, m) == mat' = [mat EXCEPT ![k] = m] /\ log' = log \o lines \o Ask(k, m) /\ n' = n + 1

Init == mat = [k \in {0, 1} |-> <<>>] /\ log = <<>> /\ n = 0

\* the matrices made: the identity, a scale and translation (det 2), a shear (det 1), a singular
\* one, a swap of the axes (det -1)
Made == {<<1, 0, 0, 1, 0, 0>>, <<2, 0, 0, 1, 3, 1>>, <<1, 1, 0, 1, 0, 2>>, <<1, 1, 1, 1, 0, 0>>, <<0, 1, 1, 0, 0, 0>>}
Of(e) == [i \in 1..6 |-> F(e[i])]
Args(m) == " 0x" \o Bits(m[1]) \o " 0x" \o Bits(m[2]) \o " 0x" \o Bits(m[3]) \o " 0x" \o Bits(m[4]) \o " 0x" \o Bits(m[5]) \o " 0x" \o Bits(m[6])

\* GP.MATRIX: the elements read back as given
New(k, e) == Call(k, <<"> gpmatrix " \o S(k) \o Args(Of(e)), "matrix 0", Elements(Of(e))>>, Of(e))

\* GP.MATRIX.MUL, GP.MATRIX.ORDER: MatrixOrderPrepend (0) the passed matrix on the left, Append (1)
\* on the right, any other order InvalidParameter, the matrix unchanged
Multiply(k, o, order) ==
  LET call == "> gpmatmul " \o S(k) \o " " \o S(o) \o " " \o S(order) IN
  IF mat[k] = <<>> \/ mat[o] = <<>> THEN Call(k, <<call, "gpmatmul nomatrix">>, mat[k])
  ELSE IF order \notin {0, 1} THEN Call(k, <<call, "matmul 2", Elements(mat[k])>>, mat[k])
  ELSE LET p == IF order = 0 THEN Prod(mat[o], mat[k]) ELSE Prod(mat[k], mat[o]) IN
       Call(k, <<call, "matmul 0", Elements(p)>>, p)

\* GP.MATRIX.INVERT: not invertible (a determinant of 0) InvalidParameter, the matrix unchanged
Invert(k) ==
  LET call == "> gpmatinv " \o S(k) IN
  IF mat[k] = <<>> THEN Call(k, <<call, "gpmatinv nomatrix">>, mat[k])
  ELSE IF Det(mat[k]).v = 0 THEN Call(k, <<call, "matinv 0 0 2", Elements(mat[k])>>, mat[k])
  ELSE Call(k, <<call, "matinv 0 1 0", Elements(Inverse(mat[k]))>>, Inverse(mat[k]))

Next == /\ n < MaxOps
        /\ \/ \E k \in {0, 1}, e \in Made : New(k, e)
           \/ \E k, o \in {0, 1}, order \in {0, 1, 2, -1} : Multiply(k, o, order)
           \/ \E k \in {0, 1} : Invert(k)

Bound == n <= MaxOps

Spec == Init /\ [][Next]_vars

\* every element stays on the grid and inside single precision's exact integers
Exact == \A k \in {0, 1} : mat[k] # <<>> => \A i \in 1..6 : mat[k][i].v \in -(2 ^ 24)..(2 ^ 24)
=============================================================================
