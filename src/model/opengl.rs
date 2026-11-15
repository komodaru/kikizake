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
    pub const LINES: u32 = 1;
    pub const LINE_LOOP: u32 = 2;
    pub const LINE_STRIP: u32 = 3;
    pub const LINE_SMOOTH: u32 = 0xb20;
    pub const LINE_STIPPLE: u32 = 0xb24;

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

    pub const FRONT_LEFT: u32 = 0x400;
    pub const FRONT: u32 = 0x404;
    pub const LEFT: u32 = 0x406;
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
    pub const FOG: u32 = 0xb60;
    pub const FOG_INDEX: u32 = 0xb61;
    pub const FOG_DENSITY: u32 = 0xb62;
    pub const FOG_START: u32 = 0xb63;
    pub const FOG_END: u32 = 0xb64;
    pub const FOG_MODE: u32 = 0xb65;
    pub const FOG_COLOR: u32 = 0xb66;
    pub const POLYGON_OFFSET_FILL: u32 = 0x8037;
    pub const LIGHTING: u32 = 0xb50;
    pub const COLOR_MATERIAL: u32 = 0xb57;
    pub const POINT_SMOOTH: u32 = 0xb10;
    pub const POLYGON_SMOOTH: u32 = 0xb41;
    pub const MAP_COLOR: u32 = 0xd10;
    pub const INDEX_SHIFT: u32 = 0xd12;
    pub const INDEX_OFFSET: u32 = 0xd13;
    pub const DEPTH_SCALE: u32 = 0xd1e;
    pub const DEPTH_BIAS: u32 = 0xd1f;
    pub const MAP_STENCIL: u32 = 0xd11;
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
    pub const INDEX_LOGIC_OP: u32 = 0xbf1;
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

use crate::spec::open;
use crate::spec::clause;
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;
use e::*;

/// a pixel format: what DescribePixelFormat gives (the fields `glpf` prints)
#[derive(Clone, Debug, PartialEq)]
pub struct Pfd {
    pub flags: u32,
    pub ty: u32,
    pub color: u32,
    /// (bits, shift) of red, green, blue, alpha
    pub rgba: [(u32, u32); 4],
    pub accum: u32,
    pub accum_rgba: [u32; 4],
    pub depth: u32,
    pub stencil: u32,
}

impl Pfd {
    /// the fields `glpf` prints after the index
    pub fn describe(&self) -> String {
        let [r, g, b, a] = self.rgba;
        let [ar, ag, ab, aa] = self.accum_rgba;
        format!("{:x} {} {} {},{} {},{} {},{} {},{} accum {} {ar},{ag},{ab},{aa} depth {} stencil {} aux 0 0 0 0 0 0", self.flags, self.ty, self.color, r.0, r.1, g.0, g.1, b.0, b.1, a.0, a.1, self.accum, self.depth, self.stencil)
    }
}

/// the generic implementation's pixel formats (MEASURED, golden opengl-32 pf-*: the same 36 on
/// DIBs of every depth): the display's (32 bits, on bitmaps and windows, single or double
/// buffered), then 24, 16, 8 and 4 bits (bitmaps only); each RGBA without alpha, RGBA with 8
/// bits of alpha, color index; each with depth 32 then 16, stencil 8
pub fn formats() -> Vec<Pfd> {
    let mut v = Vec::new();
    for (color, rgb, accum, acc_rgb, acc_a) in [
        (32, [(8, 16), (8, 8), (8, 0)], 64, [16, 16, 16], 16),
        (24, [(8, 16), (8, 8), (8, 0)], 64, [16, 16, 16], 16),
        (16, [(5, 10), (5, 5), (5, 0)], 32, [11, 11, 10], 8),
        (8, [(3, 0), (3, 3), (2, 6)], 32, [11, 11, 10], 8),
        (4, [(1, 0), (1, 1), (1, 2)], 16, [5, 6, 5], 4),
    ] {
        for kind in 0..3 {
            for double in [false, true] {
                if double && color != 32 {
                    continue;
                }
                for depth in [32, 16] {
                    let alpha = kind == 1;
                    let ty = (kind == 2) as u32;
                    let (accum, accum_rgba) = match kind {
                        0 => (accum, [acc_rgb[0], acc_rgb[1], acc_rgb[2], 0]),
                        1 => (accum, [acc_a; 4]),
                        _ => (0, [0; 4]),
                    };
                    let flags = match (color, double) {
                        (32, false) => 0x807c,
                        (32, true) => 0x8465,
                        _ => 0x78,
                    };
                    v.push(Pfd { flags, ty, color, rgba: [rgb[0], rgb[1], rgb[2], if alpha { (8, 0) } else { (0, 0) }], accum, accum_rgba, depth, stencil: 8 });
                }
            }
        }
    }
    v
}

/// ChoosePixelFormat (MEASURED, golden opengl-32 choose-*, a weighted mismatch fitted on all 292
/// requests): of the formats that draw to a window if that is asked (the other flags,
/// double buffering included, are not looked at), the one with the least penalty, the first on a
/// tie: 3 for less accumulation than asked, 1 for more, 2 for another pixel type, 1 for other
/// color bits than asked, 2 for less alpha, 7 for less depth, and a deeper depth buffer 1 for each
/// 16 bits past the request. `req`: flags, type, color, alpha, accum, depth, stencil.
pub fn choose(req: &[i64]) -> usize {
    hit(clause::WGL_CHOOSE);
    let (flags, ty, color, alpha, accum, depth) = (req[0] as u32, req[1] as u32, req[2] as u32, req[3] as u32, req[4] as u32, req[5] as u32);
    let mut best = (u32::MAX, 0);
    for (i, f) in formats().iter().enumerate() {
        if flags & PFD_DRAW_TO_WINDOW != 0 && f.flags & PFD_DRAW_TO_WINDOW == 0 {
            continue;
        }
        let mut p = 0;
        if accum > 0 && f.accum < accum {
            p += 3;
        }
        if accum > 0 && f.accum > accum {
            p += 1;
        }
        if f.ty != ty {
            p += 2;
        }
        if color > 0 && f.color != color {
            p += 1;
        }
        if f.rgba[3].0 < alpha {
            p += 2;
        }
        if f.depth < depth {
            p += 7;
        } else {
            p += (f.depth - depth) / 16;
        }
        if p < best.0 {
            best = (p, i + 1);
        }
    }
    best.1
}

/// a value of the state as glGet sees it
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum V {
    /// an integer (or enum)
    I(i64),
    /// a float state value
    F(f32),
    /// a color component, normal or depth: glGetIntegerv maps it linearly (1.0 to the largest integer)
    C(f32),
    B(bool),
}

const W: usize = 16;
const H: usize = 16;

/// the state of a context (glGet's table of initial values)
#[derive(Clone, Debug)]
pub struct Ctx {
    pub pf: Pfd,
    pub err: u32,
    pub in_begin: Option<u32>,
    pub caps: std::collections::BTreeMap<u32, bool>,
    pub mode: u32,
    /// the matrix stacks, column-major, the top last: modelview, projection, texture
    pub stacks: [Vec<[f32; 16]>; 3],
    pub clear_color: [f32; 4],
    /// a GLclampd, kept as a double (golden opengl-32 depth-types-0: 0.999 clears to 0xffbe76c6)
    pub clear_depth: f64,
    pub clear_stencil: i64,
    pub color: [f32; 4],
    pub blend: (u32, u32),
    pub logic_op: u32,
    pub depth_func: u32,
    pub depth_mask: bool,
    pub depth_range: [f32; 2],
    pub alpha: (u32, f32),
    pub stencil_func: (u32, i64, u32),
    pub stencil_op: [u32; 3],
    pub stencil_mask: u32,
    /// glDrawBuffer, glReadBuffer
    pub buffers: [u32; 2],
    /// the attribute stack (glPushAttrib): each entry the mask and the state when pushed
    pub attrib: Vec<(u32, Ctx)>,
    pub clear_accum: [f32; 4],
    /// color index mode (GL.COLORINDEX): the current index, the clear index, the index write mask
    pub index: f32,
    pub clear_index: f32,
    pub index_mask: u32,
    /// the accumulation buffer, signed channels at the format's accumulation bits; None while its
    /// content is still undefined (nothing loaded or cleared into it yet)
    pub acc: Vec<[Option<i64>; 4]>,
    /// which framebuffer channels hold undefined content (GL.UNDEFINED): a new alpha buffer's,
    /// and whatever is made from undefined content; a read gives "??" for their bytes
    pub fb_undefined: Vec<[bool; 4]>,
    pub color_mask: [bool; 4],
    /// the GLboolean bytes glColorMask was given (glGet gives them back as they are: GL.COLORMASK)
    pub color_mask_raw: [u32; 4],
    pub viewport: [i64; 4],
    pub scissor: [i64; 4],
    pub shade: u32,
    pub hints: [u32; 5],
    /// PACK_SWAP_BYTES.. PACK_ALIGNMENT (0xd00..0xd05), UNPACK (0xcf0..0xcf5)
    pub pack: [i64; 6],
    pub unpack: [i64; 6],
    /// RED_SCALE, RED_BIAS, GREEN_*, BLUE_*, ALPHA_* (scale, bias)
    pub transfer: [(f32, f32); 4],
    /// GL_MAP_COLOR, GL_MAP_STENCIL (glPixelTransfer): the pixel maps the model does not follow
    pub map: [bool; 2],
    /// GL_INDEX_SHIFT, GL_INDEX_OFFSET
    pub index_shift: (i64, i64),
    /// GL_DEPTH_SCALE, GL_DEPTH_BIAS
    pub depth_transfer: (f32, f32),
    /// glPixelZoom
    pub zoom: (f32, f32),
    /// the current raster position in window coordinates, None when invalid (initially the window
    /// origin at depth 0), and its color
    pub raster: Option<[f64; 3]>,
    pub raster_color: [f32; 4],
    /// the raster position's eye z, for fog on images
    pub raster_eye: f64,
    /// glFog
    pub fog: Fog,
    /// the current normal (glNormal3f)
    pub normal: [f32; 3],
    /// glPolygonOffset: (factor, units)
    pub offset: (f32, f32),
    pub line_width: f32,
    pub point_size: f32,
    pub cull_mode: u32,
    pub front_face: u32,
    pub poly_mode: [u32; 2],
    /// the framebuffer, row 0 at the bottom: color channels at the format's bits, depth, stencil
    pub fb: Vec<[u32; 4]>,
    pub depth: Vec<Depth>,
    pub stencil: Vec<u32>,
    /// which primitive the fragments come from, for GL_NEVER (GL.DEPTH.NEVER.CLIP)
    pub never: NeverPath,
    /// an unclipped rectangle has been drawn under GL_NEVER (GL.DEPTH.NEVER.CLIP)
    pub never_primed: bool,
    /// the quad being drawn is a glRect's
    pub rect_call: bool,
    /// the DIB's bits a pixel
    pub bpp: u32,
    /// the vertices since glBegin: object position, color
    pub verts: Vec<([f64; 3], [f32; 4])>,
}

pub const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

impl Ctx {
    pub fn new(pf: Pfd, bpp: u32) -> Ctx {
        hit(clause::GL_INIT);
        let stencil_bits = pf.stencil;
        let alpha_init = u32::from(pf.rgba[3].0 != 0);
        let index_mask = if pf.ty != 0 { low_bits(pf.color) } else { 0 };
        let mut caps = std::collections::BTreeMap::new();
        for c in CAPS {
            caps.insert(*c, *c == DITHER);
        }
        Ctx {
            pf,
            err: NO_ERROR,
            in_begin: None,
            caps,
            mode: MODELVIEW,
            stacks: [vec![IDENTITY], vec![IDENTITY], vec![IDENTITY]],
            clear_color: [0.0; 4],
            clear_depth: 1.0,
            clear_stencil: 0,
            color: [1.0; 4],
            blend: (ONE, ZERO),
            logic_op: COPY,
            depth_func: LESS,
            depth_mask: true,
            depth_range: [0.0, 1.0],
            alpha: (ALWAYS, 0.0),
            // (MEASURED, golden opengl-32 defaults-*: the value mask starts as the stencil bits)
            stencil_func: (ALWAYS, 0, ((1u64 << stencil_bits) - 1) as u32),
            stencil_op: [KEEP; 3],
            stencil_mask: ((1u64 << stencil_bits) - 1) as u32,
            buffers: [FRONT; 2],
            attrib: Vec::new(),
            clear_accum: [0.0; 4],
            index: 1.0,
            clear_index: 0.0,
            index_mask,
            acc: vec![[None; 4]; W * H],
            fb_undefined: vec![[false, false, false, alpha_init != 0]; W * H],
            color_mask: [true; 4],
            color_mask_raw: [1; 4],
            viewport: [0, 0, W as i64, H as i64],
            scissor: [0, 0, W as i64, H as i64],
            shade: SMOOTH,
            hints: [DONT_CARE; 5],
            pack: [0, 0, 0, 0, 0, 4],
            unpack: [0, 0, 0, 0, 0, 4],
            transfer: [(1.0, 0.0); 4],
            map: [false; 2],
            index_shift: (0, 0),
            depth_transfer: (1.0, 0.0),
            zoom: (1.0, 1.0),
            raster: Some([0.0; 3]),
            raster_color: [1.0; 4],
            raster_eye: 0.0,
            fog: Fog { mode: FogMode::Exp, density: 1.0, start: 0.0, end: 1.0, index: 0.0, color: [0.0; 4] },
            offset: (0.0, 0.0),
            normal: [0.0, 0.0, 1.0],
            line_width: 1.0,
            point_size: 1.0,
            cull_mode: BACK,
            front_face: CCW,
            poly_mode: [0x1b02, 0x1b02],
            // (MEASURED, golden opengl-32 / 64 edge-blend-alpha-buffer: a new alpha buffer holds
            // whatever its memory held, 1 in every pixel on one executor and scattered bytes on
            // the other: GL.UNDEFINED; the color buffer the DIB's zeros)
            fb: vec![[0; 4]; W * H],
            depth: vec![Depth(0); W * H],
            stencil: vec![0; W * H],
            never: NeverPath::Other,
            never_primed: false,
            rect_call: false,
            bpp,
            verts: Vec::new(),
        }
    }

    /// record an error: only the first since the last glGetError is kept (GL.ERROR.ONE)
    pub fn error(&mut self, e: u32) {
        if self.err == NO_ERROR {
            self.err = e;
        } else {
            hit(clause::GL_ERROR_ONE);
        }
    }
}

/// the depths of the modelview, projection and texture matrix stacks (GL.LIMITS)
pub const STACK_DEPTH: [usize; 3] = [32, 10, 10];

/// the implementation's limits and constant answers (MEASURED, golden opengl-32: see the
/// clauses GL.LIMITS)
pub fn limits(p: u32) -> Option<Vec<V>> {
    use V::*;
    let i = |v: i64| vec![I(v)];
    Some(match p {
        0xb12 | 0xb22 => vec![F(0.5), F(10.0)],
        0xb13 | 0xb23 => vec![F(0.125)],
        0xb31 => i(64),
        0xd30 => i(30),
        0xd31 => i(8),
        0xd32 => i(6),
        0xd33 => i(1024),
        0xd34 => i(65536),
        0xd35 | 0xd3b => i(16),
        0xd36 => i(STACK_DEPTH[0] as i64),
        0xd37 => i(128),
        0xd38 => i(STACK_DEPTH[1] as i64),
        0xd39 => i(STACK_DEPTH[2] as i64),
        0xd3a => vec![I(16384), I(16384)],
        0xd50 => i(3),
        _ => return None,
    })
}

impl Ctx {
    fn cap(&self, c: u32) -> bool {
        self.caps.get(&c).copied().unwrap_or(false)
    }
    fn top(&self, m: usize) -> &[f32; 16] {
        self.stacks[m].last().unwrap()
    }

    /// the values glGet gives for `p`, None for a name it does not know (INVALID_ENUM)
    pub fn get(&self, p: u32) -> Res<Option<Vec<V>>> {
        use V::*;
        if self.caps.contains_key(&p) && !(0x3000..=0x4007).contains(&p) || [0x3000, 0x3005, 0x4000, 0x4007].contains(&p) {
            hit(clause::GL_GET_CAP);
            return Ok(Some(vec![B(self.cap(p))]));
        }
        let c4 = |v: [f32; 4]| v.iter().map(|x| C(*x)).collect::<Vec<_>>();
        let i = |v: i64| vec![I(v)];
        let pf = &self.pf;
        hit(clause::GL_GET_STATE);
        Ok(Some(match p {
            0xb00 => c4(self.color),
            0xb01 => vec![F(self.index)],
            0xb02 => self.normal.iter().map(|x| C(*x)).collect(),
            0xb03 | 0xb06 => vec![F(0.0), F(0.0), F(0.0), F(1.0)],
            0xb04 => c4([1.0; 4]),
            0xb05 => vec![F(1.0)],
            0xb07 => vec![F(0.0), F(0.0), F(0.0), F(1.0)],
            0xb08 => vec![B(true)],
            0xb09 => vec![F(0.0)],
            0xb11 => vec![F(self.point_size)],
            0xb21 => vec![F(self.line_width)],
            0xb25 => i(0xffff),
            0xb26 => i(1),
            0xb30 | 0xb32 | 0xb33 => i(0),
            0xb40 => self.poly_mode.iter().map(|m| I(*m as i64)).collect(),
            0xb43 => vec![B(true)],
            0xb45 => i(self.cull_mode as i64),
            0xb46 => i(self.front_face as i64),
            0xb51 | 0xb52 => vec![B(false)],
            0xb53 => {
                // (MEASURED, golden opengl-32 defaults-*: the ambient 0.2 is the float 0x3e4ccccf,
                // two ulps above, with 8-bit channels, 0.2 itself with 5-bit ones)
                let a = f32::from_bits(if pf.rgba[0].0 == 8 { 0x3e4c_cccf } else { 0x3e4c_cccd });
                c4([a, a, a, 1.0])
            }
            0xb54 => i(self.shade as i64),
            0xb55 => i(FRONT_AND_BACK as i64),
            0xb56 => i(0x1602),
            0xb61 => vec![F(self.fog.index)],
            0xb62 => vec![F(self.fog.density)],
            0xb63 => vec![F(self.fog.start)],
            0xb64 => vec![F(self.fog.end)],
            0xb65 => i(i64::from(self.fog.mode.name())),
            0xb66 => {
                // (MEASURED, golden opengl-32 fog-rects-*: kept times each channel's largest value,
                // a missing alpha red's, and read back times its reciprocal: 0.668 gives
                // 0x3f2b020d on 8 bits, 0.941 0x3f70e560 on 5: GL.FOG.STATE)
                let b = self.bits();
                let mut v = self.fog.color;
                for k in 0..4 {
                    let m = ((1u64 << if b[k] == 0 { b[0] } else { b[k] }) - 1) as f32;
                    v[k] = v[k] * m * (1.0 / m);
                }
                c4(v)
            }
            0xb70 => vec![C(self.depth_range[0]), C(self.depth_range[1])],
            0xb72 => vec![B(self.depth_mask)],
            0xb73 => vec![C(self.clear_depth as f32)],
            0xb74 => i(self.depth_func as i64),
            0xb80 => c4(self.clear_accum),
            0xb91 => i(self.clear_stencil),
            0xb92 => i(self.stencil_func.0 as i64),
            0xb93 => i(self.stencil_func.2 as i32 as i64),
            0xb94 => i(self.stencil_op[0] as i64),
            0xb95 => i(self.stencil_op[1] as i64),
            0xb96 => i(self.stencil_op[2] as i64),
            0xb97 => i(self.stencil_func.1),
            0xb98 => i(self.stencil_mask as i32 as i64),
            0xba0 => i(self.mode as i64),
            0xba2 => self.viewport.iter().map(|v| I(*v)).collect(),
            0xba3..=0xba5 => i(self.stacks[(p - 0xba3) as usize].len() as i64),
            0xba6..=0xba8 => self.top((p - 0xba6) as usize).iter().map(|v| F(*v)).collect(),
            0xbb0 => i(self.attrib.len() as i64),
            0xbb1 => i(0),
            0xbc1 => i(self.alpha.0 as i64),
            0xbc2 => vec![F(self.alpha.1)],
            0xbe0 => i(self.blend.1 as i64),
            0xbe1 => i(self.blend.0 as i64),
            0xbf0 => i(self.logic_op as i64),
            0xc00 => i(0),
            0xc01 | 0xc02 => i(self.buffers[(p - 0xc01) as usize] as i64),
            0xc10 => self.scissor.iter().map(|v| I(*v)).collect(),
            0xc20 => vec![F(self.clear_index)],
            // (MEASURED, golden opengl-32 defaults-*: no index bits in RGBA mode, so no mask;
            // ci-*: all the index bits in color index mode)
            0xc21 => i(self.index_mask as i64),
            0xc22 => c4(self.clear_color),
            0xc23 => self.color_mask_raw.iter().map(|b| I(*b as i64)).collect(),
            0xc30 => vec![B(false)],
            0xc31 => vec![B(true)],
            0xc32 => vec![B(pf.flags & PFD_DOUBLEBUFFER != 0)],
            0xc33 => vec![B(false)],
            0xc40 => i(0x1c00),
            0xc50..=0xc54 => i(self.hints[(p - 0xc50) as usize] as i64),
            0xcb0..=0xcb9 => i(1),
            0xcf0..=0xcf5 => {
                let v = self.unpack[(p - 0xcf0) as usize];
                if p <= 0xcf1 { vec![B(v != 0)] } else { i(v) }
            }
            0xd00..=0xd05 => {
                let v = self.pack[(p - 0xd00) as usize];
                if p <= 0xd01 { vec![B(v != 0)] } else { i(v) }
            }
            0xd10 | 0xd11 => vec![B(false)],
            0xd12 | 0xd13 => i(0),
            0xd14..=0xd1d => {
                let k = [0xd14, 0xd18, 0xd1a, 0xd1c].iter().position(|x| *x == p & !1).unwrap_or(9);
                match p {
                    0xd16 | 0xd17 => vec![F(1.0)],
                    _ if k < 4 => vec![F(if p & 1 == 0 { self.transfer[k].0 } else { self.transfer[k].1 })],
                    _ => vec![F(if p == 0xd14 { self.transfer[0].0 } else { self.transfer[0].1 })],
                }
            }
            0xd1e => vec![F(1.0)],
            0xd1f => vec![F(0.0)],
            // (MEASURED, golden opengl-32 defaults-*: 8 in RGBA mode at every depth; ci-*: the
            // color bits in color index mode)
            0xd51 => i(if pf.ty != 0 { pf.color as i64 } else { 8 }),
            0xd52..=0xd55 => i(pf.rgba[(p - 0xd52) as usize].0 as i64),
            0xd56 => i(pf.depth as i64),
            0xd57 => i(pf.stencil as i64),
            0xd58..=0xd5b => i(pf.accum_rgba[(p - 0xd58) as usize] as i64),
            0xd70 => i(0),
            0xdd0 => vec![F(0.0), F(1.0)],
            0xdd1 => i(1),
            0xdd2 => vec![F(0.0), F(1.0), F(0.0), F(1.0)],
            0xdd3 => vec![I(1), I(1)],
            // (MEASURED, golden opengl-32 defaults-*: the feedback type reads 0, not GL_2D)
            0xdf1 | 0xdf2 | 0xdf4 => i(0),
            0x8038 => vec![F(self.offset.0)],
            0x2a00 => vec![F(self.offset.1)],
            0x8068 | 0x8069 => i(0),
            0x807a | 0x8088 => i(4),
            0x8081 => i(4),
            0x807b | 0x807e | 0x8082 | 0x8089 => i(FLOAT as i64),
            0x8085 => i(FLOAT as i64),
            0x807c | 0x807f | 0x8083 | 0x8086 | 0x808a | 0x808c => i(0),
            _ => match limits(p) {
                Some(v) => {
                    hit(clause::GL_LIMITS);
                    v
                }
                None => {
                    hit(clause::GL_GET_ENUM);
                    return Ok(None);
                }
            },
        }))
    }
}

