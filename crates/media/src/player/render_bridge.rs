//! libmpv2 7.0.1 exposes only OpenGL rendering. This is the spike's narrow
//! mpv/render.h software ABI, kept on its creating thread and borrowed from Mpv.
use super::backend;
use libmpv2::Mpv;
use std::{ffi::c_void, marker::PhantomData, ptr, thread};
use vaio_core::ports::player::PlayerError;

#[repr(C)]
struct Param {
    kind: u32,
    data: *mut c_void,
}

#[repr(C)]
#[derive(Default)]
struct FrameInfo {
    flags: u64,
    target_time: i64,
}

impl Param {
    fn new(kind: u32, data: *mut c_void) -> Self {
        Self { kind, data }
    }
    fn end() -> Self {
        Self::new(0, ptr::null_mut())
    }
}

#[link(name = "mpv")]
unsafe extern "C" {
    fn mpv_render_context_create(
        out: *mut *mut c_void,
        mpv: *mut c_void,
        params: *mut Param,
    ) -> i32;
    fn mpv_render_context_set_update_callback(
        ctx: *mut c_void,
        callback: Option<unsafe extern "C" fn(*mut c_void)>,
        data: *mut c_void,
    );
    fn mpv_render_context_update(ctx: *mut c_void) -> u64;
    fn mpv_render_context_get_info(ctx: *mut c_void, param: Param) -> i32;
    fn mpv_render_context_render(ctx: *mut c_void, params: *mut Param) -> i32;
    fn mpv_render_context_free(ctx: *mut c_void);
}

#[repr(C, align(64))]
#[derive(Clone)]
struct Block([u8; 64]);

pub(super) struct Software<'a> {
    context: *mut c_void,
    wake: Box<thread::Thread>,
    buffer: Vec<Block>,
    width: u32,
    height: u32,
    bytes: usize,
    _mpv: PhantomData<&'a Mpv>,
}

unsafe extern "C" fn wake(data: *mut c_void) {
    // SAFETY: data points to the boxed Thread kept alive until context_free returns.
    // unpark does not call mpv, block on the render thread, or unwind.
    unsafe { (&*data.cast::<thread::Thread>()).unpark() };
}

pub(super) fn validate_size(width: u32, height: u32) -> Result<usize, PlayerError> {
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err(backend(
            "surface dimensions must be positive signed 32-bit integers",
        ));
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .filter(|n| *n <= isize::MAX as usize - 63)
        .ok_or_else(|| backend("surface byte size overflows"))
}

impl<'a> Software<'a> {
    pub fn new(mpv: &'a Mpv, width: u32, height: u32) -> Result<Self, PlayerError> {
        let bytes = validate_size(width, height)?;
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(bytes.div_ceil(64))
            .map_err(backend)?;
        buffer.resize(bytes.div_ceil(64), Block([0; 64]));
        let mut context = ptr::null_mut();
        let mut params = [
            Param::new(1, c"sw".as_ptr().cast_mut().cast()),
            Param::end(),
        ];
        // SAFETY: handle is owned by the borrowed Mpv; the terminated parameter
        // array and C string stay alive through this synchronous native call.
        let code = unsafe {
            mpv_render_context_create(&mut context, mpv.ctx.as_ptr().cast(), params.as_mut_ptr())
        };
        if code < 0 || context.is_null() {
            return Err(backend(format!("software render context: {code}")));
        }
        let mut renderer = Self {
            context,
            wake: Box::new(thread::current()),
            buffer,
            width,
            height,
            bytes,
            _mpv: PhantomData,
        };
        // SAFETY: boxed callback data has a stable address and outlives the
        // context. The callback only wakes this thread; it never accesses mpv.
        unsafe {
            mpv_render_context_set_update_callback(
                context,
                Some(wake),
                (&mut *renderer.wake as *mut thread::Thread).cast(),
            )
        };
        Ok(renderer)
    }

    pub fn render_if_ready(&mut self) -> Result<bool, PlayerError> {
        // SAFETY: all render calls and destruction occur on the creating thread;
        // Software is not Send/Sync, and the Mpv outlives this context.
        if unsafe { mpv_render_context_update(self.context) } & 1 == 0 {
            return Ok(false);
        }
        let mut info = FrameInfo::default();
        // SAFETY: NEXT_FRAME_INFO takes a writable mpv_render_frame_info with
        // this exact C layout; it is used only during this synchronous call.
        let code = unsafe {
            mpv_render_context_get_info(
                self.context,
                Param::new(11, (&mut info as *mut FrameInfo).cast()),
            )
        };
        if code < 0 {
            return Err(backend(format!("render frame info: {code}")));
        }
        let mut size = [self.width as i32, self.height as i32];
        let mut stride = self.width as usize * 4;
        let mut params = [
            Param::new(17, size.as_mut_ptr().cast()),
            Param::new(18, c"rgba".as_ptr().cast_mut().cast()),
            Param::new(19, (&mut stride as *mut usize).cast()),
            Param::new(20, self.buffer.as_mut_ptr().cast()),
            Param::end(),
        ];
        // SAFETY: checked dimensions and stride fit the 64-byte-aligned allocation.
        // Every row remains aligned to RGBA's required 4 bytes. The exclusive
        // borrow prevents concurrent access; mpv retains none of the parameters.
        // Positive stride starts at the top-left, as required by the core Frame.
        let code = unsafe { mpv_render_context_render(self.context, params.as_mut_ptr()) };
        if code < 0 {
            return Err(backend(format!("software render: {code}")));
        }
        // Paused exact seeks can present their new picture as a redraw.
        Ok(info.flags & 1 != 0)
    }

    pub fn rgba(&self) -> &[u8] {
        // SAFETY: Block contains exactly 64 initialized bytes, with no padding;
        // allocation covers bytes (rounded up), borrowed for this slice's lifetime.
        unsafe { std::slice::from_raw_parts(self.buffer.as_ptr().cast(), self.bytes) }
    }
}

impl Drop for Software<'_> {
    fn drop(&mut self) {
        // SAFETY: unique context, still-live Mpv and callback box; freeing the
        // context synchronizes with outstanding callbacks before fields drop.
        unsafe { mpv_render_context_free(self.context) };
    }
}
