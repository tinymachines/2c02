//! The $2002 read race, held to the table measured on rung 0 with the
//! console's access shape (`v2c02-sim --example race-shape-probe`,
//! lib.rs `race`): at every half-step of the two dots before the set,
//! a timed read misses, or suppresses the set, exactly where the chip
//! did; on the set's dot it consumes. MUTATE=1 moves the read one
//! half-step and must go red.

use nes_bus::{ACTIVE_ROWS, DOTS_PER_LINE};
use v2c02_fast::{Fast, Position};

/// NMI enabled, rendering off, stepped until the dot `before` the set
/// (1 = the dot before, 2 = two before) is the last one stepped.
fn at(before: usize) -> Fast {
    let mut f = Fast::new(|_| 0);
    f.write(0, 0x80);
    let set = Position { line: ACTIVE_ROWS + 1, dot: 1 };
    let target = if before <= set.dot {
        Position { line: set.line, dot: set.dot - before }
    } else {
        Position { line: set.line - 1, dot: DOTS_PER_LINE - (before - set.dot) }
    };
    // The next dot to step, after stepping `target`, is one past it.
    let next = if target.dot + 1 < DOTS_PER_LINE { Position { line: target.line, dot: target.dot + 1 } } else { Position { line: target.line + 1, dot: 0 } };
    while f.position() != next {
        f.step_dot();
    }
    assert_eq!(f.nmi_asserted(), before == 0, "the set's dot is stepped only when `before` is 0");
    f
}

fn mutate() -> u8 {
    std::env::var_os("MUTATE").map_or(0, |_| 1)
}

#[test]
fn a_read_starting_eight_or_more_half_steps_before_the_set_misses_it() {
    // Two dots before: every half-step; one dot before: its first.
    let mut cases: Vec<(usize, u8)> = (0..8u8).map(|h| (2, h)).collect();
    cases.push((1, 0));
    for (before, into) in cases {
        let mut f = at(before);
        let byte = f.read_timed(2, into + if before == 1 { mutate() } else { 0 });
        assert_eq!(byte & 0x80, 0, "{before} dots before, half-step {into}: read as clear");
        while f.position() != (Position { line: ACTIVE_ROWS + 1, dot: 2 }) {
            f.step_dot();
        }
        assert!(f.nmi_asserted(), "{before} dots before, half-step {into}: the set went ahead, /INT falls");
        assert_eq!(f.read(2) & 0x80, 0x80, "{before} dots before, half-step {into}: the flag is there for a later read");
    }
}

#[test]
fn a_read_starting_inside_the_dot_before_the_set_suppresses_it() {
    for into in 1..8u8 {
        let mut f = at(1);
        let byte = f.read_timed(2, into);
        assert_eq!(byte & 0x80, 0, "half-step {into}: read as clear");
        while f.position() != (Position { line: ACTIVE_ROWS + 1, dot: 2 }) {
            f.step_dot();
        }
        assert!(!f.nmi_asserted(), "half-step {into}: the set was suppressed, /INT never falls");
        assert_eq!(f.read(2) & 0x80, 0, "half-step {into}: the flag never set");
    }
}

#[test]
fn a_read_on_the_sets_dot_consumes_it_after_int_fell() {
    for into in 0..8u8 {
        let mut f = at(0);
        assert!(f.nmi_asserted(), "the set's dot was stepped: /INT is low");
        let byte = f.read_timed(2, into);
        assert_eq!(byte & 0x80, 0x80, "half-step {into}: read as set");
        assert!(!f.nmi_asserted(), "half-step {into}: consumed, /INT back up");
    }
}
