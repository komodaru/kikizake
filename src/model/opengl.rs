//! OpenGL 1.1 through WGL on a 16x16 DIB section: an executable specification of what Microsoft's
//! GDI generic implementation (opengl32.dll with no driver) does for the opengl suite's cases.
//!
//! Sources (Microsoft Learn, "OpenGL" reference): the pages of each function named in
//! spec/opengl.txt (glGet's table of initial values, glGetError, glReadPixels, glDrawPixels,
//! glPixelStore, glBlendFunc, glLogicOp, glAlphaFunc, glStencilFunc / glStencilOp, glDepthFunc,
//! glScissor, glColorMask, glOrtho, glTranslate / glScale / glRotate, glPushMatrix), and for WGL
//! "PIXELFORMATDESCRIPTOR", "ChoosePixelFormat", "wglCreateContext", "wglMakeCurrent".

/// GL enumerants the suite uses (values from gl.h)
pub mod e {
    pub const NO_ERROR: u32 = 0;
    pub const INVALID_ENUM: u32 = 0x500;
    pub const INVALID_VALUE: u32 = 0x501;
    pub const INVALID_OPERATION: u32 = 0x502;
    pub const STACK_OVERFLOW: u32 = 0x503;
    pub const STACK_UNDERFLOW: u32 = 0x504;

    pub const POINTS: u32 = 0;
    pub const QUADS: u32 = 7;

    pub const NEVER: u32 = 0x200;
    pub const LESS: u32 = 0x201;
    pub const ALWAYS: u32 = 0x207;

    pub const ZERO: u32 = 0;
    pub const ONE: u32 = 1;
    pub const SRC_COLOR: u32 = 0x300;
    pub const ONE_MINUS_SRC_COLOR: u32 = 0x301;
    pub const SRC_ALPHA: u32 = 0x302;
    pub const ONE_MINUS_SRC_ALPHA: u32 = 0x303;
    pub const DST_ALPHA: u32 = 0x304;
    pub const ONE_MINUS_DST_ALPHA: u32 = 0x305;
    pub const DST_COLOR: u32 = 0x306;
    pub const ONE_MINUS_DST_COLOR: u32 = 0x307;
    pub const SRC_ALPHA_SATURATE: u32 = 0x308;

    pub const FRONT: u32 = 0x404;
    pub const BACK: u32 = 0x405;
    pub const FRONT_AND_BACK: u32 = 0x408;
    pub const CW: u32 = 0x900;
    pub const CCW: u32 = 0x901;

    pub const BYTE: u32 = 0x1400;
    pub const UNSIGNED_BYTE: u32 = 0x1401;
    pub const SHORT: u32 = 0x1402;
    pub const UNSIGNED_SHORT: u32 = 0x1403;
    pub const INT: u32 = 0x1404;
    pub const UNSIGNED_INT: u32 = 0x1405;
    pub const FLOAT: u32 = 0x1406;
    pub const BITMAP: u32 = 0x1a00;

    pub const CLEAR: u32 = 0x1500;
    pub const COPY: u32 = 0x1503;
    pub const SET: u32 = 0x150f;

    pub const MODELVIEW: u32 = 0x1700;
    pub const PROJECTION: u32 = 0x1701;
    pub const TEXTURE: u32 = 0x1702;

    pub const COLOR_INDEX: u32 = 0x1900;
    pub const STENCIL_INDEX: u32 = 0x1901;
    pub const DEPTH_COMPONENT: u32 = 0x1902;
    pub const RED: u32 = 0x1903;
    pub const GREEN: u32 = 0x1904;
    pub const BLUE: u32 = 0x1905;
    pub const ALPHA: u32 = 0x1906;
    pub const RGB: u32 = 0x1907;
    pub const RGBA: u32 = 0x1908;
    pub const LUMINANCE: u32 = 0x1909;
    pub const LUMINANCE_ALPHA: u32 = 0x190a;
    pub const BGR_EXT: u32 = 0x80e0;
    pub const BGRA_EXT: u32 = 0x80e1;

