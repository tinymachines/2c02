// The per-dot event bits, one definition shared by the recorder
// (build.rs) and the stepper (lib.rs) by `include!`, so the two cannot
// disagree about a bit's meaning. Every bit is MEASURED: a named node
// high on any half-step of the dot, or a CHR-bus fetch classified by
// the address the chip latched at ALE. See docs/p3-plan.md.

/// A nametable fetch: ALE latched a $2xxx address outside the attribute
/// rows.
pub const FETCH_NT: u16 = 1 << 0;
/// An attribute fetch: a $2xxx address with bits 6..9 all set.
pub const FETCH_AT: u16 = 1 << 1;
/// A pattern fetch below $2000, low plane (bit 3 clear).
pub const FETCH_PT_LO: u16 = 1 << 2;
/// A pattern fetch below $2000, high plane (bit 3 set).
pub const FETCH_PT_HI: u16 = 1 << 3;
/// The same three kinds inside the sprite window (the 64 dots from the
/// `clear_spr_ptr` pulse past dot 200): garbage nametable fetches, and
/// the sprite pattern planes.
pub const SPR_GARBAGE: u16 = 1 << 4;
pub const SPR_PT_LO: u16 = 1 << 5;
pub const SPR_PT_HI: u16 = 1 << 6;
/// `load_vramaddr_v_hscroll_next`: the coarse X increment.
pub const INC_X: u16 = 1 << 7;
/// `load_vramaddr_v_vscroll_next`: the Y increment.
pub const INC_Y: u16 = 1 << 8;
/// `copy_vramaddr_hscroll`: t's horizontal bits into v.
pub const COPY_X: u16 = 1 << 9;
/// `copy_vramaddr_vscroll`: t's vertical bits into v.
pub const COPY_Y: u16 = 1 << 10;
/// `set_vbl_flag`.
pub const SET_VBL: u16 = 1 << 11;
/// `vbl_clear_flags`.
pub const CLR_FLAGS: u16 = 1 << 12;
/// /RD fell during the dot: the byte of the preceding fetch lands here.
pub const RD: u16 = 1 << 13;
