/* what gdi.c's slots offer the other GDI modules (gdiline.c, gdirgn.c, ...) */
HDC gdi_dc(int i);      /* token i names a slot: its DC (else bad, NULL) */
/* slot n takes over a memory DC with a bitmap of any kind (DIB section or DDB) selected */
void gdi_adopt(long n, HDC dc, HBITMAP bmp, HGDIOBJ oldbmp);
HBITMAP gdi_bitmap(int i); /* token i names a slot: its bitmap */
HRGN *rgn_slot(int i);  /* token i names a region slot (gdirgn.c): its handle, NULL if none (else bad) */