    pub const FLAT: u32 = 0x1d00;
    pub const SMOOTH: u32 = 0x1d01;
    pub const KEEP: u32 = 0x1e00;
    pub const REPLACE: u32 = 0x1e01;
    pub const INCR: u32 = 0x1e02;
    pub const DECR: u32 = 0x1e03;
    pub const INVERT: u32 = 0x150a;

    pub const VENDOR: u32 = 0x1f00;
    pub const RENDERER: u32 = 0x1f01;
    pub const VERSION: u32 = 0x1f02;
    pub const EXTENSIONS: u32 = 0x1f03;

    pub const DONT_CARE: u32 = 0x1100;
    pub const FASTEST: u32 = 0x1101;
    pub const NICEST: u32 = 0x1102;

    pub const CULL_FACE: u32 = 0xb44;
    pub const DEPTH_TEST: u32 = 0xb71;
    pub const STENCIL_TEST: u32 = 0xb90;
    pub const ALPHA_TEST: u32 = 0xbc0;
    pub const DITHER: u32 = 0xbd0;
    pub const BLEND: u32 = 0xbe2;
    pub const COLOR_LOGIC_OP: u32 = 0xbf2;
    pub const SCISSOR_TEST: u32 = 0xc11;
    pub const PERSPECTIVE_CORRECTION_HINT: u32 = 0xc50;
    pub const UNPACK_SWAP_BYTES: u32 = 0xcf0;
    pub const UNPACK_LSB_FIRST: u32 = 0xcf1;
    pub const UNPACK_ROW_LENGTH: u32 = 0xcf2;
    pub const UNPACK_SKIP_ROWS: u32 = 0xcf3;
    pub const UNPACK_SKIP_PIXELS: u32 = 0xcf4;
    pub const UNPACK_ALIGNMENT: u32 = 0xcf5;
    pub const PACK_SWAP_BYTES: u32 = 0xd00;
    pub const PACK_LSB_FIRST: u32 = 0xd01;
    pub const PACK_ROW_LENGTH: u32 = 0xd02;
    pub const PACK_SKIP_ROWS: u32 = 0xd03;
    pub const PACK_SKIP_PIXELS: u32 = 0xd04;
    pub const PACK_ALIGNMENT: u32 = 0xd05;
    pub const MATRIX_MODE: u32 = 0xba0;
    pub const MODELVIEW_MATRIX: u32 = 0xba6;
    pub const PROJECTION_MATRIX: u32 = 0xba7;
    pub const TEXTURE_MATRIX: u32 = 0xba8;
    pub const MODELVIEW_STACK_DEPTH: u32 = 0xba3;
    pub const PROJECTION_STACK_DEPTH: u32 = 0xba4;
    pub const TEXTURE_STACK_DEPTH: u32 = 0xba5;

    pub const COLOR_BUFFER_BIT: u32 = 0x4000;
    pub const DEPTH_BUFFER_BIT: u32 = 0x100;
    pub const STENCIL_BUFFER_BIT: u32 = 0x400;
    pub const ACCUM_BUFFER_BIT: u32 = 0x200;

    pub const RED_SCALE: u32 = 0xd14;
    pub const RED_BIAS: u32 = 0xd15;
    pub const GREEN_SCALE: u32 = 0xd18;
    pub const GREEN_BIAS: u32 = 0xd19;
    pub const BLUE_SCALE: u32 = 0xd1a;
    pub const BLUE_BIAS: u32 = 0xd1b;
    pub const ALPHA_SCALE: u32 = 0xd1c;
    pub const ALPHA_BIAS: u32 = 0xd1d;

    /// PIXELFORMATDESCRIPTOR flags
    pub const PFD_DOUBLEBUFFER: u32 = 1;
    pub const PFD_DRAW_TO_WINDOW: u32 = 4;
    pub const PFD_DRAW_TO_BITMAP: u32 = 8;
    pub const PFD_SUPPORT_GDI: u32 = 0x10;
    pub const PFD_SUPPORT_OPENGL: u32 = 0x20;
}

