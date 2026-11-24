//! The libmpv2 7.0.1 safe wrapper exposes only OpenGL. This narrow bridge uses
//! the installed mpv/render.h ABI for the software backend on its owning thread.
use crate::failure;
use gpui_kit::Result;
use libmpv2::Mpv;
use std::{ffi::c_void, marker::PhantomData, ptr, thread};

pub const WIDTH: usize = 1280;
pub const HEIGHT: usize = 720;
const BYTES: usize = WIDTH * HEIGHT * 4;

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

pub struct Software<'a> {
    context: *mut c_void,
    wake: Box<thread::Thread>,
    buffer: Vec<Block>,
    _mpv: PhantomData<&'a Mpv>,
}

unsafe extern "C" fn wake(data: *mut c_void) {
    // SAFETY: data points to the boxed Thread kept alive until context_free returns.
    // unpark does not call mpv, block on the render thread, or unwind.
    unsafe { (&*data.cast::<thread::Thread>()).unpark() };
}

pub fn player(headless: bool) -> Result<Mpv> {
    Ok(Mpv::with_initializer(|init| {
        init.set_option("config", "no")?;
        init.set_option("terminal", "no")?;
        init.set_option("hwdec", "auto-safe")?;
        init.set_option("vo", "libmpv")?;
        init.set_option("keep-open", "yes")?;
        if headless {
            init.set_option("ao", "null")?;
        }
        Ok(())
    })?)
}

impl<'a> Software<'a> {
    pub fn new(mpv: &'a Mpv) -> Result<Self> {
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
            return Err(failure(format!("software render context: {code}")).into());
        }
        let mut renderer = Self {
            context,
            wake: Box::new(thread::current()),
            buffer: vec![Block([0; 64]); BYTES / 64],
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

    pub fn render_if_ready(&mut self) -> Result<bool> {
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
            return Err(failure(format!("render frame info: {code}")).into());
        }
        let mut size = [WIDTH as i32, HEIGHT as i32];
        let mut stride = WIDTH * 4;
        let mut params = [
            Param::new(17, size.as_mut_ptr().cast()),
            Param::new(18, c"rgba".as_ptr().cast_mut().cast()),
            Param::new(19, (&mut stride as *mut usize).cast()),
            Param::new(20, self.buffer.as_mut_ptr().cast()),
            Param::end(),
        ];
        // SAFETY: size and stride match the writable 64-byte-aligned allocation;
        // the exclusive borrow prevents concurrent buffer access. mpv retains
        // none of these render parameters beyond the synchronous call.
        let code = unsafe { mpv_render_context_render(self.context, params.as_mut_ptr()) };
        if code < 0 {
            return Err(failure(format!("software render: {code}")).into());
        }
        // A paused exact seek can present its new picture as a redraw. Include
        // those renders; exclude empty notifications. The caller also checks
        // post-command render time, playback position and seek completion.
        Ok(info.flags & 1 != 0)
    }

    pub fn rgba(&self) -> &[u8] {
        // SAFETY: Block contains exactly 64 initialized bytes, with no padding;
        // the allocation covers BYTES bytes and is borrowed for the slice lifetime.
        unsafe { std::slice::from_raw_parts(self.buffer.as_ptr().cast(), BYTES) }
    }
}

impl Drop for Software<'_> {
    fn drop(&mut self) {
        // SAFETY: unique context, still-live Mpv and callback box; freeing the
        // context synchronizes with outstanding callbacks before fields drop.
        unsafe { mpv_render_context_free(self.context) };
    }
}