/// glGetIntegerv of a color-like value (GL.GET.MAP)
/// (MEASURED, golden opengl-32 set-values-*: the component times the float 2147482496 in single
/// precision, truncated; out of range the conversion's "integer indefinite" 0x80000000)
pub fn color_to_int(c: f32) -> i64 {
    hit(clause::GL_GET_MAP);
    float_to_int(c * 2_147_482_496.0f32)
}

/// a float to an integer as the x86 conversion instructions do: truncated, 0x80000000 when out of range or NaN
pub fn float_to_int(v: f32) -> i64 {
    if v >= -2_147_483_648.0 && v < 2_147_483_648.0 {
        v.trunc() as i64
    } else {
        i32::MIN as i64
    }
}

/// one value as `glget <kind>` prints it
pub fn print_value(kind: &str, v: V) -> String {
    use V::*;
    let as_f = |v: V| -> f32 {
        match v {
            I(i) => i as f32,
            F(x) | C(x) => x,
            B(b) => b as i32 as f32,
        }
    };
    match kind {
        "i" => match v {
            I(i) => (i as i32).to_string(),
            F(x) => {
                hit(clause::GL_GET_ROUND);
                (x as f64).round().clamp(i32::MIN as f64, i32::MAX as f64).to_string()
            }
            C(x) => color_to_int(x).to_string(),
            B(b) => (b as i32).to_string(),
        },
        "f" => format!("{:08x}", as_f(v).to_bits()),
        "d" => format!("{:016x}", (as_f(v) as f64).to_bits()),
        _ => {
            let b = match v {
                I(i) => i != 0,
                F(x) | C(x) => x != 0.0,
                B(b) => b,
            };
            (b as i32).to_string()
        }
    }
}

/// the calls `gl` may name and their argument types (runner/opengl.c's table)
pub const CALLS: &[(&str, &str)] = &[
    ("glLoadIdentity", ""), ("glPushMatrix", ""), ("glPopMatrix", ""), ("glEnd", ""), ("glFlush", ""), ("glFinish", ""), ("glPopAttrib", ""),
    ("glEnable", "i"), ("glDisable", "i"), ("glMatrixMode", "i"), ("glBegin", "i"), ("glClear", "i"), ("glLogicOp", "i"), ("glDepthFunc", "i"),
    ("glShadeModel", "i"), ("glCullFace", "i"), ("glFrontFace", "i"), ("glStencilMask", "i"), ("glDepthMask", "i"), ("glPushAttrib", "i"),
    ("glDrawBuffer", "i"), ("glReadBuffer", "i"), ("glClearStencil", "i"), ("glIndexMask", "i"), ("glIndexi", "i"),
    ("glBlendFunc", "ii"), ("glPixelStorei", "ii"), ("glHint", "ii"), ("glPolygonMode", "ii"), ("glVertex2i", "ii"), ("glPixelTransferi", "ii"),
    ("glStencilOp", "iii"), ("glStencilFunc", "iii"), ("glColor3ub", "iii"), ("glVertex3i", "iii"),
    ("glViewport", "iiii"), ("glScissor", "iiii"), ("glColorMask", "iiii"), ("glColor4ub", "iiii"), ("glRecti", "iiii"),
    ("glLineWidth", "f"), ("glPointSize", "f"), ("glClearIndex", "f"),
    ("glVertex2f", "ff"), ("glPolygonOffset", "ff"),
    ("glTranslatef", "fff"), ("glScalef", "fff"), ("glColor3f", "fff"), ("glVertex3f", "fff"), ("glNormal3f", "fff"),
    ("glClearColor", "ffff"), ("glColor4f", "ffff"), ("glRotatef", "ffff"), ("glRectf", "ffff"), ("glClearAccum", "ffff"), ("glVertex4f", "ffff"),
    ("glClearDepth", "d"), ("glDepthRange", "dd"), ("glOrtho", "dddddd"), ("glFrustum", "dddddd"),
    ("glAlphaFunc", "if"), ("glPixelStoref", "if"), ("glPixelTransferf", "if"), ("glAccum", "if"), ("glFogf", "if"),
    ("glPixelZoom", "ff"), ("glRasterPos2i", "ii"), ("glRasterPos2f", "ff"),
];

/// an argument: an integer as the executor passes it (a C long, 32 bits), or a float
#[derive(Clone, Copy, Debug)]
enum A {
    I(i64),
    F(f64),
}

fn int(a: &[A], i: usize) -> i64 {
    match a[i] {
        A::I(v) => v as u32 as i32 as i64,
        A::F(v) => v as i64,
    }
}
fn u(a: &[A], i: usize) -> u32 {
    int(a, i) as u32
}
fn fl(a: &[A], i: usize) -> f64 {
    match a[i] {
        A::F(v) => v,
        A::I(v) => v as f64,
    }
}

fn parse_float(t: &str) -> Res<f64> {
    t.parse::<f64>().map_err(|_| Fail::Bad)
}

/// the opengl model of one case: the context the case made
#[derive(Default)]
pub struct Gl {
    pub ctx: Option<Ctx>,
}

fn clamp01(v: f64) -> f32 {
    v.clamp(0.0, 1.0) as f32
}

fn mat_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut r = [0f32; 16];
    for c in 0..4 {
        for row in 0..4 {
            // (the four products summed from the first: no +0.0 to start from, so zeros keep their sign)
            let p = |k: usize| a[k * 4 + row] * b[c * 4 + k];
            let s = p(0) + p(1) + p(2) + p(3);
            r[c * 4 + row] = s;
        }
    }
    r
}

const BEGIN_OK: &[&str] = &["glVertex2f", "glVertex3f", "glVertex4f", "glVertex2i", "glVertex3i", "glColor4ub", "glColor3ub", "glColor3f", "glColor4f", "glNormal3f", "glIndexi", "glEnd"];

impl Gl {
    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let name = t[0].as_str();
        match name {
            "glpf" => {
                let bpp = t.get(1).and_then(|v| num(v)).ok_or(Fail::Bad)?;
                let f = formats();
                hit(clause::WGL_FORMATS);
                out.push(format!("glpf {bpp} n {}", f.len()));
                for (i, p) in f.iter().enumerate() {
                    out.push(format!("pf {} {}", i + 1, p.describe()));
                }
                return Ok(true);
            }
            "glchoose" => {
                let v: Option<Vec<i64>> = t[1..].iter().map(|x| num(x)).collect();
                let v = v.ok_or(Fail::Bad)?;
                if v.len() != 8 {
                    return Err(Fail::Bad);
                }
                out.push(format!("glchoose {}", choose(&v[1..])));
                return Ok(true);
            }
            "glctx" => {
                self.ctx = None;
                let bpp = t.get(1).and_then(|v| num(v)).ok_or(Fail::Bad)? as u32;
                let pf = if t.get(2).is_some_and(|v| v == "choose") {
                    let v: Option<Vec<i64>> = t[3..].iter().map(|x| num(x)).collect();
                    let v = v.ok_or(Fail::Bad)?;
                    if v.len() != 7 {
                        return Err(Fail::Bad);
                    }
                    choose(&v)
                } else {
                    if t.len() != 3 {
                        return Err(Fail::Bad);
                    }
                    num(&t[2]).ok_or(Fail::Bad)? as usize
                };
                let f = formats();
                let Some(p) = pf.checked_sub(1).and_then(|i| f.get(i)) else {
                    // (MEASURED, golden opengl-32 edge-pf-range: ERROR_INVALID_PARAMETER, no context)
                    hit(clause::WGL_SETPIXELFORMAT_RANGE);
                    out.push("glctx set failed 87".into());
                    return Ok(true);
                };
                if p.flags & PFD_DRAW_TO_BITMAP == 0 {
                    hit(clause::WGL_SETPIXELFORMAT_BITMAP);
                    out.push("glctx set failed 87".into());
                } else if p.color != bpp {
                    hit(clause::WGL_SETPIXELFORMAT_DEPTH);
                    out.push("glctx set failed 1".into());
                } else {
                    hit(clause::WGL_CONTEXT);
                    if p.ty != 0 {
                        hit(clause::GL_COLORINDEX);
                    }
                    self.ctx = Some(Ctx::new(p.clone(), bpp));
                    out.push(format!("glctx ok {pf}"));
                }
                return Ok(true);
            }
            "glcur" => {
                hit(clause::WGL_CURRENT);
                let c = self.ctx.is_some() as u32;
                out.push(format!("glcur {c} {c}"));
                return Ok(true);
            }
            "gl" | "glmat" | "glfogv" | "glerr" | "glget" | "glisen" | "glstr" | "glread" | "gldraw" | "gldib" | "glrelease" => {}
            _ => return Ok(false),
        }
        let Some(ctx) = self.ctx.as_mut() else {
            out.push("noctx".into());
            return Ok(true);
        };
        match name {
            "gl" => {
                let Some(call) = t.get(1).and_then(|n| CALLS.iter().find(|c| c.0 == n)) else { return Err(Fail::Bad) };
                if t.len() != 2 + call.1.len() {
                    return Err(Fail::Bad);
                }
                let mut a = Vec::new();
                for (k, ty) in call.1.chars().enumerate() {
                    let tok = &t[2 + k];
                    // (a GLfloat argument reaches the call as an f32, a GLdouble one as it is)
                    a.push(match ty {
                        'i' => A::I(num(tok).ok_or(Fail::Bad)?),
                        'f' => A::F(f64::from(parse_float(tok)? as f32)),
                        'd' => A::F(parse_float(tok)?),
                        _ => return Err(Fail::Bad),
                    });
                }
                ctx.call(call.0, &a)?;
            }
            "glmat" => {
                if t.len() != 18 || !(t[1] == "load" || t[1] == "mult") {
                    return Err(Fail::Bad);
                }
                let mut m = [0f32; 16];
                for k in 0..16 {
                    m[k] = parse_float(&t[2 + k])? as f32;
                }
                if ctx.in_begin.is_some() {
                    ctx.error(INVALID_OPERATION);
                } else {
                    hit(clause::GL_MATRIX_LOAD);
                    let i = ctx.mi();
                    let top = ctx.stacks[i].last_mut().unwrap();
                    *top = if t[1] == "load" { m } else { mat_mul(top, &m) };
                }
            }
            "glfogv" => {
                if t.len() < 3 || t.len() > 6 {
                    return Err(Fail::Bad);
                }
                let p = num(&t[1]).ok_or(Fail::Bad)? as u32;
                let mut v = [0f32; 4];
                for k in 2..t.len() {
                    v[k - 2] = parse_float(&t[k])? as f32;
                }
                if ctx.in_begin.is_some() {
                    ctx.error(INVALID_OPERATION);
                } else {
                    ctx.fog_param(p, &v, true);
                }
            }
            "glerr" => {
                // (MEASURED, golden opengl-32 errors-begin: inside glBegin / glEnd glGetError is
                // itself INVALID_OPERATION and returns the error without clearing it, so the
                // executor's loop of eight reads it eight times)
                if ctx.in_begin.is_some() {
                    hit(clause::GL_BEGIN_GETERROR);
                    ctx.error(INVALID_OPERATION);
                    out.push(format!("glerr{}", format!(" {:x}", ctx.err).repeat(8)));
                    return Ok(true);
                }
                hit(clause::GL_ERROR);
                let e = std::mem::replace(&mut ctx.err, NO_ERROR);
                out.push(if e == NO_ERROR { "glerr 0".into() } else { format!("glerr {e:x}") });
            }
            "glget" => {
                let (Some(p), Some(n)) = (t.get(2).and_then(|v| num(v)), t.get(3).and_then(|v| num(v))) else { return Err(Fail::Bad) };
                let kind = t[1].as_str();
                if !(1..=16).contains(&n) || !["i", "f", "b", "d"].contains(&kind) || t.len() != 4 {
                    return Err(Fail::Bad);
                }
                if ctx.in_begin.is_some() {
                    // (MEASURED, golden opengl-32 errors-begin: nothing written, INVALID_OPERATION)
                    hit(clause::GL_BEGIN_GET);
                    ctx.error(INVALID_OPERATION);
                    out.push(format!("get {:x} ={}", p as u32, " -".repeat(n as usize)));
                    return Ok(true);
                }
                let vals = match ctx.get(p as u32)? {
                    Some(v) => v,
                    None => {
                        ctx.error(INVALID_ENUM);
                        vec![]
                    }
                };
                let mut s = format!("get {:x} =", p as u32);
                for k in 0..n as usize {
                    s.push(' ');
                    match vals.get(k) {
                        Some(v) => s.push_str(&print_value(kind, *v)),
                        None => s.push('-'),
                    }
                }
                out.push(s);
            }
            "glisen" => {
                let Some(c) = t.get(1).and_then(|v| num(v)) else { return Err(Fail::Bad) };
                let c = c as u32;
                if ctx.in_begin.is_some() {
                    // (MEASURED, golden opengl-32 edge-begin-query: 0, INVALID_OPERATION)
                    hit(clause::GL_BEGIN_QUERY);
                    ctx.error(INVALID_OPERATION);
                    out.push(format!("isen {c:x} = 0"));
                    return Ok(true);
                }
                let v = match ctx.caps.get(&c) {
                    Some(v) => {
                        hit(clause::GL_ISENABLED);
                        *v as u32
                    }
                    None => {
                        hit(clause::GL_ISENABLED_ENUM);
                        ctx.error(INVALID_ENUM);
                        0
                    }
                };
                out.push(format!("isen {c:x} = {v}"));
            }
            "glstr" => {
                let Some(n) = t.get(1).and_then(|v| num(v)) else { return Err(Fail::Bad) };
                if ctx.in_begin.is_some() {
                    // (MEASURED, golden opengl-32 begin-string: answered as outside, no error)
                    hit(clause::GL_BEGIN_STRING);
                }
                // (MEASURED, golden opengl-32/64 ctx-*: the same four strings at every depth)
                let s = match n {
                    0x1f00 => "Microsoft Corporation",
                    0x1f01 => "GDI Generic",
                    0x1f02 => "1.1.0",
                    0x1f03 => "GL_WIN_swap_hint GL_EXT_bgra GL_EXT_paletted_texture",
                    _ => {
                        hit(clause::GL_STRINGS_ENUM);
                        ctx.error(INVALID_ENUM);
                        "(null)"
                    }
                };
                hit(clause::GL_STRINGS);
                out.push(format!("str {:x} = {s}", n as u32));
            }
            "glrelease" => {
                hit(clause::WGL_RELEASE);
                self.ctx = None;
                out.push("glrelease 1 1".into());
            }
            "glread" => {
                let v: Option<Vec<i64>> = t[1..].iter().map(|x| num(x)).collect();
                let v = v.ok_or(Fail::Bad)?;
                if v.len() != 7 || !(1..=65536).contains(&v[6]) {
                    return Err(Fail::Bad);
                }
                let mut buf = vec![0xccu8; v[6] as usize + 64];
                let mut undefined = vec![false; buf.len()];
                ctx.read_pixels(v[0], v[1], v[2], v[3], v[4] as u32, v[5] as u32, &mut buf, &mut undefined)?;
                let n = v[6] as usize;
                // (an undefined byte as "??": any byte matches it, crate::compare)
                let text: String = buf[..n].iter().zip(&undefined).map(|(b, u)| if *u { "??".to_string() } else { format!("{b:02x}") }).collect();
                out.push(format!("read {text}"));
                if buf[n..].iter().any(|b| *b != 0xcc) {
                    out.push("over".into());
                }
            }
            "gldraw" => {
                let v: Option<Vec<i64>> = t[1..].iter().map(|x| num(x)).collect();
                let v = v.ok_or(Fail::Bad)?;
                if v.len() != 6 || !(1..=65536).contains(&v[5]) {
                    return Err(Fail::Bad);
                }
                let mut r = crate::rng::XorShift32::new(v[4] as u32);
                let data: Vec<u8> = (0..v[5]).map(|_| r.next() as u8).collect();
                ctx.draw_pixels(v[0], v[1], v[2] as u32, v[3] as u32, &data)?;
            }
            "gldib" => {
                let rows = ctx.dib()?;
                out.push(format!("px gl 16 16 {}", ctx.bpp));
                out.extend(rows);
            }
            _ => unreachable!(),
        }
        Ok(true)
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn is_func(f: u32) -> bool {
    (NEVER..=ALWAYS).contains(&f)
}

/// how a fragment's color components become channel values
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToFixed {
    /// a flat, unfogged polygon or point: rounded on 8-bit channels (GL.COLOR.TOFIXED)
    Nearest,
    /// from glDrawPixels, and fogged points: truncated to 8-bit channels (GL.DRAW.TOFIXED, GL.FOG.TOFIXED)
    Trunc,
    /// a fogged one: one 32nd added and truncated, as on channels under 8 bits (GL.FOG.TOFIXED)
    Span,
}

/// a depth already in a depth buffer's fixed point, for a buffer of `bits` bits only
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedZ {
    pub bits: u32,
    pub value: u64,
}

/// a fragment: window position, color, depth in 0..1
#[derive(Clone, Copy, Debug)]
pub struct Frag {
    pub x: i64,
    pub y: i64,
    pub c: [f32; 4],
    pub z: f64,
    /// how the color becomes the channels' values
    pub fixed: ToFixed,
    /// a depth already in the 32-bit buffer's fixed point (a depth image's element)
    pub zfixed: Option<FixedZ>,
}

impl Ctx {
    fn mi(&self) -> usize {
        (self.mode - MODELVIEW) as usize
    }