/// glGet's parameter names (the state of OpenGL 1.1, gl.h's values), with the number of values
/// each returns by glGet's documentation
pub const PNAMES: &[(&str, u32, usize)] = &[
    ("CURRENT_COLOR", 0xb00, 4), ("CURRENT_INDEX", 0xb01, 1), ("CURRENT_NORMAL", 0xb02, 3), ("CURRENT_TEXTURE_COORDS", 0xb03, 4),
    ("CURRENT_RASTER_COLOR", 0xb04, 4), ("CURRENT_RASTER_INDEX", 0xb05, 1), ("CURRENT_RASTER_TEXTURE_COORDS", 0xb06, 4),
    ("CURRENT_RASTER_POSITION", 0xb07, 4), ("CURRENT_RASTER_POSITION_VALID", 0xb08, 1), ("CURRENT_RASTER_DISTANCE", 0xb09, 1),
    ("POINT_SMOOTH", 0xb10, 1), ("POINT_SIZE", 0xb11, 1), ("POINT_SIZE_RANGE", 0xb12, 2), ("POINT_SIZE_GRANULARITY", 0xb13, 1),
    ("LINE_SMOOTH", 0xb20, 1), ("LINE_WIDTH", 0xb21, 1), ("LINE_WIDTH_RANGE", 0xb22, 2), ("LINE_WIDTH_GRANULARITY", 0xb23, 1),
    ("LINE_STIPPLE", 0xb24, 1), ("LINE_STIPPLE_PATTERN", 0xb25, 1), ("LINE_STIPPLE_REPEAT", 0xb26, 1),
    ("LIST_MODE", 0xb30, 1), ("MAX_LIST_NESTING", 0xb31, 1), ("LIST_BASE", 0xb32, 1), ("LIST_INDEX", 0xb33, 1),
    ("POLYGON_MODE", 0xb40, 2), ("POLYGON_SMOOTH", 0xb41, 1), ("POLYGON_STIPPLE", 0xb42, 1), ("EDGE_FLAG", 0xb43, 1),
    ("CULL_FACE", 0xb44, 1), ("CULL_FACE_MODE", 0xb45, 1), ("FRONT_FACE", 0xb46, 1),
    ("LIGHTING", 0xb50, 1), ("LIGHT_MODEL_LOCAL_VIEWER", 0xb51, 1), ("LIGHT_MODEL_TWO_SIDE", 0xb52, 1), ("LIGHT_MODEL_AMBIENT", 0xb53, 4),
    ("SHADE_MODEL", 0xb54, 1), ("COLOR_MATERIAL_FACE", 0xb55, 1), ("COLOR_MATERIAL_PARAMETER", 0xb56, 1), ("COLOR_MATERIAL", 0xb57, 1),
    ("FOG", 0xb60, 1), ("FOG_INDEX", 0xb61, 1), ("FOG_DENSITY", 0xb62, 1), ("FOG_START", 0xb63, 1), ("FOG_END", 0xb64, 1),
    ("FOG_MODE", 0xb65, 1), ("FOG_COLOR", 0xb66, 4),
    ("DEPTH_RANGE", 0xb70, 2), ("DEPTH_TEST", 0xb71, 1), ("DEPTH_WRITEMASK", 0xb72, 1), ("DEPTH_CLEAR_VALUE", 0xb73, 1), ("DEPTH_FUNC", 0xb74, 1),
    ("ACCUM_CLEAR_VALUE", 0xb80, 4),
    ("STENCIL_TEST", 0xb90, 1), ("STENCIL_CLEAR_VALUE", 0xb91, 1), ("STENCIL_FUNC", 0xb92, 1), ("STENCIL_VALUE_MASK", 0xb93, 1),
    ("STENCIL_FAIL", 0xb94, 1), ("STENCIL_PASS_DEPTH_FAIL", 0xb95, 1), ("STENCIL_PASS_DEPTH_PASS", 0xb96, 1), ("STENCIL_REF", 0xb97, 1),
    ("STENCIL_WRITEMASK", 0xb98, 1),
    ("MATRIX_MODE", 0xba0, 1), ("NORMALIZE", 0xba1, 1), ("VIEWPORT", 0xba2, 4), ("MODELVIEW_STACK_DEPTH", 0xba3, 1),
    ("PROJECTION_STACK_DEPTH", 0xba4, 1), ("TEXTURE_STACK_DEPTH", 0xba5, 1), ("MODELVIEW_MATRIX", 0xba6, 16), ("PROJECTION_MATRIX", 0xba7, 16),
    ("TEXTURE_MATRIX", 0xba8, 16), ("ATTRIB_STACK_DEPTH", 0xbb0, 1), ("CLIENT_ATTRIB_STACK_DEPTH", 0xbb1, 1),
    ("ALPHA_TEST", 0xbc0, 1), ("ALPHA_TEST_FUNC", 0xbc1, 1), ("ALPHA_TEST_REF", 0xbc2, 1), ("DITHER", 0xbd0, 1),
    ("BLEND_DST", 0xbe0, 1), ("BLEND_SRC", 0xbe1, 1), ("BLEND", 0xbe2, 1), ("LOGIC_OP_MODE", 0xbf0, 1), ("INDEX_LOGIC_OP", 0xbf1, 1),
    ("COLOR_LOGIC_OP", 0xbf2, 1), ("AUX_BUFFERS", 0xc00, 1), ("DRAW_BUFFER", 0xc01, 1), ("READ_BUFFER", 0xc02, 1),
    ("SCISSOR_BOX", 0xc10, 4), ("SCISSOR_TEST", 0xc11, 1), ("INDEX_CLEAR_VALUE", 0xc20, 1), ("INDEX_WRITEMASK", 0xc21, 1),
    ("COLOR_CLEAR_VALUE", 0xc22, 4), ("COLOR_WRITEMASK", 0xc23, 4), ("INDEX_MODE", 0xc30, 1), ("RGBA_MODE", 0xc31, 1),
    ("DOUBLEBUFFER", 0xc32, 1), ("STEREO", 0xc33, 1), ("RENDER_MODE", 0xc40, 1),
    ("PERSPECTIVE_CORRECTION_HINT", 0xc50, 1), ("POINT_SMOOTH_HINT", 0xc51, 1), ("LINE_SMOOTH_HINT", 0xc52, 1),
    ("POLYGON_SMOOTH_HINT", 0xc53, 1), ("FOG_HINT", 0xc54, 1),
    ("TEXTURE_GEN_S", 0xc60, 1), ("TEXTURE_GEN_T", 0xc61, 1), ("TEXTURE_GEN_R", 0xc62, 1), ("TEXTURE_GEN_Q", 0xc63, 1),
    ("PIXEL_MAP_I_TO_I_SIZE", 0xcb0, 1), ("PIXEL_MAP_S_TO_S_SIZE", 0xcb1, 1), ("PIXEL_MAP_I_TO_R_SIZE", 0xcb2, 1),
    ("PIXEL_MAP_I_TO_G_SIZE", 0xcb3, 1), ("PIXEL_MAP_I_TO_B_SIZE", 0xcb4, 1), ("PIXEL_MAP_I_TO_A_SIZE", 0xcb5, 1),
    ("PIXEL_MAP_R_TO_R_SIZE", 0xcb6, 1), ("PIXEL_MAP_G_TO_G_SIZE", 0xcb7, 1), ("PIXEL_MAP_B_TO_B_SIZE", 0xcb8, 1),
    ("PIXEL_MAP_A_TO_A_SIZE", 0xcb9, 1),
    ("UNPACK_SWAP_BYTES", 0xcf0, 1), ("UNPACK_LSB_FIRST", 0xcf1, 1), ("UNPACK_ROW_LENGTH", 0xcf2, 1), ("UNPACK_SKIP_ROWS", 0xcf3, 1),
    ("UNPACK_SKIP_PIXELS", 0xcf4, 1), ("UNPACK_ALIGNMENT", 0xcf5, 1),
    ("PACK_SWAP_BYTES", 0xd00, 1), ("PACK_LSB_FIRST", 0xd01, 1), ("PACK_ROW_LENGTH", 0xd02, 1), ("PACK_SKIP_ROWS", 0xd03, 1),
    ("PACK_SKIP_PIXELS", 0xd04, 1), ("PACK_ALIGNMENT", 0xd05, 1),
    ("MAP_COLOR", 0xd10, 1), ("MAP_STENCIL", 0xd11, 1), ("INDEX_SHIFT", 0xd12, 1), ("INDEX_OFFSET", 0xd13, 1),
    ("RED_SCALE", 0xd14, 1), ("RED_BIAS", 0xd15, 1), ("ZOOM_X", 0xd16, 1), ("ZOOM_Y", 0xd17, 1), ("GREEN_SCALE", 0xd18, 1),
    ("GREEN_BIAS", 0xd19, 1), ("BLUE_SCALE", 0xd1a, 1), ("BLUE_BIAS", 0xd1b, 1), ("ALPHA_SCALE", 0xd1c, 1), ("ALPHA_BIAS", 0xd1d, 1),
    ("DEPTH_SCALE", 0xd1e, 1), ("DEPTH_BIAS", 0xd1f, 1),
    ("MAX_EVAL_ORDER", 0xd30, 1), ("MAX_LIGHTS", 0xd31, 1), ("MAX_CLIP_PLANES", 0xd32, 1), ("MAX_TEXTURE_SIZE", 0xd33, 1),
    ("MAX_PIXEL_MAP_TABLE", 0xd34, 1), ("MAX_ATTRIB_STACK_DEPTH", 0xd35, 1), ("MAX_MODELVIEW_STACK_DEPTH", 0xd36, 1),
    ("MAX_NAME_STACK_DEPTH", 0xd37, 1), ("MAX_PROJECTION_STACK_DEPTH", 0xd38, 1), ("MAX_TEXTURE_STACK_DEPTH", 0xd39, 1),
    ("MAX_VIEWPORT_DIMS", 0xd3a, 2), ("MAX_CLIENT_ATTRIB_STACK_DEPTH", 0xd3b, 1),
    ("SUBPIXEL_BITS", 0xd50, 1), ("INDEX_BITS", 0xd51, 1), ("RED_BITS", 0xd52, 1), ("GREEN_BITS", 0xd53, 1), ("BLUE_BITS", 0xd54, 1),
    ("ALPHA_BITS", 0xd55, 1), ("DEPTH_BITS", 0xd56, 1), ("STENCIL_BITS", 0xd57, 1), ("ACCUM_RED_BITS", 0xd58, 1),
    ("ACCUM_GREEN_BITS", 0xd59, 1), ("ACCUM_BLUE_BITS", 0xd5a, 1), ("ACCUM_ALPHA_BITS", 0xd5b, 1), ("NAME_STACK_DEPTH", 0xd70, 1),
    ("AUTO_NORMAL", 0xd80, 1),
    ("MAP1_COLOR_4", 0xd90, 1), ("MAP1_INDEX", 0xd91, 1), ("MAP1_NORMAL", 0xd92, 1), ("MAP1_TEXTURE_COORD_1", 0xd93, 1),
    ("MAP1_TEXTURE_COORD_2", 0xd94, 1), ("MAP1_TEXTURE_COORD_3", 0xd95, 1), ("MAP1_TEXTURE_COORD_4", 0xd96, 1), ("MAP1_VERTEX_3", 0xd97, 1),
    ("MAP1_VERTEX_4", 0xd98, 1), ("MAP2_COLOR_4", 0xdb0, 1), ("MAP2_INDEX", 0xdb1, 1), ("MAP2_NORMAL", 0xdb2, 1),
    ("MAP2_TEXTURE_COORD_1", 0xdb3, 1), ("MAP2_TEXTURE_COORD_2", 0xdb4, 1), ("MAP2_TEXTURE_COORD_3", 0xdb5, 1),
    ("MAP2_TEXTURE_COORD_4", 0xdb6, 1), ("MAP2_VERTEX_3", 0xdb7, 1), ("MAP2_VERTEX_4", 0xdb8, 1),
    ("MAP1_GRID_DOMAIN", 0xdd0, 2), ("MAP1_GRID_SEGMENTS", 0xdd1, 1), ("MAP2_GRID_DOMAIN", 0xdd2, 4), ("MAP2_GRID_SEGMENTS", 0xdd3, 2),
    ("TEXTURE_1D", 0xde0, 1), ("TEXTURE_2D", 0xde1, 1), ("FEEDBACK_BUFFER_SIZE", 0xdf1, 1), ("FEEDBACK_BUFFER_TYPE", 0xdf2, 1),
    ("SELECTION_BUFFER_SIZE", 0xdf4, 1),
    ("POLYGON_OFFSET_UNITS", 0x2a00, 1), ("POLYGON_OFFSET_POINT", 0x2a01, 1), ("POLYGON_OFFSET_LINE", 0x2a02, 1),
    ("CLIP_PLANE0", 0x3000, 1), ("CLIP_PLANE5", 0x3005, 1), ("LIGHT0", 0x4000, 1), ("LIGHT7", 0x4007, 1),
    ("POLYGON_OFFSET_FILL", 0x8037, 1), ("POLYGON_OFFSET_FACTOR", 0x8038, 1),
    ("TEXTURE_BINDING_1D", 0x8068, 1), ("TEXTURE_BINDING_2D", 0x8069, 1),
    ("VERTEX_ARRAY", 0x8074, 1), ("NORMAL_ARRAY", 0x8075, 1), ("COLOR_ARRAY", 0x8076, 1), ("INDEX_ARRAY", 0x8077, 1),
    ("TEXTURE_COORD_ARRAY", 0x8078, 1), ("EDGE_FLAG_ARRAY", 0x8079, 1), ("VERTEX_ARRAY_SIZE", 0x807a, 1), ("VERTEX_ARRAY_TYPE", 0x807b, 1),
    ("VERTEX_ARRAY_STRIDE", 0x807c, 1), ("NORMAL_ARRAY_TYPE", 0x807e, 1), ("NORMAL_ARRAY_STRIDE", 0x807f, 1), ("COLOR_ARRAY_SIZE", 0x8081, 1),
    ("COLOR_ARRAY_TYPE", 0x8082, 1), ("COLOR_ARRAY_STRIDE", 0x8083, 1), ("INDEX_ARRAY_TYPE", 0x8085, 1), ("INDEX_ARRAY_STRIDE", 0x8086, 1),
    ("TEXTURE_COORD_ARRAY_SIZE", 0x8088, 1), ("TEXTURE_COORD_ARRAY_TYPE", 0x8089, 1), ("TEXTURE_COORD_ARRAY_STRIDE", 0x808a, 1),
    ("EDGE_FLAG_ARRAY_STRIDE", 0x808c, 1),
];

