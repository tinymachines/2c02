//! Saved states: the stepper saved at any dot, restored through its bytes
//! onto a fresh stepper on a copy of the world, runs on exactly as the one
//! that never stopped. Rendering is on with sprites (sprite 0 over the
//! background, more than eight on some lines, 8x16 on the second frame),
//! the scroll is rewritten mid-frame, $2002 is read across vblank and
//! $2007 read and written in it. Each restored stepper runs a frame beside
//! the original: the NMI line and every register read at every dot, every
//! picture in full, and the whole state again at the end.
//!
//! SKIPs by name without the table. MUTATE_STATE=1 loses the sprite
//! units on a load and must go red.

use std::cell::RefCell;
use std::rc::Rc;

use nes_bus::{DOTS_PER_LINE, LINES};
use v2c02_fast::{Fast, VramBus};

fn noise(n: usize, seed: u32) -> Vec<u8> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

/// The PPU's world: pattern tables and nametables in one 16 KiB space
/// that takes writes, copyable for a restored stepper.
#[derive(Clone)]
struct World(Rc<RefCell<Vec<u8>>>);

impl VramBus for World {
    fn read(&mut self, a: u16) -> u8 {
        self.0.borrow()[(a & 0x3fff) as usize]
    }
    fn write(&mut self, a: u16, v: u8) {
        self.0.borrow_mut()[(a & 0x3fff) as usize] = v;
    }
}

enum Act {
    W(u8, u8),
    R(u8),
}

/// What the CPU does at (frame, line, dot).
fn acts(frame: usize, line: usize, dot: usize) -> Vec<Act> {
    let mut v = Vec::new();
    // Mid-frame scroll: a split at line 100 each frame.
    if line == 100 && dot == 256 {
        v.extend([Act::W(5, (frame * 13) as u8), Act::W(5, 7)]);
    }
    // $2002 read across the vblank set, a dot a frame apart.
    if line == 241 && dot == frame % 3 {
        v.push(Act::R(2));
    }
    if line == 245 && dot == 10 {
        // In vblank: 8x16 on the second frame, $2007 written and read.
        v.extend([Act::W(0, if frame == 1 { 0xa0 } else { 0x80 }), Act::W(6, 0x23), Act::W(6, (0x40 + frame) as u8), Act::W(7, 0x55), Act::R(7), Act::R(7)]);
        v.extend([Act::W(5, 0), Act::W(5, 0), Act::W(6, 0x20), Act::W(6, 0x00)]);
    }
    v
}

fn run_acts(p: &mut Fast, acts: &[Act]) -> Vec<u8> {
    acts.iter()
        .filter_map(|a| match *a {
            Act::W(r, v) => {
                p.write(r, v);
                None
            }
            Act::R(r) => Some(p.read(r)),
        })
        .collect()
}

fn bytes(p: &Fast) -> Vec<u8> {
    postcard::to_allocvec(p).expect("a state encodes")
}

fn restored(world: &World, saved: &[u8]) -> (Fast, World) {
    let copy = World(Rc::new(RefCell::new(world.0.borrow().clone())));
    let mut p = Fast::on_bus(Box::new(copy.clone()));
    p.load_state(postcard::from_bytes(saved).expect("a state decodes")).expect("a state restores");
    (p, copy)
}

#[test]
fn a_stepper_restored_at_any_dot_runs_on_as_if_it_had_never_stopped() {
    if !v2c02_fast::table_available() {
        eprintln!("SKIP: no table (extern/visual2c02 not fetched at build time)");
        return;
    }
    let world = World(Rc::new(RefCell::new(noise(0x4000, 0xc0ffee))));
    // Nametable 0 as tiles, attributes varied.
    let mut a = Fast::on_bus(Box::new(world.clone()));
    // OAM: sprite 0 at (60, 40) over the background, and twelve sprites
    // on lines 80..88 for the overflow; the rest random.
    let mut oam = noise(256, 7);
    oam[..4].copy_from_slice(&[39, 0x11, 0x00, 60]);
    for i in 1..13 {
        oam[4 * i..4 * i + 4].copy_from_slice(&[79, i as u8, (i % 4) as u8 | if i % 2 == 0 { 0x40 } else { 0 }, (20 * i) as u8]);
    }
    a.write(3, 0);
    for &b in &oam {
        a.write(4, b);
    }
    a.write(6, 0x3f);
    a.write(6, 0x00);
    for b in noise(32, 9) {
        a.write(7, b & 0x3f);
    }
    a.write(0, 0x80);
    a.write(1, 0x1e);
    a.write(5, 0);
    a.write(5, 0);
    a.write(6, 0x20);
    a.write(6, 0x00);

    const FRAMES: usize = 3;
    const EVERY: usize = 997;
    const RUN: usize = LINES * DOTS_PER_LINE;
    let mut live: Vec<(usize, Fast, World)> = Vec::new();
    let (mut splits, mut hits) = (0, 0);
    for step in 0..FRAMES * LINES * DOTS_PER_LINE {
        let frame = step / (LINES * DOTS_PER_LINE);
        if frame < FRAMES - 1 && step % EVERY == 0 {
            let saved = bytes(&a);
            let (b, w) = restored(&world, &saved);
            assert_eq!(bytes(&b), saved, "step {step}: a restored stepper saves the same state");
            live.push((step, b, w));
            splits += 1;
        }
        let pos = a.position();
        let todo = acts(frame, pos.line, pos.dot);
        let want_reads = run_acts(&mut a, &todo);
        let want_frame = a.step_dot();
        if let Some(f) = &want_frame {
            hits += a.spr0_hit.is_some() as usize;
            let _ = f;
        }
        for (at, b, _) in live.iter_mut() {
            assert_eq!(b.position(), pos, "split at {at}, step {step}");
            assert_eq!(run_acts(b, &todo), want_reads, "split at {at}, step {step}: the register reads");
            let got = b.step_dot();
            assert_eq!(b.nmi_asserted(), a.nmi_asserted(), "split at {at}, step {step}: NMI");
            match (&got, &want_frame) {
                (None, None) => {}
                (Some(g), Some(w)) => {
                    assert!(g.colour == w.colour && g.emphasis == w.emphasis, "split at {at}: the picture at step {step}");
                }
                _ => panic!("split at {at}: a picture came out at step {step} on one side only"),
            }
        }
        let now = live.iter().any(|(at, ..)| step + 1 - at >= RUN).then(|| bytes(&a));
        live.retain(|(at, b, w)| {
            if step + 1 - at < RUN {
                return true;
            }
            assert!(Some(bytes(b)) == now, "split at {at}: the whole state a frame on");
            assert!(*w.0.borrow() == *world.0.borrow(), "split at {at}: the world as written");
            false
        });
    }
    assert!(live.is_empty());
    assert!(splits > 150, "{splits} splits");
    assert_eq!(hits, FRAMES, "sprite 0 hit once a frame");
}