    /// what a call does; errors as each function's page lists them
    fn call(&mut self, name: &str, a: &[A]) -> Res<()> {
        if self.in_begin.is_some() && !BEGIN_OK.contains(&name) {
            hit(clause::GL_BEGIN_ONLY);
            self.error(INVALID_OPERATION);
            return Ok(());
        }
        match name {
            "glFlush" | "glFinish" => hit(clause::GL_FLUSH),
            "glEnable" | "glDisable" => {
                let c = u(a, 0);
                if self.caps.contains_key(&c) {
                    hit(clause::GL_ENABLE);
                    self.caps.insert(c, name == "glEnable");
                } else {
                    hit(clause::GL_ENABLE_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glMatrixMode" => {
                let m = u(a, 0);
                if (MODELVIEW..=TEXTURE).contains(&m) {
                    hit(clause::GL_MATRIX_MODE);
                    self.mode = m;
                } else {
                    hit(clause::GL_MATRIX_MODE_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glLoadIdentity" => {
                hit(clause::GL_MATRIX_LOAD);
                let i = self.mi();
                *self.stacks[i].last_mut().unwrap() = IDENTITY;
            }
            "glPushMatrix" => {
                let i = self.mi();
                if self.stacks[i].len() == STACK_DEPTH[i] {
                    hit(clause::GL_MATRIX_OVERFLOW);
                    self.error(STACK_OVERFLOW);
                } else {
                    hit(clause::GL_MATRIX_PUSH);
                    let top = *self.top(i);
                    self.stacks[i].push(top);
                }
            }
            "glPopMatrix" => {
                let i = self.mi();
                if self.stacks[i].len() == 1 {
                    hit(clause::GL_MATRIX_UNDERFLOW);
                    self.error(STACK_UNDERFLOW);
                } else {
                    hit(clause::GL_MATRIX_POP);
                    self.stacks[i].pop();
                }
            }
            "glTranslatef" | "glScalef" | "glRotatef" | "glOrtho" | "glFrustum" => {
                let m = match name {
                    "glTranslatef" => {
                        hit(clause::GL_MATRIX_TRANSLATE);
                        let mut m = IDENTITY;
                        m[12..15].copy_from_slice(&[fl(a, 0) as f32, fl(a, 1) as f32, fl(a, 2) as f32]);
                        m
                    }
                    "glScalef" => {
                        // (MEASURED, golden opengl-32 matrix-*: the first three columns multiplied
                        // in place, so a negative factor gives their zeros a sign)
                        hit(clause::GL_MATRIX_SCALE);
                        let i = self.mi();
                        let top = self.stacks[i].last_mut().unwrap();
                        for c in 0..3 {
                            for r in 0..4 {
                                top[c * 4 + r] *= fl(a, c) as f32;
                            }
                        }
                        return Ok(());
                    }
                    "glRotatef" => {
                        hit(clause::GL_MATRIX_ROTATE);
                        let (ang, x, y, z) = (fl(a, 0) as f32, fl(a, 1) as f32, fl(a, 2) as f32, fl(a, 3) as f32);
                        let len = (x * x + y * y + z * z).sqrt();
                        let mut m = IDENTITY;
                        if len > 0.0 {
                            let (x, y, z) = (x / len, y / len, z / len);
                            let (s, c) = (ang * (std::f32::consts::PI / 180.0)).sin_cos();
                            let k = 1.0 - c;
                            // (MEASURED, golden opengl-32 matrix-*: in floats, the diagonal as xx + (1 - xx) c)
                            let (xx, yy, zz, xy, yz, zx) = (x * x, y * y, z * z, x * y, y * z, z * x);
                            let v = [xx + (1.0 - xx) * c, xy * k + z * s, zx * k - y * s, 0.0, xy * k - z * s, yy + (1.0 - yy) * c, yz * k + x * s, 0.0, zx * k + y * s, yz * k - x * s, zz + (1.0 - zz) * c, 0.0];
                            m[..12].copy_from_slice(&v);
                        }
                        m
                    }
                    _ => {
                        let (l, r, b, t, n, f) = (fl(a, 0), fl(a, 1), fl(a, 2), fl(a, 3), fl(a, 4), fl(a, 5));
                        if name == "glOrtho" {
                            hit(clause::GL_MATRIX_ORTHO);
                            if l == r || b == t || n == f {
                                self.error(INVALID_VALUE);
                                return Ok(());
                            }
                            let mut m = IDENTITY;
                            m[0] = (2.0 / (r - l)) as f32;
                            m[5] = (2.0 / (t - b)) as f32;
                            m[10] = (-2.0 / (f - n)) as f32;
                            m[12] = (-(r + l) / (r - l)) as f32;
                            m[13] = (-(t + b) / (t - b)) as f32;
                            m[14] = (-(f + n) / (f - n)) as f32;
                            m
                        } else {
                            hit(clause::GL_MATRIX_FRUSTUM);
                            if n <= 0.0 || f <= 0.0 || l == r || b == t || n == f {
                                self.error(INVALID_VALUE);
                                return Ok(());
                            }
                            let mut m = [0f32; 16];
                            m[0] = (2.0 * n / (r - l)) as f32;
                            m[5] = (2.0 * n / (t - b)) as f32;
                            m[8] = ((r + l) / (r - l)) as f32;
                            m[9] = ((t + b) / (t - b)) as f32;
                            m[10] = (-(f + n) / (f - n)) as f32;
                            m[11] = -1.0;
                            m[14] = (-2.0 * f * n / (f - n)) as f32;
                            m
                        }
                    }
                };
                let i = self.mi();
                let top = self.stacks[i].last_mut().unwrap();
                *top = mat_mul(top, &m);
            }
            "glClearAccum" => {
                // (doc: clamped to -1..1)
                hit(clause::GL_CLEARACCUM);
                self.clear_accum = [0, 1, 2, 3].map(|k| (fl(a, k) as f32).clamp(-1.0, 1.0));
            }
            "glClearColor" => {
                hit(clause::GL_CLEARCOLOR);
                self.clear_color = [clamp01(fl(a, 0)), clamp01(fl(a, 1)), clamp01(fl(a, 2)), clamp01(fl(a, 3))];
            }
            "glClearDepth" => {
                hit(clause::GL_CLEARDEPTH);
                self.clear_depth = fl(a, 0).clamp(0.0, 1.0);
            }
            "glDepthRange" => {
                hit(clause::GL_DEPTHRANGE);
                self.depth_range = [clamp01(fl(a, 0)), clamp01(fl(a, 1))];
            }
            "glClearStencil" => {
                hit(clause::GL_CLEARSTENCIL);
                self.clear_stencil = int(a, 0) & ((1i64 << self.pf.stencil) - 1);
            }
            "glColor4ub" | "glColor3ub" => {
                hit(clause::GL_COLOR_UB);
                // (MEASURED: between glBegin and glEnd times the float reciprocal of 255, golden
                // opengl-32 errors-begin; outside divided by 255, attrib-*: 3 reads 0x3c40c0c1)
                let inside = self.in_begin.is_some();
                let c = |i: usize| {
                    let b = (u(a, i) & 255) as f32;
                    if inside { b * (1.0 / 255.0) } else { b / 255.0 }
                };
                self.color = [c(0), c(1), c(2), if a.len() == 4 { c(3) } else { 1.0 }];
            }
            "glIndexi" => {
                hit(clause::GL_COLORINDEX);
                self.index = int(a, 0) as f32;
            }
            "glClearIndex" => {
                hit(clause::GL_COLORINDEX);
                self.clear_index = fl(a, 0) as f32;
            }
            "glIndexMask" => {
                hit(clause::GL_COLORINDEX);
                self.index_mask = u(a, 0);
            }
            "glColor4f" | "glColor3f" => {
                hit(clause::GL_COLOR_F);
                self.color = [fl(a, 0) as f32, fl(a, 1) as f32, fl(a, 2) as f32, if a.len() == 4 { fl(a, 3) as f32 } else { 1.0 }];
            }
            "glBlendFunc" => {
                let (s, d) = (u(a, 0), u(a, 1));
                let ok_s = [ZERO, ONE, DST_COLOR, ONE_MINUS_DST_COLOR, SRC_ALPHA, ONE_MINUS_SRC_ALPHA, DST_ALPHA, ONE_MINUS_DST_ALPHA, SRC_ALPHA_SATURATE].contains(&s);
                let ok_d = [ZERO, ONE, SRC_COLOR, ONE_MINUS_SRC_COLOR, SRC_ALPHA, ONE_MINUS_SRC_ALPHA, DST_ALPHA, ONE_MINUS_DST_ALPHA].contains(&d);
                hit(clause::GL_BLENDFUNC);
                if ok_s && ok_d {
                    self.blend = (s, d);
                } else {
                    hit(clause::GL_BLENDFUNC_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glLogicOp" => {
                let o = u(a, 0);
                if (CLEAR..=SET).contains(&o) {
                    hit(clause::GL_LOGICOP);
                    self.logic_op = o;
                } else {
                    hit(clause::GL_LOGICOP_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glDepthFunc" | "glAlphaFunc" | "glStencilFunc" => {
                let f = u(a, 0);
                if !is_func(f) {
                    hit(clause::GL_FUNC_ENUM);
                    self.error(INVALID_ENUM);
                    return Ok(());
                }
                hit(clause::GL_FUNC);
                match name {
                    "glDepthFunc" => self.depth_func = f,
                    "glAlphaFunc" => self.alpha = (f, clamp01(fl(a, 1))),
                    _ => {
                        hit(clause::GL_STENCIL_STORE);
                        let smax = (1i64 << self.pf.stencil) - 1;
                        self.stencil_func = (f, int(a, 1).clamp(0, smax), u(a, 2) & smax as u32)
                    }
                }
            }
            "glStencilOp" => {
                let o = [u(a, 0), u(a, 1), u(a, 2)];
                if o.iter().all(|x| [KEEP, ZERO, REPLACE, INCR, DECR, INVERT].contains(x)) {
                    hit(clause::GL_STENCILOP);
                    self.stencil_op = o;
                } else {
                    hit(clause::GL_STENCILOP_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glStencilMask" => {
                hit(clause::GL_STENCILMASK);
                self.stencil_mask = u(a, 0) & (low_bits(self.pf.stencil));
            }
            "glDepthMask" => {
                hit(clause::GL_DEPTHMASK);
                self.depth_mask = u(a, 0) & 255 != 0;
            }
            "glColorMask" => {
                hit(clause::GL_COLORMASK);
                self.color_mask_raw = [u(a, 0) & 255, u(a, 1) & 255, u(a, 2) & 255, u(a, 3) & 255];
                self.color_mask = self.color_mask_raw.map(|v| v != 0);
            }
            "glShadeModel" => {
                let m = u(a, 0);
                if m == FLAT || m == SMOOTH {
                    hit(clause::GL_SHADEMODEL);
                    self.shade = m;
                } else {
                    hit(clause::GL_SHADEMODEL_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glCullFace" => {
                let m = u(a, 0);
                if [FRONT, BACK, FRONT_AND_BACK].contains(&m) {
                    self.cull_mode = m;
                } else {
                    hit(clause::GL_FACE_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glFrontFace" => {
                let m = u(a, 0);
                if m == CW || m == CCW {
                    self.front_face = m;
                } else {
                    hit(clause::GL_FACE_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glHint" => {
                let (t, m) = (u(a, 0), u(a, 1));
                if (0xc50..=0xc54).contains(&t) && (DONT_CARE..=NICEST).contains(&m) {
                    hit(clause::GL_HINT);
                    self.hints[(t - 0xc50) as usize] = m;
                } else {
                    hit(clause::GL_HINT_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glPolygonMode" => {
                let (f, m) = (u(a, 0), u(a, 1));
                if [FRONT, BACK, FRONT_AND_BACK].contains(&f) && (0x1b00..=0x1b02).contains(&m) {
                    if f != BACK {
                        self.poly_mode[0] = m;
                    }
                    if f != FRONT {
                        self.poly_mode[1] = m;
                    }
                } else {
                    hit(clause::GL_FACE_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glPixelStorei" | "glPixelStoref" => {
                let p = u(a, 0);
                // (MEASURED, golden opengl-32 other-calls-3: glPixelStoref's value rounded, halves
                // up, a boolean then false for 0: 2.5 gives 3, -0.5 and 0.25 false:
                // GL.PIXELSTORE.FLOAT)
                let v = if name == "glPixelStoref" {
                    hit(clause::GL_PIXELSTORE_FLOAT);
                    (fl(a, 1) + 0.5).floor() as i64
                } else {
                    int(a, 1)
                };
                let (arr, k) = match p {
                    0xd00..=0xd05 => (&mut self.pack, (p - 0xd00) as usize),
                    0xcf0..=0xcf5 => (&mut self.unpack, (p - 0xcf0) as usize),
                    _ => {
                        hit(clause::GL_PIXELSTORE_ENUM);
                        self.error(INVALID_ENUM);
                        return Ok(());
                    }
                };
                if (k == 5 && ![1, 2, 4, 8].contains(&v)) || (k >= 2 && k <= 4 && v < 0) {
                    hit(clause::GL_PIXELSTORE_VALUE);
                    self.error(INVALID_VALUE);
                    return Ok(());
                }
                hit(clause::GL_PIXELSTORE);
                arr[k] = if k < 2 { (v != 0) as i64 } else { v };
            }
            "glPixelTransferf" | "glPixelTransferi" => {
                let p = u(a, 0);
                let k = [RED_SCALE, GREEN_SCALE, BLUE_SCALE, ALPHA_SCALE].iter().position(|x| *x == p & !1);
                match k {
                    Some(k) if p != 0xd17 && p != 0xd16 => {
                        hit(clause::GL_PIXELTRANSFER);
                        if p & 1 == 0 {
                            self.transfer[k].0 = fl(a, 1) as f32;
                        } else {
                            self.transfer[k].1 = fl(a, 1) as f32;
                        }
                    }
                    // (golden opengl-32 edge-transfer-map: the maps turned on with no error; images
                    // drawn or read through them: GL.PIXELTRANSFER.MAP, GL.PIXELTRANSFER.MAPINDEX)
                    _ if p == MAP_COLOR || p == MAP_STENCIL => {
                        hit(clause::GL_PIXELTRANSFER);
                        self.map[usize::from(p == MAP_STENCIL)] = fl(a, 1) != 0.0;
                    }
                    // (MEASURED, golden opengl-32 transfer-other, transfer-other-ci: GL.PIXELTRANSFER.SHIFT)
                    _ if p == INDEX_SHIFT || p == INDEX_OFFSET => {
                        hit(clause::GL_PIXELTRANSFER_SHIFT);
                        if p == INDEX_SHIFT { self.index_shift.0 = int(a, 1) } else { self.index_shift.1 = int(a, 1) }
                    }
                    _ if p == DEPTH_SCALE || p == DEPTH_BIAS => {
                        hit(clause::GL_PIXELTRANSFER_SHIFT);
                        if p == DEPTH_SCALE { self.depth_transfer.0 = fl(a, 1) as f32 } else { self.depth_transfer.1 = fl(a, 1) as f32 }
                    }
                    // (MEASURED, golden opengl-32 transfer-maps-stencil: GL_ZOOM_X and
                    // GL_PACK_SWAP_BYTES INVALID_ENUM: GL.PIXELTRANSFER.ENUM)
                    _ => {
                        hit(clause::GL_PIXELTRANSFER_ENUM);
                        self.error(INVALID_ENUM);
                    }
                }
            }
            "glRasterPos2i" | "glRasterPos2f" => {
                // (MEASURED, golden opengl-32 other-calls-0: the position through the matrices
                // and the viewport; one outside the view volume is invalid and images at it draw
                // nothing: GL.RASTER.POS)
                hit(clause::GL_RASTER_POS);
                let v = [fl(a, 0), fl(a, 1), 0.0];
                let e = xform(self.top(0), [v[0], v[1], v[2], 1.0]);
                let c = xform(self.top(1), e);
                if c[3] != 1.0 {
                    return Err(Fail::Unmodeled(open::GL_RASTER_PERSPECTIVE));
                }
                let inside = c[..3].iter().all(|v| (-1.0..=1.0).contains(v));
                let [vx, vy, vw, vh] = self.viewport.map(|v| v as f64);
                let (n, f) = (self.depth_range[0] as f64, self.depth_range[1] as f64);
                self.raster = inside.then(|| [(c[0] + 1.0) * vw / 2.0 + vx, (c[1] + 1.0) * vh / 2.0 + vy, ((f - n) * c[2] + n + f) / 2.0]);
                self.raster_color = self.current();
                self.raster_eye = e[2];
            }
            "glFogf" => self.fog_param(u(a, 0), &[fl(a, 1) as f32], false),
            "glPolygonOffset" => {
                hit(clause::GL_POLYGON_OFFSET_STATE);
                self.offset = (fl(a, 0) as f32, fl(a, 1) as f32);
            }
            "glPixelZoom" => {
                // (golden opengl-32 edge-other-call: no error; images drawn zoomed: GL.DRAW.ZOOM)
                hit(clause::GL_PIXELTRANSFER);
                self.zoom = (fl(a, 0) as f32, fl(a, 1) as f32);
            }
            "glViewport" | "glScissor" => {
                let v = [int(a, 0), int(a, 1), int(a, 2), int(a, 3)];
                if v[2] < 0 || v[3] < 0 {
                    hit(clause::GL_VIEWPORT_VALUE);
                    self.error(INVALID_VALUE);
                } else if name == "glScissor" {
                    hit(clause::GL_SCISSOR);
                    self.scissor = v;
                } else {
                    hit(clause::GL_VIEWPORT);
                    self.viewport = [v[0], v[1], v[2].min(16384), v[3].min(16384)];
                }
            }
            "glLineWidth" | "glPointSize" => {
                let v = fl(a, 0) as f32;
                if v <= 0.0 {
                    hit(clause::GL_WIDTH_VALUE);
                    self.error(INVALID_VALUE);
                } else if name == "glLineWidth" {
                    hit(clause::GL_WIDTH);
                    self.line_width = v;
                } else {
                    hit(clause::GL_WIDTH);
                    self.point_size = v;
                }
            }
            "glBegin" => {
                let m = u(a, 0);
                if m > 9 {
                    hit(clause::GL_BEGIN_ENUM);
                    self.error(INVALID_ENUM);
                } else {
                    hit(clause::GL_BEGIN);
                    self.in_begin = Some(m);
                    self.verts.clear();
                }
            }
            "glEnd" => match self.in_begin.take() {
                None => {
                    hit(clause::GL_END_OUTSIDE);
                    self.error(INVALID_OPERATION);
                }
                Some(m) => self.end(m)?,
            },
            "glNormal3f" => {
                hit(clause::GL_NORMAL);
                self.normal = [fl(a, 0) as f32, fl(a, 1) as f32, fl(a, 2) as f32];
            }
            "glVertex4f" if fl(a, 3) != 1.0 => return Err(Fail::Unmodeled(open::GL_RASTER_PERSPECTIVE)),
            "glVertex2f" | "glVertex3f" | "glVertex4f" | "glVertex2i" | "glVertex3i" => {
                let v = [fl(a, 0), fl(a, 1), if a.len() > 2 { fl(a, 2) } else { 0.0 }];
                if self.in_begin.is_some() {
                    self.verts.push((v, self.lit(self.current())));
                } else {
                    // (MEASURED, golden opengl-32 edge-vertex-outside: ignored, no error)
                    hit(clause::GL_VERTEX_OUTSIDE);
                }
            }
            "glClear" => {
                let m = u(a, 0);
                if m & !(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT | STENCIL_BUFFER_BIT | ACCUM_BUFFER_BIT) != 0 {
                    hit(clause::GL_CLEAR_VALUE);
                    self.error(INVALID_VALUE);
                } else {
                    self.clear(m)?;
                }
            }
            "glRectf" | "glRecti" => {
                hit(clause::GL_RECT);
                let (x0, y0, x1, y1) = (fl(a, 0), fl(a, 1), fl(a, 2), fl(a, 3));
                self.verts.clear();
                for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
                    self.verts.push(([x, y, 0.0], self.lit(self.current())));
                }
                self.rect_call = true;
                let r = self.end(QUADS);
                self.rect_call = false;
                r?;
            }
            "glDrawBuffer" | "glReadBuffer" => {
                // a single-buffered DIB has the front left buffer only (MEASURED, golden opengl-32
                // errors: right, back and auxiliary buffers INVALID_OPERATION, other names INVALID_ENUM)
                let b = u(a, 0);
                let draw = name == "glDrawBuffer";
                if [FRONT, FRONT_LEFT, LEFT].contains(&b) || (draw && (b == FRONT_AND_BACK || b == 0)) {
                    hit(clause::GL_BUFFER);
                    self.buffers[!draw as usize] = b;
                } else if (0x400..=0x40c).contains(&b) && (draw || b != FRONT_AND_BACK) {
                    hit(clause::GL_BUFFER_MISSING);
                    self.error(INVALID_OPERATION);
                } else {
                    hit(clause::GL_BUFFER_ENUM);
                    self.error(INVALID_ENUM);
                }
            }
            "glPushAttrib" => {
                if self.attrib.len() == 16 {
                    hit(clause::GL_ATTRIB_OVERFLOW);
                    self.error(STACK_OVERFLOW);
                } else {
                    hit(clause::GL_ATTRIB_PUSH);
                    let mut saved = self.clone();
                    saved.attrib = Vec::new();
                    saved.fb = Vec::new();
                    saved.depth = Vec::new();
                    saved.stencil = Vec::new();
                    saved.acc = Vec::new();
                    saved.fb_undefined = Vec::new();
                    self.attrib.push((u(a, 0), saved));
                }
            }
            "glPopAttrib" => match self.attrib.pop() {
                None => {
                    hit(clause::GL_ATTRIB_UNDERFLOW);
                    self.error(STACK_UNDERFLOW);
                }
                Some((mask, saved)) => self.pop_attrib(mask, saved),
            },
            "glAccum" => {
                match AccumOp::try_from(u(a, 0)) {
                    Err(()) => {
                        hit(clause::GL_ACCUM_ENUM);
                        self.error(INVALID_ENUM);
                    }
                    // (only the color index formats have none; GL 1.1 4.2.4: INVALID_OPERATION in
                    // color index mode; case accum-none)
                    Ok(_) if self.pf.accum == 0 => {
                        hit(clause::GL_COLORINDEX);
                        self.error(INVALID_OPERATION);
                    }
                    Ok(op) => self.accum(op, fl(a, 1) as f32)?,
                }
            }
            // (every call of the executor's table is followed above: a name here is the model's bug)
            _ => return Err(Fail::Bad),
        }
        Ok(())
    }
}

/// the ordered dither matrix (MEASURED, golden opengl-32 dither-fill-0..3: every position of a
/// 16-bit DIB, 16 colors, fits floor(c * 31 + (m + 1/2) / 16)), indexed by window y then x
const DITHER_MATRIX: [[u8; 4]; 4] = [[15, 7, 13, 5], [3, 11, 1, 9], [12, 4, 14, 6], [0, 8, 2, 10]];

/// the dither threshold at a window pixel
fn dither_threshold(x: i64, y: i64) -> f32 {
    let m = DITHER_MATRIX[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize];
    (f32::from(m) + 0.5) / 16.0
}

/// glPushAttrib's groups
pub mod attrib {
    pub const CURRENT: u32 = 0x1;
    pub const POINT: u32 = 0x2;
    pub const LINE: u32 = 0x4;
    pub const POLYGON: u32 = 0x8;
    pub const PIXEL_MODE: u32 = 0x20;
    pub const LIGHTING: u32 = 0x40;
    pub const FOG: u32 = 0x80;
    pub const DEPTH: u32 = 0x100;
    pub const ACCUM: u32 = 0x200;
    pub const STENCIL: u32 = 0x400;
    pub const VIEWPORT: u32 = 0x800;
    pub const TRANSFORM: u32 = 0x1000;
    pub const ENABLE: u32 = 0x2000;
    pub const COLOR: u32 = 0x4000;
    pub const HINT: u32 = 0x8000;
    pub const EVAL: u32 = 0x10000;
    pub const TEXTURE: u32 = 0x40000;
    pub const SCISSOR: u32 = 0x80000;
}

/// glAccum's operations (GL_ACCUM 0x100 .. GL_ADD 0x104)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccumOp {
    Accum,
    Load,
    Return,
    Mult,
    Add,
}

impl TryFrom<u32> for AccumOp {
    type Error = ();
    fn try_from(op: u32) -> Result<AccumOp, ()> {
        match op {
            0x100 => Ok(AccumOp::Accum),
            0x101 => Ok(AccumOp::Load),
            0x102 => Ok(AccumOp::Return),
            0x103 => Ok(AccumOp::Mult),
            0x104 => Ok(AccumOp::Add),
            _ => Err(()),
        }
    }
}

/// glFog's modes (GL_EXP 0x800, GL_EXP2 0x801, GL_LINEAR 0x2601)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FogMode {
    Exp,
    Exp2,
    Linear,
}

impl TryFrom<u32> for FogMode {
    type Error = ();
    fn try_from(m: u32) -> Result<FogMode, ()> {
        match m {
            0x800 => Ok(FogMode::Exp),
            0x801 => Ok(FogMode::Exp2),
            0x2601 => Ok(FogMode::Linear),
            _ => Err(()),
        }
    }
}

impl FogMode {
    pub fn name(self) -> u32 {
        match self {
            FogMode::Exp => 0x800,
            FogMode::Exp2 => 0x801,
            FogMode::Linear => 0x2601,
        }
    }
}

/// the fog state glFog sets
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub mode: FogMode,
    pub density: f32,
    pub start: f32,
    pub end: f32,
    pub index: f32,
    pub color: [f32; 4],
}

impl Fog {
    /// the fog factor at a vertex `z` from the eye, held to 0..1 (GL 1.1 3.9; MEASURED, golden
    /// opengl-32 other-calls-2: the distance is |z| in eye coordinates, the factor taken at the
    /// vertices and interpolated across the primitive: GL.FOG.APPLY)
    pub fn factor(&self, z: f64) -> f32 {
        let c = z.abs() as f32;
        let f = match self.mode {
            FogMode::Linear => (self.end - c) / (self.end - self.start),
            FogMode::Exp => (-(self.density * c)).exp(),
            FogMode::Exp2 => (-(self.density * c) * (self.density * c)).exp(),
        };
        if f.is_nan() { 0.0 } else { f.clamp(0.0, 1.0) }
    }
}

/// the transform of a vertex to window coordinates (x, y, z in 0..1)
fn xform(m: &[f32; 16], v: [f64; 4]) -> [f64; 4] {
    let mut r = [0f64; 4];
    for row in 0..4 {
        r[row] = (0..4).map(|k| m[k * 4 + row] as f64 * v[k]).sum();
    }
    r
}

impl Ctx {
    /// glFogf / glFogfv: one of the fog values (GL 1.1 3.9: a negative density INVALID_VALUE,
    /// a mode or name not fog's INVALID_ENUM, the color by the vector call only)
    fn fog_param(&mut self, p: u32, v: &[f32], vector: bool) {
        match (p, v) {
            (FOG_MODE, [m, ..]) => match FogMode::try_from(*m as u32) {
                Ok(m) => {
                    hit(clause::GL_FOG_STATE);
                    self.fog.mode = m;
                }
                Err(()) => {
                    hit(clause::GL_FOG_ENUM);
                    self.error(INVALID_ENUM);
                }
            },
            (FOG_DENSITY, [d, ..]) if *d < 0.0 => {
                hit(clause::GL_FOG_ENUM);
                self.error(INVALID_VALUE);
            }
            (FOG_DENSITY, [d, ..]) => {
                hit(clause::GL_FOG_STATE);
                self.fog.density = *d;
            }
            (FOG_START, [d, ..]) => {
                hit(clause::GL_FOG_STATE);
                self.fog.start = *d;
            }
            (FOG_END, [d, ..]) => {
                hit(clause::GL_FOG_STATE);
                self.fog.end = *d;
            }
            (FOG_INDEX, [d, ..]) => {
                hit(clause::GL_FOG_STATE);
                self.fog.index = *d;
            }
            (FOG_COLOR, [r, g, b, al, ..]) if vector => {
                hit(clause::GL_FOG_STATE);
                self.fog.color = [*r, *g, *b, *al].map(|c| c.clamp(0.0, 1.0));
            }
            _ => {
                hit(clause::GL_FOG_ENUM);
                self.error(INVALID_ENUM);
            }
        }
    }

    /// a vertex's color fogged at its eye z (GL.FOG.APPLY): the RGB toward the fog color, alpha
    /// kept; a color index plus the fog index times one less the factor
    fn fogged(&self, c: [f32; 4], v: [f64; 3]) -> [f32; 4] {
        if !self.cap(FOG) {
            return c;
        }
        hit(clause::GL_FOG_APPLY);
        self.fog_at(c, xform(self.top(0), [v[0], v[1], v[2], 1.0])[2])
    }

    /// a color fogged at eye z (GL.FOG.APPLY), whatever GL_FOG
    fn fog_at(&self, c: [f32; 4], z: f64) -> [f32; 4] {
        let f = self.fog.factor(z);
        if self.pf.ty != 0 {
            return [c[0] + (1.0 - f) * self.fog.index, 0.0, 0.0, 0.0];
        }
        let k = self.fog.color;
        [f * c[0] + (1.0 - f) * k[0], f * c[1] + (1.0 - f) * k[1], f * c[2] + (1.0 - f) * k[2], c[3]]
    }

    /// how a primitive's fragments become channel values: fogged ones another way (GL.FOG.TOFIXED)
    fn span(&self) -> ToFixed {
        if self.cap(FOG) || self.cap(LIGHTING) { ToFixed::Span } else { ToFixed::Nearest }
    }

    /// how an antialiased primitive's fragments become channel values (MEASURED, golden
    /// opengl-32 other-calls-7: alpha 255 k / 16 truncated for k of 16 samples covered: 47 for 3)
    fn smooth_fixed(&self) -> ToFixed {
        ToFixed::Trunc
    }

    /// how a point's fragments become channel values (MEASURED, golden opengl-32 fog-points: 192
    /// fogged channels truncated, without the 32nd polygons add: GL.FOG.TOFIXED)
    fn point_fixed(&self) -> ToFixed {
        if self.cap(FOG) || self.cap(LIGHTING) {
            hit(clause::GL_FOG_TOFIXED);
            ToFixed::Trunc
        } else {
            ToFixed::Nearest
        }
    }

    /// a vertex's color under GL_LIGHTING with every light off and the initial material
    /// (MEASURED, golden opengl-32 render-caps-0: the scene's ambient times the material's
    /// ambient 0.2, the material's diffuse alpha 1, whatever the current color; lighting-material:
    /// under GL_COLOR_MATERIAL (ambient and diffuse) the scene's ambient times the current color,
    /// its alpha: GL.LIGHTING)
    fn lit(&self, c: [f32; 4]) -> [f32; 4] {
        if !self.cap(LIGHTING) {
            return c;
        }
        hit(clause::GL_LIGHTING);
        let a = match self.get(0xb53) {
            Ok(Some(v)) => match v[0] {
                V::C(x) | V::F(x) => x,
                _ => 0.2,
            },
            _ => 0.2,
        };
        // n . L for GL_LIGHT0, the only light whose initial diffuse is not 0: its direction
        // (0, 0, 1) in eye coordinates, the normal through the modelview's inverse transpose
        // (MEASURED, golden opengl-32 other-calls-8: 0.04 plus 0.8 for the normal (0, 0, 1))
        let d = if self.cap(0x4000) {
            let n = self.eye_normal();
            f32::max(n[2], 0.0)
        } else {
            0.0
        };
        if self.pf.ty != 0 {
            // color index lighting (GL 1.1 2.13.5) with the initial material indices (0, 1, 1):
            // the ambient index 0 alone with no light on (MEASURED, golden opengl-32
            // other-calls-9); with GL_LIGHT0 the specular index 1 where n . L > 0, the
            // shininess being 0
            return [if d > 0.0 { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0];
        }
        if self.cap(COLOR_MATERIAL) {
            return [a * c[0] + d * c[0], a * c[1] + d * c[1], a * c[2] + d * c[2], c[3]];
        }
        let l = a * 0.2 + d * 0.8;
        [l, l, l, 1.0]
    }

    /// the current normal in eye coordinates: times the inverse of the modelview's upper 3 by 3
    fn eye_normal(&self) -> [f32; 3] {
        let m = self.top(0);
        let a = |r: usize, c: usize| f64::from(m[c * 4 + r]);
        let det = a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0)) + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0));
        let n = self.normal.map(f64::from);
        if det == 0.0 {
            return [0.0; 3];
        }
        // n' = n M^-1 (a row vector): component k is n . column k of the inverse
        let inv = |r: usize, c: usize| {
            let (r1, r2) = ((c + 1) % 3, (c + 2) % 3);
            let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
            (a(r1, c1) * a(r2, c2) - a(r1, c2) * a(r2, c1)) / det
        };
        let mut out = [0f32; 3];
        for k in 0..3 {
            out[k] = (0..3).map(|i| n[i] * inv(i, k)).sum::<f64>() as f32;
        }
        out
    }

    /// whether every vertex is past the same near or far plane, the primitive then not drawn
    /// (MEASURED, golden opengl-32 edge-raster-clip: a quad at z 2 under glOrtho(.., -1, 1)
    /// draws nothing: GL.RASTER.CLIP.Z)
    fn culled_z(&self, vs: &[[f64; 3]]) -> bool {
        let z: Vec<f64> = vs.iter().map(|v| xform(self.top(1), xform(self.top(0), [v[0], v[1], v[2], 1.0]))).filter(|c| c[3] == 1.0).map(|c| c[2]).collect();
        if z.len() != vs.len() || z.is_empty() {
            return false;
        }
        let out = z.iter().all(|z| *z < -1.0) || z.iter().all(|z| *z > 1.0);
        if out {
            hit(clause::GL_RASTER_CLIP_Z);
        }
        out
    }

    fn window(&self, v: [f64; 3]) -> Res<[f64; 3]> {
        let e = xform(self.top(0), [v[0], v[1], v[2], 1.0]);
        let c = xform(self.top(1), e);
        if c[3] != 1.0 {
            return Err(Fail::Unmodeled(open::GL_RASTER_PERSPECTIVE));
        }
        let [vx, vy, vw, vh] = self.viewport.map(|v| v as f64);
        let (n, f) = (self.depth_range[0] as f64, self.depth_range[1] as f64);
        if c[2] < -1.0 || c[2] > 1.0 {
            // (a primitive the near or far plane cuts: depths across the cut part)
            return Err(Fail::Unmodeled(open::GL_RASTER_INTERPOLATE));
        }
        hit(clause::GL_RASTER_WINDOW);
        Ok([(c[0] + 1.0) * vw / 2.0 + vx, (c[1] + 1.0) * vh / 2.0 + vy, ((f - n) * c[2] + n + f) / 2.0])
    }

    fn end(&mut self, mode: u32) -> Res<()> {
        let verts = std::mem::take(&mut self.verts);
        // (antialiased points, antialiased polygons whose coverage would show in alpha, lights,
        // color materials and color index lighting are not measured)
        let alpha_used = self.pf.rgba[3].0 > 0 || self.cap(BLEND) || self.cap(ALPHA_TEST);
        let alpha_used = alpha_used;
        match mode {
            POINTS => {
                hit(clause::GL_POINT);
                for (v, c) in verts {
                    if self.culled_z(&[v]) {
                        continue;
                    }
                    let w = self.window(v)?;
                    // (MEASURED, golden opengl-32 points-*: a coordinate on a pixel boundary belongs
                    // to the pixel right of it in x and below it in y: the rows counted from the top)
                    // (MEASURED, golden opengl-32 point-edge-*: a coordinate on a pixel boundary
                    // goes to the pixel toward the tie pivot, the pivot's own column to the right
                    // and its own row below: GL.POINT.TIE)
                    let (px, py) = self.tie_pivot();
                    let x = if w[0].fract() == 0.0 {
                        hit(clause::GL_POINT_TIE);
                        if w[0] <= px { w[0] } else { w[0] - 1.0 }
                    } else {
                        w[0].floor()
                    };
                    let y = if w[1].fract() == 0.0 {
                        hit(clause::GL_POINT_TIE);
                        if w[1] < py { w[1] } else { w[1] - 1.0 }
                    } else {
                        w[1].floor()
                    };
                    let _ = (x, y);
                    // (MEASURED, golden opengl-32 prim-*: a wide point whose position is outside
                    // the viewport is clipped whole, one on its border kept: GL.RASTER.CLIP.XY)
                    let [vx, vy, vw, vh] = self.viewport.map(|v| v as f64);
                    if w[0] < vx || w[0] > vx + vw || w[1] < vy || w[1] > vy + vh {
                        hit(clause::GL_RASTER_CLIP_XY);
                        continue;
                    }
                    // (MEASURED, golden opengl-32 prim-*: a point of width n, its size rounded and
                    // held to the size range, covers n x n pixels: for odd n centered on the pixel
                    // the position is in, for even n with its corner at the pixel boundary nearest
                    // the position, ties toward the pivot as for 1-pixel points: GL.POINT.WIDE)
                    if self.cap(POINT_SMOOTH) {
                        for (x, y, k) in self.smooth_point(w[0], w[1])? {
                            let mut c = self.fogged(c, v);
                            c[3] *= k as f32 / 16.0;
                            self.fragment(Frag { x, y, c, z: w[2], fixed: self.smooth_fixed(), zfixed: None })?;
                        }
                        continue;
                    }
                    let n = f64::from(self.point_size.clamp(1.0, 10.0)).round();
                    if n > 1.0 {
                        hit(clause::GL_POINT_WIDE);
                    }
                    let half = (n / 2.0).floor();
                    let (x0, y0) = if n % 2.0 == 1.0 {
                        (x - half, y - half)
                    } else {
                        // (the boundary nearest the position; one halfway between two, toward the pivot)
                        let near = |v: f64, p: f64| if (v + 0.5).fract() != 0.0 { (v + 0.5).floor() } else if v < p { v + 0.5 } else { v - 0.5 };
                        (near(w[0], px) - half, near(w[1], py) - half)
                    };
                    for j in 0..n as i64 {
                        for i in 0..n as i64 {
                            let (x, y) = (x0 as i64 + i, y0 as i64 + j);
                            if (0..W as i64).contains(&x) && (0..H as i64).contains(&y) {
                                self.fragment(Frag { x, y, c: self.fogged(c, v), z: w[2], fixed: self.point_fixed(), zfixed: None })?;
                            }
                        }
                    }
                }
            }
            QUADS => {
                for q in verts.chunks(4).filter(|q| q.len() == 4) {
                    if self.culled_z(&q.iter().map(|v| v.0).collect::<Vec<_>>()) {
                        continue;
                    }
                    let w: Vec<[f64; 3]> = q.iter().map(|v| self.window(v.0)).collect::<Res<_>>()?;
                    if q.iter().any(|v| v.1 != q[3].1) || w.iter().any(|p| p[2] != w[0][2]) {
                        return Err(Fail::Unmodeled(open::GL_RASTER_INTERPOLATE));
                    }
                    // (axis-aligned rectangles only: the cases draw no other quads)
                    let xs = [w[0][0], w[1][0], w[2][0], w[3][0]];
                    let ys = [w[0][1], w[1][1], w[2][1], w[3][1]];
                    if !(xs[0] == xs[3] && xs[1] == xs[2] && ys[0] == ys[1] && ys[2] == ys[3]) {
                        return Err(Fail::Unmodeled(open::GL_RASTER_POLYGON));
                    }
                    // clipped to the view volume: the viewport's rectangle (GL.RASTER.CLIP.XY)
                    let [vx, vy, vw, vh] = self.viewport.map(|v| v as f64);
                    let (x0, x1) = (xs[0].min(xs[1]).max(vx), xs[0].max(xs[1]).min(vx + vw));
                    let (y0, y1) = (ys[0].min(ys[2]).max(vy), ys[0].max(ys[2]).min(vy + vh));
                    if x0 != xs[0].min(xs[1]) || x1 != xs[0].max(xs[1]) || y0 != ys[0].min(ys[2]) || y1 != ys[0].max(ys[2]) {
                        hit(clause::GL_RASTER_CLIP_XY);
                    }
                    if x0 >= x1 || y0 >= y1 {
                        continue;
                    }
                    // (MEASURED, golden opengl-32 render-caps-1: under GL_CULL_FACE a rectangle
                    // facing the culled way is not drawn, its facing the sign of its window area
                    // and glFrontFace; GL_FRONT_AND_BACK culls every one: GL.CULL)
                    if self.cap(CULL_FACE) {
                        hit(clause::GL_CULL);
                        let area: f64 = (0..4).map(|k| xs[k] * ys[(k + 1) % 4] - xs[(k + 1) % 4] * ys[k]).sum();
                        let front = (area > 0.0) == (self.front_face == CCW);
                        let culled = match self.cull_mode {
                            FRONT => front,
                            BACK => !front,
                            _ => true,
                        };
                        if culled {
                            continue;
                        }
                    }
                    // (MEASURED, golden opengl-32 render-caps-3: under GL_POLYGON_SMOOTH every
                    // pixel the rectangle covers any of is drawn, with no alpha to show the
                    // coverage in: GL.POLYGON.SMOOTH)
                    let smooth = self.cap(POLYGON_SMOOTH);
                    if smooth {
                        hit(clause::GL_POLYGON_SMOOTH);
                    }
                    // a pixel whose center is on an edge (MEASURED, golden opengl-32 rect-*, tie-x-*,
                    // tie-y-*, tie-vp-*: every vertical and horizontal edge through pixel centers):
                    // as if the vertices were moved a little toward the window point of normalized
                    // device coordinates (0.5, -0.5), the pixel in when that moves the edge past
                    // its center (GL.RASTER.TIE.X, GL.RASTER.TIE.Y)
                    let on = |a: f64| (a - 0.5).fract() == 0.0;
                    if on(x0) || on(x1) {
                        hit(clause::GL_RASTER_TIE_X);
                    }
                    if on(y0) || on(y1) {
                        hit(clause::GL_RASTER_TIE_Y);
                    }
                    let (px, py) = self.tie_pivot();
                    let inside = |c: f64, lo: f64, hi: f64, p: f64| (c > lo || (c == lo && lo > p)) && (c < hi || (c == hi && hi < p));
                    // a corner on a pixel center that one edge's rule takes in and the other's
                    // leaves out: the diagonal the rectangle is split along decides (golden
                    // opengl-32 tie-x-2, rect-16: in, where tie-x-* and tie-y-* have it out)
                    for cx in [x0, x1] {
                        for cy in [y0, y1] {
                            if !smooth && on(cx) && on(cy) && (0.0..W as f64).contains(&cx) && (0.0..H as f64).contains(&cy) {
                                let ix = (cx == x0 && x0 > px) || (cx == x1 && x1 < px);
                                let iy = (cy == y0 && y0 > py) || (cy == y1 && y1 < py);
                                if ix != iy {
                                    return Err(Fail::Unmodeled(open::GL_RASTER_TIE));
                                }
                            }
                        }
                    }
                    hit(clause::GL_RASTER_RECT);
                    let clipped = x0 != xs[0].min(xs[1]) || x1 != xs[0].max(xs[1]) || y0 != ys[0].min(ys[2]) || y1 != ys[0].max(ys[2]);
                    // (the x86-64 executor and quads from glBegin draw scattered pixels under NEVER:
                    // GL.DEPTH.NEVER)
                    self.never = match (self.rect_call && crate::model::arch() == 32, clipped) {
                        (true, true) => NeverPath::Clipped,
                        (true, false) => NeverPath::Unclipped,
                        (false, _) => NeverPath::Other,
                    };
                    // fogged at the vertices: the rectangle's are at one depth in eye coordinates
                    let c = self.fogged(q[3].1, q[3].0);
                    if q.iter().any(|v| self.fogged(q[3].1, v.0) != c) {
                        return Err(Fail::Unmodeled(open::GL_RASTER_INTERPOLATE));
                    }
                    // (MEASURED, golden opengl-32 other-calls-1: a flat polygon's depth moved by
                    // units times 2^-23 under GL_POLYGON_OFFSET_FILL, the factor times its zero
                    // slope adding nothing: GL.POLYGON.OFFSET)
                    let (z, zfixed) = match (self.cap(POLYGON_OFFSET_FILL), self.pf.depth) {
                        (false, _) => (w[0][2], None),
                        (true, 16) => {
                            // (MEASURED, golden opengl-32 poly-offset-1: on 16 bits units of
                            // 1/32767, z times 32767 plus the units truncated, shifted left by one;
                            // past 1.0 the buffer's 0, below 0 its 0)
                            hit(clause::GL_POLYGON_OFFSET);
                            let v = (w[0][2] as f32 * 32767.0 + self.offset.1).floor();
                            let v = if (0.0..32768.0).contains(&v) { (v as u64) << 1 } else { 0 };
                            (w[0][2], Some(FixedZ { bits: 16, value: v }))
                        }
                        (true, _) => {
                            // (in floats: golden poly-offset-0, -0.5 units on 0.8755)
                            hit(clause::GL_POLYGON_OFFSET);
                            (f64::from(w[0][2] as f32 + self.offset.1 * 2f32.powi(-23)), None)
                        }
                    };
                    if smooth && !alpha_used {
                        for y in 0..H as i64 {
                            for x in 0..W as i64 {
                                if (x as f64 + 1.0) > x0 && (x as f64) < x1 && (y as f64 + 1.0) > y0 && (y as f64) < y1 {
                                    self.fragment(Frag { x, y, c, z, fixed: self.span(), zfixed })?;
                                }
                            }
                        }
                        self.never = NeverPath::Other;
                        continue;
                    }
                    if smooth {
                        // (MEASURED, golden opengl-32 other-calls-7: the quad's two triangles
                        // (v0 v1 v2, then v0 v2 v3) each drawn on its own, a pixel with k of 4 by
                        // 4 samples inside one taking alpha times k / 16, the second's
                        // fragments over the first's along the diagonal: GL.POLYGON.SMOOTH)
                        // (a sample on the diagonal counted in neither: golden other-calls-7 the
                        // same either way)
                        for (n, t) in [[0, 1, 2], [0, 2, 3]].into_iter().enumerate() {
                            let p = t.map(|k| (xs[k], ys[k]));
                            let edge = |a: (f64, f64), b: (f64, f64), x: f64, y: f64| (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0);
                            for y in 0..H as i64 {
                                for x in 0..W as i64 {
                                    let mut k = 0u32;
                                    for i in 0..4 {
                                        for j in 0..4 {
                                            let (sx, sy) = (x as f64 + (i as f64 + 0.5) / 4.0, y as f64 + (j as f64 + 0.5) / 4.0);
                                            if sx < x0 || sx > x1 || sy < y0 || sy > y1 {
                                                continue;
                                            }
                                            let e = [edge(p[0], p[1], sx, sy), edge(p[1], p[2], sx, sy), edge(p[2], p[0], sx, sy)];
                                            let outer = if n == 0 { [0, 1] } else { [1, 2] };
                                            if outer.iter().any(|k| e[*k] == 0.0) || sx == x0 || sx == x1 || sy == y0 || sy == y1 {
                                                return Err(Fail::Unmodeled(open::GL_RASTER_TIE));
                                            }
                                            k += u32::from(e.iter().all(|v| *v > 0.0) || e.iter().all(|v| *v < 0.0));
                                        }
                                    }
                                    if k > 0 {
                                        let mut c = c;
                                        c[3] *= k as f32 / 16.0;
                                        self.fragment(Frag { x, y, c, z, fixed: self.smooth_fixed(), zfixed })?;
                                    }
                                }
                            }
                        }
                        self.never = NeverPath::Other;
                        continue;
                    }
                    for y in 0..H as i64 {
                        for x in 0..W as i64 {
                            let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
                            let covered = inside(cx, x0, x1, px) && inside(cy, y0, y1, py);
                            if covered {
                                let r = self.fragment(Frag { x, y, c, z, fixed: self.span(), zfixed });
                                if r.is_err() {
                                    self.never = NeverPath::Other;
                                }
                                r?;
                            }
                        }
                    }
                    self.never = NeverPath::Other;
                }
            }
            LINES | LINE_LOOP | LINE_STRIP => {
                // (wide, stippled and antialiased lines are not measured)
                if self.line_width != 1.0 || self.cap(LINE_STIPPLE) || self.cap(LINE_SMOOTH) {
                    return Err(Fail::Unmodeled(open::GL_RASTER_MODE));
                }
                let n = verts.len();
                let pairs: Vec<(usize, usize)> = match mode {
                    LINES => (0..n / 2).map(|k| (2 * k, 2 * k + 1)).collect(),
                    LINE_STRIP => (1..n).map(|k| (k - 1, k)).collect(),
                    _ if n < 2 => Vec::new(),
                    _ => (0..n).map(|k| (k, (k + 1) % n)).collect(),
                };
                for (i, j) in pairs {
                    let (a, b) = (&verts[i], &verts[j]);
                    if self.culled_z(&[a.0, b.0]) {
                        continue;
                    }
                    let (wa, wb) = (self.window(a.0)?, self.window(b.0)?);
                    if a.1 != b.1 || wa[2] != wb[2] {
                        return Err(Fail::Unmodeled(open::GL_RASTER_INTERPOLATE));
                    }
                    let c = self.fogged(a.1, a.0);
                    if self.fogged(b.1, b.0) != c {
                        return Err(Fail::Unmodeled(open::GL_RASTER_INTERPOLATE));
                    }
                    for (x, y) in self.line_pixels([wa[0], wa[1]], [wb[0], wb[1]])? {
                        self.fragment(Frag { x, y, c, z: wa[2], fixed: self.span(), zfixed: None })?;
                    }
                }
            }
            _ => return Err(Fail::Unmodeled(open::GL_RASTER_MODE)),
        }
        Ok(())
    }

    /// the pixels of an antialiased point at window (x, y) (MEASURED, golden opengl-32
    /// smooth-points-*: 51 images of points 0.5 to 9.25 wide: its size held to 0.5..10 and
    /// rounded to the granularity 1/8, the pixels with any of 4 by 4 samples at ((i + 1/2) / 4,
    /// (j + 1/2) / 4) strictly inside the circle of that diameter: GL.POINT.SMOOTH); a sample
    /// within 0.004 of the circle is GL.RASTER.TIE (13 images on both sides of it)
    fn smooth_point(&self, x: f64, y: f64) -> Res<Vec<(i64, i64, u32)>> {
        // (where alpha shows the coverage, every sample near the circle undecided)
        let alpha_used = self.pf.rgba[3].0 > 0 || self.cap(BLEND) || self.cap(ALPHA_TEST);
        hit(clause::GL_POINT_SMOOTH);
        let size = (f64::from(self.point_size.clamp(0.5, 10.0)) * 8.0 + 0.5).floor() / 8.0;
        let r = size / 2.0;
        let mut out = Vec::new();
        for py in 0..H as i64 {
            for px in 0..W as i64 {
                let mut k = 0;
                let mut dmin = f64::INFINITY;
                for i in 0..4 {
                    for j in 0..4 {
                        let (sx, sy) = (px as f64 + (i as f64 + 0.5) / 4.0, py as f64 + (j as f64 + 0.5) / 4.0);
                        let d = ((sx - x).powi(2) + (sy - y).powi(2)).sqrt();
                        if alpha_used && (d - r).abs() < 0.004 {
                            return Err(Fail::Unmodeled(open::GL_RASTER_TIE));
                        }
                        dmin = dmin.min(d);
                        k += u32::from(d < r);
                    }
                }
                if (dmin - r).abs() < 0.004 {
                    return Err(Fail::Unmodeled(open::GL_RASTER_TIE));
                }
                if k > 0 {
                    out.push((px, py, k));
                }
            }
        }
        Ok(out)
    }

    /// the pixels of a 1-pixel line from a to b in window coordinates (MEASURED, golden opengl-32
    /// prim-3..8, 14, 15, lines-*: the ends rounded to 1/64, then the diamond-exit rule, GL 1.1 3.4.1: a pixel whose diamond
    /// |x - xc| + |y - yc| < 1/2 the segment meets, unless b is inside it, inside the viewport:
    /// GL.RASTER.LINE); a segment touching a diamond's border or ending on one is GL.RASTER.TIE
    fn line_pixels(&self, a: [f64; 2], b: [f64; 2]) -> Res<Vec<(i64, i64)>> {
        hit(clause::GL_RASTER_LINE);
        // (MEASURED, golden opengl-32 lines-0: the ends first rounded to 1/64 of a pixel, a line
        // 0.0004 from a diamond falling on the other side)
        let snap = |v: f64| (v * 64.0).round() / 64.0;
        let (a, b) = ([snap(a[0]), snap(a[1])], [snap(b[0]), snap(b[1])]);
        let l1 = |p: [f64; 2], xc: f64, yc: f64| (p[0] - xc).abs() + (p[1] - yc).abs();
        let [vx, vy, vw, vh] = self.viewport;
        let mut out = Vec::new();
        if a == b {
            return Ok(out);
        }
        for y in vy.max(0)..(vy + vh).min(H as i64) {
            for x in vx.max(0)..(vx + vw).min(W as i64) {
                let (xc, yc) = (x as f64 + 0.5, y as f64 + 0.5);
                // the least |x - xc| + |y - yc| on the segment: at an end or where it crosses
                // the center's row or column
                let mut d = l1(a, xc, yc).min(l1(b, xc, yc));
                for t in [(xc - a[0]) / (b[0] - a[0]), (yc - a[1]) / (b[1] - a[1])] {
                    if (0.0..=1.0).contains(&t) {
                        d = d.min(l1([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])], xc, yc));
                    }
                }
                let e = l1(b, xc, yc);
                if d == 0.5 || e == 0.5 {
                    return Err(Fail::Unmodeled(open::GL_RASTER_TIE));
                }
                if d < 0.5 && e > 0.5 {
                    out.push((x, y));
                }
            }
        }
        Ok(out)
    }

    fn bits(&self) -> [u32; 4] {
        [self.pf.rgba[0].0, self.pf.rgba[1].0, self.pf.rgba[2].0, self.pf.rgba[3].0]
    }

    /// a color component (0..1) to a channel of `bits` (MEASURED, golden opengl-32 clear-*: on a
    /// 24-bit DIB truncated, on the others rounded to nearest)
    pub fn to_fixed(&self, c: f32, bits: u32) -> u32 {
        let m = ((1u64 << bits) - 1) as f32;
        let v = c.clamp(0.0, 1.0) * m;
        if self.bpp == 24 {
            hit(clause::GL_COLOR_TOFIXED24);
            v.floor() as u32
        } else {
            hit(clause::GL_COLOR_TOFIXED);
            (v + 0.5).floor() as u32
        }
    }

    /// a channel back to a component; a missing alpha channel reads 1 (GL.COLOR.NOALPHA)
    pub fn to_float(&self, v: u32, k: usize) -> f32 {
        let b = self.bits()[k];
        if b == 0 {
            hit(clause::GL_COLOR_NOALPHA);
            return 1.0;
        }
        // (MEASURED, golden opengl-32 read-*: times the reciprocal, not divided)
        v as f32 * (1.0 / ((1u64 << b) - 1) as f32)
    }

    /// a pixel's channels over their largest values, a missing alpha 1, in f64
    fn dst_color_raw(&self, p: [u32; 4]) -> [f64; 4] {
        let b = self.bits();
        [0, 1, 2, 3].map(|k| if b[k] == 0 { 1.0 } else { f64::from(p[k]) / ((1u64 << b[k]) - 1) as f64 })
    }

    fn dst_color(&self, i: usize) -> [f32; 4] {
        if self.pf.ty != 0 {
            // an index as a color: through GL_PIXEL_MAP_I_TO_R .. A, one zero each at first
            // (MEASURED, golden opengl-32 ci-*: RGBA reads are all zero)
            return [0.0; 4];
        }
        let p = self.fb[i];
        [self.to_float(p[0], 0), self.to_float(p[1], 1), self.to_float(p[2], 2), self.to_float(p[3], 3)]
    }

    /// the per-fragment operations in their order: scissor, alpha, stencil, depth, blending,
    /// dithering, logic operation, masks
    pub fn fragment(&mut self, f: Frag) -> Res<()> {
        if self.buffers[0] == 0 {
            hit(clause::GL_BUFFER_NONE);
            return Ok(());
        }
        if f.x < 0 || f.y < 0 || f.x >= W as i64 || f.y >= H as i64 {
            return Ok(());
        }
        let i = f.y as usize * W + f.x as usize;
        if self.cap(SCISSOR_TEST) {
            hit(clause::GL_SCISSOR_TEST);
            let [sx, sy, sw, sh] = self.scissor;
            if f.x < sx || f.y < sy || f.x >= sx + sw || f.y >= sy + sh {
                return Ok(());
            }
        }
        let c = f.c.map(|v| v.clamp(0.0, 1.0));
        if self.cap(ALPHA_TEST) {
            hit(clause::GL_ALPHA_TEST);
            // (MEASURED, golden opengl-32 alphatest-*: in 8-bit fixed point, the reference truncated)
            let a = (c[3] * 255.0 + 0.5).floor() as u64;
            let r = (self.alpha.1 * 255.0).floor() as u64;
            if !compare_u(self.alpha.0, a, r) {
                return Ok(());
            }
        }
        let has_stencil = self.pf.stencil > 0 && self.cap(STENCIL_TEST);
        let z = match f.zfixed {
            _ if !(self.pf.depth > 0 && self.cap(DEPTH_TEST)) => 0,
            Some(z) if z.bits == self.pf.depth => z.value,
            _ => Ctx::depth_fixed(f.z, self.pf.depth)?,
        };
        if has_stencil {
            hit(clause::GL_STENCIL_TEST);
            let (func, r, m) = self.stencil_func;
            let smax = (1i64 << self.pf.stencil) - 1;
            let r = (r.clamp(0, smax) as u32) & m;
            let s = self.stencil[i] & m;
            if !compare_u(func, r as u64, s as u64) {
                self.stencil_update(i, self.stencil_op[0]);
                return Ok(());
            }
        }
        if self.pf.depth > 0 && self.cap(DEPTH_TEST) {
            hit(clause::GL_DEPTH_TEST);
            // (golden opengl-32 depth-0: under NEVER Windows writes scattered pixels of the
            // primitives along diagonals, no pattern found yet)
            // (MEASURED, golden opengl-32 never-*, never-more-*: under NEVER a rectangle inside the
            // viewport draws nothing; one the viewport cuts is drawn whole, its depth not written,
            // once a rectangle inside it has been drawn under NEVER and while the depth mask is on,
            // and draws nothing before)
            let pass = if self.depth_func == NEVER {
                hit(clause::GL_DEPTH_NEVER_CLIP);
                match self.never {
                    NeverPath::Unclipped => {
                        self.never_primed = true;
                        false
                    }
                    NeverPath::Clipped => self.never_primed && self.depth_mask,
                    NeverPath::Other => return Err(Fail::Unmodeled(open::GL_DEPTH_NEVER)),
                }
            } else {
                compare_u(self.depth_func, z, self.depth[i].0)
            };
            if !pass {
                if has_stencil {
                    self.stencil_update(i, self.stencil_op[1]);
                }
                return Ok(());
            }
            // (a rectangle drawn under NEVER leaves the depth as it is)
            if self.depth_mask && self.depth_func != NEVER {
                self.depth[i] = Depth(z);
            }
        }
        if has_stencil {
            self.stencil_update(i, self.stencil_op[2]);
        }
        if self.pf.ty != 0 {
            // color index mode: the index in the format's bits, the logic operation, the index
            // write mask (MEASURED, golden opengl-32 ci-*)
            let m = low_bits(self.pf.color);
            // (MEASURED, golden opengl-32 fog-ci: a fogged polygon's fractional index dithered, the
            // matrix's entry plus one half over 16 added and truncated: GL.FOG.TOFIXED; an
            // image's fraction truncated, golden transfer-other-ci)
            let ci = if f.fixed == ToFixed::Span && self.cap(DITHER) {
                hit(clause::GL_FOG_TOFIXED);
                f64::from(f.c[0] + dither_threshold(f.x, f.y))
            } else {
                f64::from(f.c[0])
            };
            let mut v = index_bits(ci) & m;
            if self.cap(INDEX_LOGIC_OP) {
                v = logic(self.logic_op, v, self.fb[i][0]) & m;
            }
            self.fb[i][0] = (self.fb[i][0] & !self.index_mask) | (v & self.index_mask);
            return Ok(());
        }
        // (dithered glDrawPixels and masked fragments: cases dither-draw-*)
        let dither = self.cap(DITHER) && self.bits()[0] < 8;
        hit(clause::GL_DITHER_NONE);
        // what is added before truncating to a channel under 8 bits: the 4 by 4 ordered dither
        // matrix's entry plus one half, over 16; without dithering always the first entry's
        // (MEASURED, golden opengl-32 dither-fill-6, 7: undithered 1-bit channels round)
        let threshold = if dither {
            hit(clause::GL_DITHER);
            dither_threshold(f.x, f.y)
        } else if self.bits()[0] == 1 {
            0.5
        } else {
            1.0 / 32.0
        };
        let bits = self.bits();
        let mut v = [0u32; 4];
        if self.cap(BLEND) {
            v = self.blend(c, self.fb[i], dither.then_some(threshold))?;
        } else {
            for k in 0..4 {
                v[k] = if bits[k] == 0 {
                    0
                } else if dither {
                    (c[k] * ((1u64 << bits[k]) - 1) as f32 + threshold).floor() as u32
                } else if f.fixed == ToFixed::Span && bits[k] == 8 {
                    // (MEASURED, golden opengl-32 fog-rects-0, 2, 3: 288 fogged channels on 32 and
                    // 24 bits fit floor(c * 255 + 1/32), where unfogged rectangles round)
                    hit(clause::GL_FOG_TOFIXED);
                    (c[k] * 255.0 + 1.0 / 32.0).floor() as u32
                } else if f.fixed == ToFixed::Trunc && bits[k] == 8 {
                    // (MEASURED, golden opengl-32 draw-*: every type truncated to 8-bit channels; scissor-mask-*: rounded to 5-bit ones)
                    hit(clause::GL_DRAW_TOFIXED);
                    (c[k] * ((1u64 << bits[k]) - 1) as f32).floor() as u32
                } else if f.fixed != ToFixed::Trunc && bits[k] < 8 && self.color_mask == [true; 4] {
                    // (MEASURED, golden opengl-32 dither-1: with every channel writable, 768 channels of a 16-bit DIB fit
                    // floor(c * 31 + b) for b in [0.0275, 0.0314), one 32nd taken; scissor-mask-*: under a color mask rounded)
                    hit(clause::GL_COLOR_TOFIXED16);
                    (c[k] * ((1u64 << bits[k]) - 1) as f32 + threshold).floor() as u32
                } else {
                    self.to_fixed(c[k], bits[k])
                };
            }
        }
        let mut undefined = if self.cap(BLEND) { blend_undefined(self.blend, self.fb_undefined[i]) } else { [false; 4] };
        if self.cap(COLOR_LOGIC_OP) {
            hit(clause::GL_LOGICOP_APPLY);
            for k in 0..4 {
                let m = low_bits(bits[k]);
                v[k] = logic(self.logic_op, v[k], self.fb[i][k]) & m;
                undefined[k] |= logic_reads_dst(self.logic_op) && self.fb_undefined[i][k];
            }
        }
        for k in 0..4 {
            if self.color_mask[k] {
                self.fb[i][k] = v[k];
                if undefined[k] {
                    hit(clause::GL_UNDEFINED);
                }
                self.fb_undefined[i][k] = undefined[k];
            }
        }
        Ok(())
    }

    /// a fragment's window depth (0..1) for a 32-bit depth buffer (MEASURED, golden opengl-32
    /// depth-*: z times 2^32, compared in full, stored in 32 bits: 1.0 fails LEQUAL against a
    /// cleared 1.0 yet stores 0)
    pub fn depth_fixed(z: f64, bits: u32) -> Res<u64> {
        if bits == 16 {
            hit(clause::GL_DEPTH_BITS);
            return Ok(Ctx::clear_depth_fixed(z, 16)? as u64);
        }
        if bits != 32 {
            // (the formats have 32 or 16 depth bits only: WGL.FORMATS)
            return Err(Fail::Bad);
        }
        hit(clause::GL_DEPTH_FIXED);
        // (the window depth is a float: golden opengl-32 depth-types-0, 0.85 gives 0xd9999a00)
        let z = f64::from(z as f32);
        Ok((z.clamp(0.0, 1.0) * 4_294_967_296.0) as u64)
    }

    /// the clear depth in a 32-bit depth buffer (MEASURED, golden opengl-32 depth-*: z times
    /// 0x7fffffff truncated, shifted left by one: 1.0 gives 0xfffffffe)
    pub fn clear_depth_fixed(z: f64, bits: u32) -> Res<u32> {
        if bits == 16 {
            // (MEASURED, golden opengl-32 edge-depth-16, depth16-*: the same at 15 bits)
            hit(clause::GL_DEPTH_BITS);
            return Ok(((z.clamp(0.0, 1.0) * 32767.0) as u32) << 1);
        }
        if bits != 32 {
            // (the formats have 32 or 16 depth bits only: WGL.FORMATS)
            return Err(Fail::Bad);
        }
        hit(clause::GL_DEPTH_CLEAR);
        Ok(((z.clamp(0.0, 1.0) * 2_147_483_647.0) as u32) << 1)
    }

    /// a stencil image's element at (i, j) written to the stencil buffer
    fn draw_stencil(&mut self, x: i64, y: i64, v: f64) {
        if self.buffers[0] == 0 || x < 0 || y < 0 || x >= W as i64 || y >= H as i64 {
            return;
        }
        if self.cap(SCISSOR_TEST) {
            let [sx, sy, sw, sh] = self.scissor;
            if x < sx || y < sy || x >= sx + sw || y >= sy + sh {
                return;
            }
        }
        let p = y as usize * W + x as usize;
        let m = self.stencil_mask & low_bits(self.pf.stencil);
        self.stencil[p] = (self.stencil[p] & !m) | (index_bits(v) & m);
    }

    fn stencil_update(&mut self, i: usize, op: u32) {
        hit(clause::GL_STENCIL_OP);
        let smax = low_bits(self.pf.stencil);
        let s = self.stencil[i];
        let r = (self.stencil_func.1.clamp(0, smax as i64)) as u32;
        let n = match op {
            KEEP => s,
            ZERO => 0,
            REPLACE => r,
            INCR => (s + 1).min(smax),
            DECR => s.saturating_sub(1),
            _ => !s & smax,
        };
        let m = self.stencil_mask & smax;
        self.stencil[i] = (s & !m) | (n & m);
    }

    /// blending on a 32-bit DIB in the 0..255 domain (MEASURED, golden opengl-32 blend-*):
    /// SRC_ALPHA / ONE_MINUS_SRC_ALPHA in integers (GL.BLEND.ALPHA), every other pair in floats
    /// (GL.BLEND.FLOAT); a missing destination alpha makes DST_ALPHA one and ONE_MINUS_DST_ALPHA
    /// zero; the result truncated and clamped
    pub fn blend(&self, c: [f32; 4], d: [u32; 4], dither: Option<f32>) -> Res<[u32; 4]> {
        hit(clause::GL_BLEND_APPLY);
        let bits = self.bits();
        if bits[0] < 8 {

            // (MEASURED, golden opengl-32 blend-fmt-0, 3: 18432 channels of a 16-bit DIB: the
            // factors in floats on the source over 255 and the channel over its largest value,
            // the sum rounded to the channel)
            // (MEASURED, golden opengl-32 / 64 blend-dither-*: dithered, the dither matrix's
            // threshold in place of the half: GL.BLEND.DITHER)
            hit(if dither.is_some() { clause::GL_BLEND_DITHER } else { clause::GL_BLEND_SMALL });
            let dc = self.dst_color_raw(d);
            let mut r = [0u32; 4];
            for k in 0..3 {
                let m = ((1u64 << bits[k]) - 1) as f64;
                let sv = ((c[k] * 255.0 + 0.5).floor() as f64) / 255.0;
                let sa = ((c[3] * 255.0 + 0.5).floor() as f64) / 255.0;
                let x = sv * blend_factor(self.blend.0, sv, sa, dc[k], dc[3]) + dc[k] * blend_factor(self.blend.1, sv, sa, dc[k], dc[3]);
                r[k] = (x.clamp(0.0, 1.0) * m + dither.map_or(0.5, f64::from)).floor() as u32;
            }
            return Ok(r);
        }
        hit(clause::GL_BLEND_DSTALPHA);
        // (24-bit DIBs as 32-bit ones: golden opengl-32 blend-fmt-1, 4)
        let s = c.map(|v| (v * 255.0 + 0.5).floor() as u32);
        Ok(blend_bytes(self.blend, s, d, if bits[3] != 0 { Some(d[3]) } else { None }))
    }

    /// the color a vertex takes: the current color, in color index mode the index in the first
    /// component
    fn current(&self) -> [f32; 4] {
        if self.pf.ty != 0 { [self.index, 0.0, 0.0, 0.0] } else { self.color }
    }

    /// the point ties lean toward (GL.RASTER.TIE.X, GL.RASTER.TIE.Y): normalized device
    /// coordinates (0.5, -0.5) in window coordinates (golden opengl-32 tie-vp-*: it moves with the
    /// viewport, not with the object coordinates)
    fn tie_pivot(&self) -> (f64, f64) {
        let [vx, vy, vw, vh] = self.viewport.map(|v| v as f64);
        (vx + 0.75 * vw, vy + 0.25 * vh)
    }

    /// the pixels inside the scissor box when the test is on
    fn scissored(&self) -> Vec<usize> {
        let [sx, sy, sw, sh] = self.scissor;
        let on = self.cap(SCISSOR_TEST);
        (0..W * H).filter(|i| !on || {
            let (x, y) = ((i % W) as i64, (i / W) as i64);
            x >= sx && y >= sy && x < sx + sw && y < sy + sh
        }).collect()
    }

    /// the largest value of an accumulation channel, 2^(bits-1) - 1, as a float
    fn acc_max(&self, k: usize) -> f32 {
        ((1i64 << (self.pf.accum_rgba[k].max(1) - 1)) - 1) as f32
    }

    /// an accumulation channel wrapped to its bits (MEASURED, golden opengl-32 accum-*: values
    /// past the range wrap around, they are not clamped)
    fn acc_wrap(&self, v: i64, k: usize) -> i64 {
        let b = self.pf.accum_rgba[k];
        if b == 0 {
            return 0;
        }
        let m = 1i64 << b;
        (v + m / 2).rem_euclid(m) - m / 2
    }

    /// value times a color channel in accumulation units, for LOAD and ACCUM (MEASURED, golden
    /// opengl-32 accum-accum-16: the channel times the ratio of the largest values, then times
    /// value, in floats: 17 of 31 at 2.0 gives 1122 exactly, not 1121); a missing alpha is 1
    fn acc_scaled(&self, v: f32, i: usize, k: usize) -> f32 {
        let b = self.bits()[k];
        if b == 0 {
            return v * self.acc_max(k);
        }
        let cm = ((1u64 << b) - 1) as f32;
        v * (self.fb[i][k] as f32 * (self.acc_max(k) / cm))
    }

    /// a clear value (-1..1) to an accumulation channel: times the largest value, truncated, at
    /// most one less than the largest value (MEASURED, golden opengl-32 accum-5: 1.0 returns 254,
    /// not 255; accum-clear-fine-*: 144 values at 32, 24 and 16 bits, where times one less fails)
    fn acc_fixed(&self, c: f32, k: usize) -> i64 {
        let m = self.acc_max(k);
        self.acc_wrap((c * m).trunc().min(m - 1.0) as i64, k)
    }

    /// glAccum (MEASURED, golden opengl-32 accum-*, every channel of 24 reads on 32, 24 and 16
    /// bits): LOAD and ACCUM take value times the color component times the largest value,
    /// truncated in floats; MULT truncates the channel times value; ADD adds value times the
    /// largest value rounded up; RETURN writes the channel times value over the largest value
    /// times the color channel's largest value, rounded on 16 bits, truncated on the others; then
    /// clamped. All wrap at the channel's bits.
    /// glPopAttrib: the groups of the mask back as they were pushed (GL 1.1 6.1.13's table)
    fn pop_attrib(&mut self, mask: u32, s: Ctx) {
        hit(clause::GL_ATTRIB_POP);
        let mut caps = |group: u32, list: &[u32]| {
            if mask & group != 0 || mask & attrib::ENABLE != 0 {
                for c in list {
                    self.caps.insert(*c, s.caps[c]);
                }
            }
        };
        caps(attrib::POINT, &[0xb10]);
        caps(attrib::LINE, &[0xb20, 0xb24]);
        caps(attrib::POLYGON, &[0xb41, 0xb42, 0xb44, 0x2a01, 0x2a02, 0x8037]);
        caps(attrib::LIGHTING, &[0xb50, 0xb57, 0x4000, 0x4001, 0x4002, 0x4003, 0x4004, 0x4005, 0x4006, 0x4007]);
        caps(attrib::FOG, &[0xb60]);
        caps(attrib::DEPTH, &[0xb71]);
        caps(attrib::STENCIL, &[0xb90]);
        caps(attrib::TRANSFORM, &[0xba1, 0x3000, 0x3001, 0x3002, 0x3003, 0x3004, 0x3005]);
        caps(attrib::COLOR, &[0xbc0, 0xbd0, 0xbe2, 0xbf1, 0xbf2]);
        caps(attrib::SCISSOR, &[0xc11]);
        caps(attrib::TEXTURE, &[0xc60, 0xc61, 0xc62, 0xc63, 0xde0, 0xde1]);
        caps(attrib::EVAL, &[0xd80, 0xd90, 0xd91, 0xd92, 0xd93, 0xd94, 0xd95, 0xd96, 0xd97, 0xd98, 0xdb0, 0xdb1, 0xdb2, 0xdb3, 0xdb4, 0xdb5, 0xdb6, 0xdb7, 0xdb8]);
        if mask & attrib::CURRENT != 0 {
            self.color = s.color;
            self.normal = s.normal;
        }
        if mask & attrib::POINT != 0 {
            self.point_size = s.point_size;
        }
        if mask & attrib::LINE != 0 {
            self.line_width = s.line_width;
        }
        if mask & attrib::POLYGON != 0 {
            self.cull_mode = s.cull_mode;
            self.front_face = s.front_face;
            self.poly_mode = s.poly_mode;
            self.offset = s.offset;
        }
        if mask & attrib::PIXEL_MODE != 0 {
            self.transfer = s.transfer;
            self.map = s.map;
            self.index_shift = s.index_shift;
            self.depth_transfer = s.depth_transfer;
            self.zoom = s.zoom;
            self.buffers[1] = s.buffers[1];
        }
        if mask & attrib::LIGHTING != 0 {
            self.shade = s.shade;
        }
        if mask & attrib::FOG != 0 {
            self.fog = s.fog;
        }
        if mask & attrib::DEPTH != 0 {
            self.depth_func = s.depth_func;
            self.depth_mask = s.depth_mask;
            self.clear_depth = s.clear_depth;
        }
        if mask & attrib::ACCUM != 0 {
            self.clear_accum = s.clear_accum;
        }
        if mask & attrib::STENCIL != 0 {
            self.stencil_func = s.stencil_func;
            self.stencil_op = s.stencil_op;
            self.stencil_mask = s.stencil_mask;
            self.clear_stencil = s.clear_stencil;
        }
        if mask & attrib::VIEWPORT != 0 {
            self.viewport = s.viewport;
            self.depth_range = s.depth_range;
        }
        if mask & attrib::TRANSFORM != 0 {
            self.mode = s.mode;
        }
        if mask & attrib::COLOR != 0 {
            self.alpha = s.alpha;
            self.blend = s.blend;
            self.logic_op = s.logic_op;
            self.color_mask = s.color_mask;
            self.color_mask_raw = s.color_mask_raw;
            self.clear_color = s.clear_color;
            self.buffers[0] = s.buffers[0];
        }
        if mask & attrib::HINT != 0 {
            self.hints = s.hints;
        }
        if mask & attrib::SCISSOR != 0 {
            self.scissor = s.scissor;
        }
    }

    fn accum(&mut self, op: AccumOp, v: f32) -> Res<()> {
        hit(clause::GL_ACCUM);
        let idx = self.scissored();
        let mut acc = std::mem::take(&mut self.acc);
        // (content never loaded or cleared is undefined, GL 1.1 4.2.3, and what is made from it
        // too: GL.UNDEFINED)
        for &i in &idx {
            for k in 0..4 {
                let fb = (!self.fb_undefined[i][k]).then_some(());
                acc[i][k] = match op {
                    AccumOp::Load => fb.map(|()| self.acc_wrap(self.acc_scaled(v, i, k).trunc() as i64, k)),
                    AccumOp::Accum => acc[i][k].zip(fb).map(|(a, ())| self.acc_wrap(a + self.acc_scaled(v, i, k).trunc() as i64, k)),
                    AccumOp::Mult => acc[i][k].map(|a| self.acc_wrap((a as f32 * v).trunc() as i64, k)),
                    AccumOp::Add => acc[i][k].map(|a| self.acc_wrap(a + (v * self.acc_max(k) + 0.5).trunc() as i64, k)),
                    AccumOp::Return => acc[i][k],
                };
            }
        }
        if op == AccumOp::Return && self.buffers[0] != 0 {
            hit(clause::GL_ACCUM_STEPS);
            let bits = self.bits();
            for &i in &idx {
                for k in 0..4 {
                    if bits[k] == 0 || !self.color_mask[k] {
                        continue;
                    }
                    let Some(a) = acc[i][k] else {
                        hit(clause::GL_UNDEFINED);
                        self.fb_undefined[i][k] = true;
                        continue;
                    };
                    let cm = ((1u64 << bits[k]) - 1) as f32;
                    let x = a as f32 * (v / self.acc_max(k)) * cm;
                    let m = if self.bpp <= 16 { (x + 0.5).floor() } else { x.trunc() };
                    self.fb[i][k] = m.clamp(0.0, cm) as u32;
                    self.fb_undefined[i][k] = false;
                }
            }
        }
        self.acc = acc;
        Ok(())
    }

    fn clear(&mut self, m: u32) -> Res<()> {
        hit(clause::GL_CLEAR);
        let m = if self.buffers[0] == 0 && m & COLOR_BUFFER_BIT != 0 {
            hit(clause::GL_BUFFER_NONE);
            m & !COLOR_BUFFER_BIT
        } else {
            m
        };
        if m & ACCUM_BUFFER_BIT != 0 && self.pf.accum > 0 {
            hit(clause::GL_CLEAR_ACCUM);
            let cv: Vec<i64> = (0..4).map(|k| self.acc_fixed(self.clear_accum[k], k)).collect();
            for i in self.scissored() {
                self.acc[i] = [Some(cv[0]), Some(cv[1]), Some(cv[2]), Some(cv[3])];
            }
        }
        let dither = self.cap(DITHER) && self.bits()[0] < 8 && m & COLOR_BUFFER_BIT != 0;
        let bits = self.bits();
        let cv: Vec<u32> = (0..4).map(|k| if bits[k] == 0 { 0 } else { self.to_fixed(self.clear_color[k], bits[k]) }).collect();
        // a dithered clear: each pixel as a fragment of the clear color would be (GL.DITHER)
        let dithered = |x: i64, y: i64, k: usize| {
            let m = ((1u64 << bits[k]) - 1) as f32;
            (self.clear_color[k].clamp(0.0, 1.0) * m + dither_threshold(x, y)).floor() as u32
        };
        let z = if m & DEPTH_BUFFER_BIT != 0 { Ctx::clear_depth_fixed(self.clear_depth, self.pf.depth)? } else { 0 };
        let smax = if self.pf.stencil == 0 { 0 } else { low_bits(self.pf.stencil) };
        for y in 0..H as i64 {
            for x in 0..W as i64 {
                if self.cap(SCISSOR_TEST) {
                    let [sx, sy, sw, sh] = self.scissor;
                    if x < sx || y < sy || x >= sx + sw || y >= sy + sh {
                        continue;
                    }
                }
                let i = y as usize * W + x as usize;
                if m & COLOR_BUFFER_BIT != 0 && self.pf.ty != 0 {
                    let v = index_bits(f64::from(self.clear_index)) & (low_bits(self.pf.color));
                    self.fb[i][0] = (self.fb[i][0] & !self.index_mask) | (v & self.index_mask);
                } else if m & COLOR_BUFFER_BIT != 0 {
                    for k in 0..4 {
                        if self.color_mask[k] {
                            self.fb[i][k] = if dither && bits[k] > 0 { dithered(x, y, k) } else { cv[k] };
                            self.fb_undefined[i][k] = false;
                        }
                    }
                }
                if m & DEPTH_BUFFER_BIT != 0 && self.depth_mask {
                    self.depth[i] = Depth(u64::from(z));
                }
                if m & STENCIL_BUFFER_BIT != 0 {
                    let mk = self.stencil_mask & smax;
                    self.stencil[i] = (self.stencil[i] & !mk) | (self.clear_stencil as u32 & mk);
                }
            }
        }
        Ok(())
    }
}

/// blending of 8-bit channels, `s` the fragment, `d` the framebuffer without alpha (Ctx::blend)
/// a blend factor in floats: source component, source alpha, destination component and alpha
fn blend_factor(f: u32, s: f64, sa: f64, d: f64, da: f64) -> f64 {
    match f {
        ZERO => 0.0,
        ONE => 1.0,
        SRC_COLOR => s,
        ONE_MINUS_SRC_COLOR => 1.0 - s,
        SRC_ALPHA => sa,
        ONE_MINUS_SRC_ALPHA => 1.0 - sa,
        DST_ALPHA => da,
        ONE_MINUS_DST_ALPHA => 1.0 - da,
        DST_COLOR => d,
        ONE_MINUS_DST_COLOR => 1.0 - d,
        _ => sa.min(1.0 - da),
    }
}

pub fn blend_bytes(f: (u32, u32), s: [u32; 4], d: [u32; 4], dst_alpha: Option<u32>) -> [u32; 4] {
    let mut r = [0u32; 4];
    let n = if dst_alpha.is_some() { 4 } else { 3 };
    // (MEASURED, golden opengl-32 blend-dstalpha-*: with destination alpha this pair too in floats)
    if f == (SRC_ALPHA, ONE_MINUS_SRC_ALPHA) && dst_alpha.is_none() {
        hit(clause::GL_BLEND_ALPHA);
        let a = s[3] as i64;
        for k in 0..n {
            let v = (((s[k] as i64 - 1).max(0) * (a - 1)) >> 8) + ((d[k] as i64 * (256 - a)) >> 8);
            r[k] = v.clamp(0, 255) as u32;
        }
        return r;
    }
    hit(clause::GL_BLEND_FLOAT);
    let inv = 1f32 / 255f32;
    let da = dst_alpha.unwrap_or(255);
    let term = |v: u32, f: u32, k: usize| -> f32 {
        let v = v as f32;
        let x = match f {
            ZERO => return 0.0,
            ONE => return v,
            ONE_MINUS_DST_ALPHA if dst_alpha.is_none() => return 0.0,
            DST_ALPHA if dst_alpha.is_none() => return v,
            DST_ALPHA | ONE_MINUS_DST_ALPHA => da,
            // (GL 1.1 4.1.6: the saturate factor is 1 for alpha)
            SRC_ALPHA_SATURATE if k == 3 => return v,
            SRC_COLOR | ONE_MINUS_SRC_COLOR => s[k],
            SRC_ALPHA | ONE_MINUS_SRC_ALPHA => s[3],
            DST_COLOR | ONE_MINUS_DST_COLOR => d[k],
            // (MEASURED, golden opengl-32 blend-fmt-5: min(As, 1 - Ad) in floats)
            _ => return v * (s[3] as f32 * inv).min(1.0 - da as f32 * inv),
        };
        // (MEASURED, golden opengl-32 blend-dstalpha-*: every pair on 145408 channels: an alpha
        // factor is a float of its own, x / 255, the channel times it; a color factor is the
        // channel times x, over 255; a one-minus factor 1 - x / 255)
        match f {
            ONE_MINUS_SRC_COLOR | ONE_MINUS_SRC_ALPHA | ONE_MINUS_DST_COLOR | ONE_MINUS_DST_ALPHA => v * (1.0 - x as f32 * inv),
            SRC_ALPHA | DST_ALPHA => v * (x as f32 * inv),
            _ => (v * x as f32) * inv,
        }
    };
    for k in 0..n {
        let v = term(d[k], f.1, k) + term(s[k], f.0, k);
        r[k] = (v.floor().clamp(0.0, 255.0)) as u32;
    }
    r
}

/// which channels of a blend's result are undefined, given the destination's (GL.UNDEFINED):
/// channel k reads the destination's channel k unless the destination factor is zero and the
/// source factor no destination color, and every channel reads the destination alpha under a
/// factor made from it
pub fn blend_undefined(f: (u32, u32), dst: [bool; 4]) -> [bool; 4] {
    let reads_color = f.1 != ZERO || matches!(f.0, DST_COLOR | ONE_MINUS_DST_COLOR);
    let reads_alpha = [f.0, f.1].iter().any(|g| matches!(*g, DST_ALPHA | ONE_MINUS_DST_ALPHA | SRC_ALPHA_SATURATE));
    [0, 1, 2, 3].map(|k| (reads_color && dst[k]) || (reads_alpha && dst[3]))
}

/// whether a logic operation's result depends on the destination (all but CLEAR, COPY,
/// COPY_INVERTED, SET)
pub fn logic_reads_dst(op: u32) -> bool {
    !matches!(op, 0x1500 | 0x1503 | 0x150c | 0x150f)
}

fn compare_u(f: u32, a: u64, b: u64) -> bool {
    match f {
        0x200 => false,
        0x201 => a < b,
        0x202 => a == b,
        0x203 => a <= b,
        0x204 => a > b,
        0x205 => a != b,
        0x206 => a >= b,
        _ => true,
    }
}


/// the sixteen logic operations, s the fragment, d the framebuffer (glLogicOp's table)
pub fn logic(op: u32, s: u32, d: u32) -> u32 {
    match op {
        0x1500 => 0,
        0x1501 => s & d,
        0x1502 => s & !d,
        0x1503 => s,
        0x1504 => !s & d,
        0x1505 => d,
        0x1506 => s ^ d,
        0x1507 => s | d,
        0x1508 => !(s | d),
        0x1509 => !(s ^ d),
        0x150a => !d,
        0x150b => s | !d,
        0x150c => !s,
        0x150d => !s | d,
        0x150e => !(s & d),
        _ => !0,
    }
}

/// (components, which: indices into r g b a, or 4 luminance, 5 depth, 6 stencil, 7 index) of a pixel format
fn components(f: u32) -> Option<Vec<usize>> {
    Some(match f {
        RED => vec![0],
        GREEN => vec![1],
        BLUE => vec![2],
        ALPHA => vec![3],
        RGB => vec![0, 1, 2],
        RGBA => vec![0, 1, 2, 3],
        LUMINANCE => vec![4],
        LUMINANCE_ALPHA => vec![4, 3],
        BGR_EXT => vec![2, 1, 0],
        BGRA_EXT => vec![2, 1, 0, 3],
        DEPTH_COMPONENT => vec![5],
        STENCIL_INDEX => vec![6],
        COLOR_INDEX => vec![7],
        _ => return None,
    })
}

/// bytes of one element of a pixel type
fn type_size(t: u32) -> Option<usize> {
    Some(match t {
        UNSIGNED_BYTE | BYTE => 1,
        UNSIGNED_SHORT | SHORT => 2,
        UNSIGNED_INT | INT | FLOAT => 4,
        BITMAP => 0,
        _ => return None,
    })
}

/// the bit of pixel (i, j) of a GL_BITMAP image under the pack / unpack parameters: (byte, the
/// bit's mask) (GL 1.1 3.6.3: rows of the row length in bits padded to the alignment, the first
/// pixel in a byte's high bit unless GL_UNPACK_LSB_FIRST)
fn bitmap_bit(store: &[i64; 6], w: i64, i: usize, j: usize) -> (usize, u8) {
    hit(clause::GL_PIXELSTORE_LAYOUT);
    let l = if store[2] > 0 { store[2] as usize } else { w as usize };
    let a = store[5] as usize;
    let k = a * l.div_ceil(8 * a);
    let p = store[4] as usize + i;
    let bit = p % 8;
    (store[3] as usize * k + j * k + p / 8, if store[1] != 0 { 1 << bit } else { 0x80 >> bit })
}

/// the element offsets of a w x h image under the pack / unpack parameters (glPixelStore):
/// (start of row j, elements a row), in elements of `size` bytes
fn layout(store: &[i64; 6], n: usize, size: usize, w: i64) -> (usize, usize) {
    hit(clause::GL_PIXELSTORE_LAYOUT);
    let l = if store[2] > 0 { store[2] as usize } else { w as usize };
    let a = store[5] as usize;
    let k = if size >= a { n * l } else { a / size * (size * n * l).div_ceil(a) };
    (store[4] as usize * n + store[3] as usize * k, k)
}

/// a depth sample: the fragment's fixed-point depth, which at 1.0 is 2^32 in a 32-bit buffer;
/// UNSIGNED_INT reads see its low 32 bits, the depth test and the other read types all of it
/// (golden opengl-32 depth-types-0, draw-depthstencil-0: a depth image of 1.0 then one of 0.7
/// under LESS draws the second)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Depth(pub u64);

/// the primitive a fragment under GL_NEVER comes from
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NeverPath {
    /// a glRect inside the viewport (x86 executor)
    Unclipped,
    /// a glRect the viewport cuts (x86 executor)
    Clipped,
    /// anything else (GL.DEPTH.NEVER)
    Other,
}

impl Depth {
    /// the bits the buffer holds
    pub fn bits(self) -> u32 {
        (self.0 & 0xffff_ffff) as u32
    }
}

/// the mask of a value's low `n` bits (all 32 from 32 up: a format's channel may have 32)
pub fn low_bits(n: u32) -> u32 {
    u32::try_from((1u64 << n.min(32)) - 1).unwrap_or(u32::MAX)
}

/// an index's bits: truncated to a 32-bit integer, an index out of its range giving 0x80000000
/// (MEASURED, golden opengl-32 ci-types-*: indices drawn as floats past 2^31 store 0)
pub fn index_bits(v: f64) -> u32 {
    let t = v.trunc();
    if t >= -2147483648.0 && t < 2147483648.0 {
        // (in range, so the cast is exact)
        (t as i32) as u32
    } else {
        0x8000_0000
    }
}

/// a depth image's element: its window depth, and its bits in a 32-bit buffer (MEASURED, golden
/// opengl-32 draw-depthstencil-0: the depth in floats, BYTE as (2b + 1) / 255, SHORT as (2s + 1)
/// times 1/65535, INT as i times the float 1/2147482624, the unsigned bytes and shorts over
/// 2^n - 1, then the float z 2^31 truncated and shifted left by one; UNSIGNED_INT its bits with
/// the lowest cleared; GL.DRAW.DEPTHSTENCIL)
pub fn depth_element(b: &[u8], t: u32, (scale, bias): (f32, f32)) -> (f64, u64) {
    let w = |n: usize| -> [u8; 4] {
        let mut x = [0u8; 4];
        x[..n].copy_from_slice(&b[..n]);
        x
    };
    let z: f32 = match t {
        BYTE => (2.0 * f32::from(b[0] as i8) + 1.0) / 255.0,
        UNSIGNED_BYTE => f32::from(b[0]) / 255.0,
        SHORT => (2.0 * f32::from(i16::from_le_bytes([b[0], b[1]])) + 1.0) * (1.0 / 65535.0),
        UNSIGNED_SHORT => f32::from(u16::from_le_bytes([b[0], b[1]])) / 65535.0,
        INT => i32::from_le_bytes(w(4)) as f32 * (1.0 / 2_147_482_624.0),
        UNSIGNED_INT if (scale, bias) == (1.0, 0.0) => {
            let u = u32::from_le_bytes(w(4));
            return (f64::from(u) / 4_294_967_295.0, u64::from(u & !1));
        }
        UNSIGNED_INT => (f64::from(u32::from_le_bytes(w(4))) / 4_294_967_295.0) as f32,
        _ => f32::from_le_bytes(w(4)),
    };
    // (GL_DEPTH_SCALE and GL_DEPTH_BIAS, then clamped: golden opengl-32 transfer-other)
    let z = z * scale + bias;
    let z = if z.is_nan() { 0.0 } else { z.clamp(0.0, 1.0) };
    // (z 2^31 is at most 2^31: the cast is exact for the floats it takes)
    (f64::from(z), ((z * 2_147_483_648.0) as u64) << 1)
}

/// a color index element's value, each type as the integer or float it holds (unsigned bytes in
/// ci-*, the others in ci-types-*)
pub fn index_value(b: &[u8], t: u32) -> Res<f64> {
    let a = |n: usize| -> [u8; 4] {
        let mut x = [0u8; 4];
        x[..n].copy_from_slice(&b[..n]);
        x
    };
    match t {
        UNSIGNED_BYTE => Ok(f64::from(b[0])),
        BYTE => Ok(f64::from(b[0] as i8)),
        UNSIGNED_SHORT => Ok(f64::from(u16::from_le_bytes([b[0], b[1]]))),
        SHORT => Ok(f64::from(i16::from_le_bytes([b[0], b[1]]))),
        UNSIGNED_INT => Ok(f64::from(u32::from_le_bytes(a(4)))),
        INT => Ok(f64::from(i32::from_le_bytes(a(4)))),
        FLOAT => Ok(f64::from(f32::from_le_bytes(a(4)))),
        _ => Err(Fail::Bad),
    }
}

/// a component (0..1) to an element of a type (MEASURED, golden opengl-32 read-*: in floats,
/// truncated but for UNSIGNED_BYTE; signed types scale by half of 2^n - 1, the 32-bit ones by the largest float below
/// 2^31 - 1024 and twice that; GL.READ.CONVERT)
pub fn from_float(c: f32, t: u32) -> Vec<u8> {
    hit(clause::GL_READ_CONVERT);
    match t {
        // rounded (scissor-mask-*: 5-bit channels of a 16-bit DIB read to the nearest byte)
        UNSIGNED_BYTE => vec![(c * 255.0 + 0.5).floor() as u8],
        BYTE => vec![(c * 127.5).floor() as i8 as u8],
        UNSIGNED_SHORT => ((c * 65535.0).floor() as u16).to_le_bytes().to_vec(),
        SHORT => ((c * 32767.5).floor() as i16).to_le_bytes().to_vec(),
        UNSIGNED_INT => ((c * 4_294_964_992.0) as u32).to_le_bytes().to_vec(),
        INT => ((c * 2_147_482_496.0) as i32).to_le_bytes().to_vec(),
        _ => c.to_le_bytes().to_vec(),
    }
}

/// an element of a type to a component (glDrawPixels' table; GL.DRAW.CONVERT)
pub fn to_float(b: &[u8], t: u32) -> f64 {
    hit(clause::GL_DRAW_CONVERT);
    match t {
        UNSIGNED_BYTE => b[0] as f64 / 255.0,
        BYTE => (2.0 * b[0] as i8 as f64 + 1.0) / 255.0,
        UNSIGNED_SHORT => u16::from_le_bytes([b[0], b[1]]) as f64 / 65535.0,
        SHORT => (2.0 * i16::from_le_bytes([b[0], b[1]]) as f64 + 1.0) / 65535.0,
        UNSIGNED_INT => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64 / 4_294_967_295.0,
        INT => (2.0 * i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64 + 1.0) / 4_294_967_295.0,
        _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
    }
}

impl Ctx {
    /// the common errors of glReadPixels / glDrawPixels; Ok(None) when the call is refused
    fn pixel_args(&mut self, w: i64, h: i64, f: u32, t: u32, read: bool) -> Res<Option<(Vec<usize>, usize)>> {
        if self.in_begin.is_some() {
            self.error(INVALID_OPERATION);
            return Ok(None);
        }
        let (Some(comps), Some(size)) = (components(f), type_size(t)) else {
            hit(clause::GL_PIXELS_ENUM);
            self.error(INVALID_ENUM);
            return Ok(None);
        };
        if w < 0 || h < 0 {
            hit(clause::GL_PIXELS_VALUE);
            self.error(INVALID_VALUE);
            return Ok(None);
        }
        if read && f == COLOR_INDEX && self.pf.ty == 0 {
            // (MEASURED, golden opengl-32 edge-pixels-errors: before the type is looked at)
            hit(clause::GL_PIXELS_NOBUFFER);
            self.error(INVALID_OPERATION);
            return Ok(None);
        }
        if t == BITMAP {
            if f != COLOR_INDEX && f != STENCIL_INDEX {
                // (MEASURED, golden opengl-32 read-formats, read-pack-*, edge-draw-bitmap:
                // glReadPixels INVALID_OPERATION, glDrawPixels INVALID_ENUM)
                if !read {
                    hit(clause::GL_DRAW_BITMAP);
                    self.error(INVALID_ENUM);
                    return Ok(None);
                }
                hit(clause::GL_READ_BITMAP);
                self.error(INVALID_OPERATION);
                return Ok(None);
            }
            hit(clause::GL_PIXELS_BITMAP_INDEX);
        }
        if (f == COLOR_INDEX && read && self.pf.ty == 0) || (f == DEPTH_COMPONENT && self.pf.depth == 0) || (f == STENCIL_INDEX && self.pf.stencil == 0) {
            hit(clause::GL_PIXELS_NOBUFFER);
            self.error(INVALID_OPERATION);
            return Ok(None);
        }
        Ok(Some((comps, size)))
    }

    /// an index through GL_INDEX_SHIFT (left, or right when negative) and GL_INDEX_OFFSET
    /// (MEASURED, golden opengl-32 transfer-other-ci: the fraction a right shift leaves truncated
    /// with the index)
    fn shifted(&self, v: f64) -> f64 {
        let (sh, off) = self.index_shift;
        if (sh, off) == (0, 0) {
            return v;
        }
        hit(clause::GL_PIXELTRANSFER_SHIFT);
        v * 2f64.powi(sh.clamp(-64, 64) as i32) + off as f64
    }

    /// an index or stencil value (after the shift and offset) through GL_PIXEL_MAP_I_TO_I or
    /// GL_PIXEL_MAP_S_TO_S under GL_MAP_COLOR in color index mode or GL_MAP_STENCIL (MEASURED,
    /// golden opengl-32 transfer-maps-ci, transfer-maps-stencil: the initial maps hold one entry
    /// of 0, so every value maps to 0, drawn and read: GL.PIXELTRANSFER.MAPINDEX)
    fn index_map(&self, stencil: bool, v: f64) -> f64 {
        if self.map[usize::from(stencil)] {
            hit(clause::GL_PIXELTRANSFER_MAPINDEX);
            0.0
        } else {
            v
        }
    }

    fn transfer(&self, c: [f64; 4]) -> Res<[f64; 4]> {
        if self.map[0] {
            // (MEASURED, golden opengl-32 transfer-map-color: through the initial maps, each
            // one entry of 0, every component of an image drawn or read is 0: GL.PIXELTRANSFER.MAP)
            hit(clause::GL_PIXELTRANSFER_MAP);
            return Ok([0.0; 4]);
        }
        let mut r = c;
        for k in 0..4 {
            let (s, b) = self.transfer[k];
            if (s, b) != (1.0, 0.0) {
                hit(clause::GL_PIXELTRANSFER_APPLY);
            }
            r[k] = (c[k] * s as f64 + b as f64).clamp(0.0, 1.0);
        }
        Ok(r)
    }

    pub fn read_pixels(&mut self, x: i64, y: i64, w: i64, h: i64, f: u32, t: u32, buf: &mut [u8], undefined: &mut [bool]) -> Res<()> {
        let Some((comps, size)) = self.pixel_args(w, h, f, t, true)? else { return Ok(()) };
        if self.pack[0] != 0 && size > 1 {
            hit(clause::GL_PIXELSTORE_SWAP);
        }
        hit(clause::GL_READ);
        if size == 0 {
            // indices or stencil values as GL_BITMAP: each pixel's low bit, the other bits of a
            // byte left as they are (MEASURED, golden opengl-32 edge-bitmap-read-stencil,
            // bitmap-index-*: GL.PIXELS.BITMAP.INDEX)
            for j in 0..h as usize {
                for i in 0..w as usize {
                    let (px, py) = (x + i as i64, y + j as i64);
                    if px < 0 || py < 0 || px >= W as i64 || py >= H as i64 {
                        hit(clause::GL_READ_OUTSIDE);
                        continue;
                    }
                    let p = py as usize * W + px as usize;
                    let v = if f == COLOR_INDEX { self.fb[p][0] } else { self.stencil[p] };
                    let v = if self.map[usize::from(f != COLOR_INDEX)] { self.index_map(f != COLOR_INDEX, 0.0) as u32 } else { v };
                    let (at, m) = bitmap_bit(&self.pack, w, i, j);
                    let Some(b) = buf.get_mut(at) else {
                        hit(clause::GL_READ_BUFFER);
                        continue;
                    };
                    if v & 1 != 0 { *b |= m } else { *b &= !m }
                }
            }
            return Ok(());
        }
        let (start, k) = layout(&self.pack, comps.len(), size, w);
        for j in 0..h as usize {
            for i in 0..w as usize {
                let (px, py) = (x + i as i64, y + j as i64);
                if px < 0 || py < 0 || px >= W as i64 || py >= H as i64 {
                    // (MEASURED, golden opengl-32 edge-read-outside: the elements of pixels
                    // outside the window are left as they are)
                    hit(clause::GL_READ_OUTSIDE);
                    continue;
                }
                let p = py as usize * W + px as usize;
                let c = self.dst_color(p).map(|v| v as f64);
                let c = self.transfer(c)?.map(|v| v as f32);
                for (n, which) in comps.iter().enumerate() {
                    let und = match which {
                        0..=3 => self.fb_undefined[p][*which],
                        4 => self.fb_undefined[p][..3].iter().any(|u| *u),
                        _ => false,
                    };
                    let mut e = match which {
                        0..=3 => from_float(c[*which], t),
                        4 => {
                            hit(clause::GL_READ_LUMINANCE);
                            from_float((c[0] + c[1] + c[2]).min(1.0), t)
                        }
                        5 => {
                            // (MEASURED, golden opengl-32 depth-*: UNSIGNED_INT gives the 32 bits as they are;
                            // depth16-*: 16 bits as they are in UNSIGNED_INT's top half and in
                            // UNSIGNED_SHORT, the other types from the value over 0xfffe)
                            let dv = self.depth[p];
                            let d = dv.bits();
                            let (ds, db) = self.depth_transfer;
                            match (self.pf.depth, t) {
                                // (MEASURED, golden opengl-32 transfer-other: under a depth scale
                                // or bias every type from the scaled value converted as colors
                                // are: GL.PIXELTRANSFER.SHIFT)
                                (depth, _) if (ds, db) != (1.0, 0.0) => {
                                    hit(clause::GL_PIXELTRANSFER_SHIFT);
                                    let z = if depth == 16 { d as f32 / 65534.0 } else { (dv.0 as f64 / 4_294_967_296.0) as f32 };
                                    from_float((z * ds + db).clamp(0.0, 1.0), t)
                                }
                                (32, UNSIGNED_INT) => {
                                    hit(clause::GL_READ_DEPTH);
                                    d.to_le_bytes().to_vec()
                                }
                                (16, UNSIGNED_INT) => {
                                    hit(clause::GL_READ_DEPTH);
                                    (d << 16).to_le_bytes().to_vec()
                                }
                                (16, UNSIGNED_SHORT) => {
                                    hit(clause::GL_READ_DEPTH_16);
                                    u16::try_from(d).map_err(|_| Fail::Bad)?.to_le_bytes().to_vec()
                                }
                                (16, BYTE | UNSIGNED_BYTE | SHORT | INT | FLOAT) => {
                                    hit(clause::GL_READ_DEPTH_16);
                                    hit(clause::GL_READ_DEPTH_TYPE);
                                    from_float(d as f32 / 65534.0, t)
                                }
                                (32, UNSIGNED_SHORT) => {
                                    // (MEASURED, golden opengl-32 depth-types-0: the top 16 bits)
                                    hit(clause::GL_READ_DEPTH_TYPE);
                                    u16::try_from(d >> 16).map_err(|_| Fail::Bad)?.to_le_bytes().to_vec()
                                }
                                (32, BYTE | UNSIGNED_BYTE | SHORT | INT | FLOAT) => {
                                    // (MEASURED, golden opengl-32 depth-types-0: the other types
                                    // from the value over 2^32 as a float, a fragment at 1.0
                                    // reading 1.0 though UNSIGNED_INT gives its 32 bits, 0)
                                    hit(clause::GL_READ_DEPTH_TYPE);
                                    from_float((dv.0 as f64 / 4_294_967_296.0) as f32, t)
                                }
                                // (the formats have 32 or 16 depth bits, pixel_args takes these types only)
                                _ => return Err(Fail::Bad),
                            }
                        }
                        _ => {
                            let s = if *which == 7 {
                                // (MEASURED, golden opengl-32 ci-*: the index as it is)
                                hit(clause::GL_COLORINDEX);
                                self.fb[p][0]
                            } else {
                                hit(clause::GL_READ_STENCIL);
                                self.stencil[p]
                            };
                            // (the index shift and offset, the value's low 32 bits)
                            let s = u64::from(index_bits(self.index_map(*which != 7, self.shifted(f64::from(s)))));
                            // (MEASURED, golden opengl-32 ci-types-*: the signed types keep the
                            // index's low 7, 15 or 31 bits)
                            let s = match t {
                                BYTE => s & 0x7f,
                                SHORT => s & 0x7fff,
                                INT => s & 0x7fff_ffff,
                                _ => s,
                            };
                            match size {
                                1 => vec![s as u8],
                                2 => (s as u16).to_le_bytes().to_vec(),
                                _ if t == FLOAT => (s as f32).to_le_bytes().to_vec(),
                                _ => (s as u32).to_le_bytes().to_vec(),
                            }
                        }
                    };
                    if self.pack[0] != 0 {
                        e.reverse();
                    }
                    let at = (start + j * k + i * comps.len() + n) * size;
                    if at + size > buf.len() {
                        // (MEASURED, golden opengl-32 read-pack-17: Windows writes past the
                        // executor's buffer, which the answer does not show; the elements inside
                        // it are written as any others: GL.READ.BUFFER)
                        hit(clause::GL_READ_BUFFER);
                        continue;
                    }
                    buf[at..at + size].copy_from_slice(&e);
                    if und {
                        hit(clause::GL_UNDEFINED);
                        undefined[at..at + size].fill(true);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn draw_pixels(&mut self, w: i64, h: i64, f: u32, t: u32, data: &[u8]) -> Res<()> {
        let Some((comps, size)) = self.pixel_args(w, h, f, t, false)? else { return Ok(()) };
        let Some([rx, ry, rz]) = self.raster else {
            hit(clause::GL_RASTER_POS);
            return Ok(());
        };
        // (MEASURED, golden opengl-32 other-calls-0: unzoomed, the image's first pixel at the
        // raster position rounded, halves up: GL.RASTER.POS)
        let (ox, oy) = ((rx + 0.5).floor() as i64, (ry + 0.5).floor() as i64);
        let zoomed = self.zoom != (1.0, 1.0);
        // (fog on images at the raster position's eye distance, GL 1.1 3.9: golden opengl-32
        // other-calls-5 at a factor of 1)
        let fog = |me: &Self, c: [f32; 4]| if me.cap(FOG) { hit(clause::GL_FOG_APPLY); me.fog_at(c, me.raster_eye) } else { c };
        hit(clause::GL_DRAW);
        if size == 0 {
            // a GL_BITMAP image of color indices: each bit the index 0 or 1 (MEASURED, golden
            // opengl-32 bitmap-index-*: GL.PIXELS.BITMAP.INDEX)
            for j in 0..h as usize {
                for i in 0..w as usize {
                    let (at, m) = bitmap_bit(&self.unpack, w, i, j);
                    let Some(b) = data.get(at) else { return Err(Fail::Unmodeled(open::GL_DRAW_BUFFER)) };
                    let v: f32 = if b & m != 0 { 1.0 } else { 0.0 };
                    if f == STENCIL_INDEX {
                        for (x, y) in self.image_pixels(rx, ry, i, j) {
                            self.draw_stencil(x, y, f64::from(v));
                        }
                        continue;
                    }
                    let c = if self.pf.ty != 0 {
                        hit(clause::GL_COLORINDEX);
                        [v, 0.0, 0.0, 0.0]
                    } else {
                        hit(clause::GL_DRAW_INDEX);
                        self.transfer([0.0; 4])?.map(|v| v as f32)
                    };
                    let c = fog(self, c);
                    for (x, y) in self.image_pixels(rx, ry, i, j) {
                        self.fragment(Frag { x, y, c, z: rz, fixed: ToFixed::Trunc, zfixed: None })?;
                    }
                }
            }
            return Ok(());
        }
        let (start, k) = layout(&self.unpack, comps.len(), size, w);
        if f == DEPTH_COMPONENT || f == STENCIL_INDEX {
            // depth images: fragments of the raster color (initially white, the model follows no
            // the raster position's, GL.RASTER.POS) at the element's depth, through the tests; stencil
            // images: each element's integer value written to the stencil buffer under the
            // stencil write mask and the scissor (GL 1.1 3.6.4; GL.DRAW.DEPTHSTENCIL)
            hit(clause::GL_DRAW_DEPTHSTENCIL);
            for j in 0..h as usize {
                for i in 0..w as usize {
                    let at = (start + j * k + i) * size;
                    let Some(e) = data.get(at..at + size) else { return Err(Fail::Unmodeled(open::GL_DRAW_BUFFER)) };
                    let mut e = e.to_vec();
                    if self.unpack[0] != 0 {
                        e.reverse();
                    }
                    if f == STENCIL_INDEX {
                        // (MEASURED, golden opengl-32 draw-depthstencil-1: the element as a float,
                        // so 32-bit integers lose their low bits)
                        let v = self.index_map(true, self.shifted(f64::from(index_value(&e, t)? as f32)));
                        for (x, y) in self.image_pixels(rx, ry, i, j) {
                            self.draw_stencil(x, y, v);
                        }
                    } else {
                        let (z, zfixed) = depth_element(&e, t, self.depth_transfer);
                        let c = fog(self, self.raster_color);
                        for (x, y) in self.image_pixels_of(rx, ry, i, j, true) {
                            self.fragment(Frag { x, y, c, z, fixed: ToFixed::Trunc, zfixed: Some(FixedZ { bits: 32, value: zfixed }) })?;
                        }
                    }
                }
            }
            return Ok(());
        }
        for j in 0..h as usize {
            for i in 0..w as usize {
                let mut c = [0.0, 0.0, 0.0, 1.0];
                for (n, which) in comps.iter().enumerate() {
                    let at = (start + j * k + i * comps.len() + n) * size;
                    if at + size > data.len() {
                        return Err(Fail::Unmodeled(open::GL_DRAW_BUFFER));
                    }
                    let mut e = data[at..at + size].to_vec();
                    if self.unpack[0] != 0 {
                        e.reverse();
                    }
                    let v = to_float(&e, t);
                    // (MEASURED, golden opengl-32 draw-6, draw-unpack-11: a NaN component draws
                    // as 0, an infinite one clamps as any other: GL.DRAW.NAN)
                    let v = if v.is_finite() {
                        v
                    } else {
                        hit(clause::GL_DRAW_NAN);
                        if v.is_nan() { 0.0 } else { v.clamp(-1.0, 1.0) }
                    };
                    match which {
                        4 => {
                            hit(clause::GL_DRAW_LUMINANCE);
                            c[0] = v;
                            c[1] = v;
                            c[2] = v;
                        }
                        7 if self.pf.ty != 0 => {
                            // an index in color index mode: the element's integer value as it is
                            // (MEASURED, golden opengl-32 ci-*: unsigned bytes)
                            hit(clause::GL_COLORINDEX);
                            c[0] = self.index_map(false, self.shifted(index_value(&e, t)?));
                        }
                        7 => {
                            // an index in RGBA mode through GL_PIXEL_MAP_I_TO_R.. A, whose
                            // initial tables hold one zero each
                            hit(clause::GL_DRAW_INDEX);
                            c = [0.0; 4];
                        }
                        _ => c[*which] = v,
                    }
                }
                let c = if self.pf.ty != 0 { c } else { self.transfer(c)? };
                let c = fog(self, c.map(|v| v as f32));
                if !zoomed {
                    self.fragment(Frag { x: ox + i as i64, y: oy + j as i64, c, z: rz, fixed: ToFixed::Trunc, zfixed: None })?;
                    continue;
                }
                // (MEASURED, golden opengl-32 pixel-zoom: image pixel (i, j) covers the window
                // pixels whose centers are in [zx i, zx (i + 1)) across and (zy j, zy (j + 1)]
                // up from the raster position: GL.DRAW.ZOOM)
                hit(clause::GL_DRAW_ZOOM);
                let (zx, zy) = (f64::from(self.zoom.0), f64::from(self.zoom.1));
                // (MEASURED, other-calls-0: from the raster position as it is, a negative zoom
                // going left or down)
                let (xa, xb) = (rx + zx * i as f64, rx + zx * (i + 1) as f64);
                let (ya, yb) = (ry + zy * j as f64, ry + zy * (j + 1) as f64);
                let (x0, x1) = (xa.min(xb), xa.max(xb));
                let (y0, y1) = (ya.min(yb), ya.max(yb));
                for y in (y0.floor() as i64).max(0)..=(y1.ceil() as i64).min(H as i64 - 1) {
                    for x in (x0.floor() as i64).max(0)..=(x1.ceil() as i64).min(W as i64 - 1) {
                        let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
                        if cx >= x0 && cx < x1 && cy > y0 && cy <= y1 {
                            self.fragment(Frag { x, y, c, z: rz, fixed: ToFixed::Trunc, zfixed: None })?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// the window pixels image pixel (i, j) covers from the raster position (rx, ry): unzoomed
    /// the one at the position rounded plus (i, j), zoomed those GL.DRAW.ZOOM gives (MEASURED,
    /// golden opengl-32 pixel-zoom for colors; other-calls-6: depth images the same)
    fn image_pixels(&self, rx: f64, ry: f64, i: usize, j: usize) -> Vec<(i64, i64)> {
        self.image_pixels_of(rx, ry, i, j, false)
    }

    /// image_pixels for a depth image, whose zoomed columns take centers in (zx i, zx (i + 1)]
    /// (MEASURED, golden opengl-32 other-calls-6: zoomed 2 by 3 from 1.5, columns 2..9)
    fn image_pixels_of(&self, rx: f64, ry: f64, i: usize, j: usize, depth: bool) -> Vec<(i64, i64)> {
        if self.zoom == (1.0, 1.0) {
            return vec![((rx + 0.5).floor() as i64 + i as i64, (ry + 0.5).floor() as i64 + j as i64)];
        }
        hit(clause::GL_DRAW_ZOOM);
        let (zx, zy) = (f64::from(self.zoom.0), f64::from(self.zoom.1));
        let (xa, xb) = (rx + zx * i as f64, rx + zx * (i + 1) as f64);
        let (ya, yb) = (ry + zy * j as f64, ry + zy * (j + 1) as f64);
        let (x0, x1) = (xa.min(xb), xa.max(xb));
        let (y0, y1) = (ya.min(yb), ya.max(yb));
        let mut out = Vec::new();
        for y in (y0.floor() as i64).max(0)..=(y1.ceil() as i64).min(H as i64 - 1) {
            for x in (x0.floor() as i64).max(0)..=(x1.ceil() as i64).min(W as i64 - 1) {
                let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
                let in_x = if depth { cx > x0 && cx <= x1 } else { cx >= x0 && cx < x1 };
                if in_x && cy > y0 && cy <= y1 {
                    out.push((x, y));
                }
            }
        }
        out
    }

    /// the DIB's rows, bottom first, as the framebuffer's pixels in the format's channels
    pub fn dib(&self) -> Res<Vec<String>> {
        hit(clause::GL_DIB);
        if self.bpp < 16 {
            // (MEASURED, golden opengl-32 edge-dib-8: the pixel value itself is the DIB's index)
            hit(clause::GL_DIB_INDEX);
        }
        let mut rows = Vec::new();
        for y in 0..H {
            let mut b = Vec::new();
            let mut acc = 0u32;
            for x in 0..W {
                let p = self.fb[y * W + x];
                let mut v = 0u32;
                for k in 0..4 {
                    v |= p[k] << self.pf.rgba[k].1;
                }
                if self.bpp >= 8 {
                    b.extend_from_slice(&v.to_le_bytes()[..self.bpp as usize / 8]);
                } else {
                    // (4 bits a pixel, the left one in the high nibble; MEASURED, golden
                    // opengl-32 dither-fill-6, 7: an RGBA color but black is the bright half of the
                    // 16-color palette, index 8 more)
                    if self.pf.ty == 0 && v != 0 {
                        hit(clause::GL_DIB_RGB4);
                        v |= 8;
                    }
                    acc = acc << 4 | (v & 15);
                    if x % 2 == 1 {
                        b.push(acc as u8);
                        acc = 0;
                    }
                }
            }
            rows.push(hex(&b));
        }
        Ok(rows)
    }
}