/// the capabilities glEnable / glIsEnabled take (OpenGL 1.1)
pub const CAPS: &[u32] = &[
    0xb10, 0xb20, 0xb24, 0xb41, 0xb42, 0xb44, 0xb50, 0xb57, 0xb60, 0xb71, 0xb90, 0xba1, 0xbc0, 0xbd0, 0xbe2, 0xbf1, 0xbf2, 0xc11,
    0xc60, 0xc61, 0xc62, 0xc63, 0xd80, 0xd90, 0xd91, 0xd92, 0xd93, 0xd94, 0xd95, 0xd96, 0xd97, 0xd98, 0xdb0, 0xdb1, 0xdb2, 0xdb3,
    0xdb4, 0xdb5, 0xdb6, 0xdb7, 0xdb8, 0xde0, 0xde1, 0x2a01, 0x2a02, 0x3000, 0x3001, 0x3002, 0x3003, 0x3004, 0x3005, 0x4000, 0x4001,
    0x4002, 0x4003, 0x4004, 0x4005, 0x4006, 0x4007, 0x8037, 0x8074, 0x8075, 0x8076, 0x8077, 0x8078, 0x8079,
];

pub fn pname(name: &str) -> u32 {
    PNAMES.iter().find(|p| p.0 == name).map(|p| p.1).unwrap_or_else(|| panic!("{name}"))
}
