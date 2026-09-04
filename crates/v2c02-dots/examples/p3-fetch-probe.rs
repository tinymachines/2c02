//! The measurement P3's datapath is authored from: what the switch-level
//! chip does at each DOT of a frame in the standard world. Per dot: the
//! VRAM address latched while ALE is high, whether /RD fell, and for
//! each named control line an 8-bit mask of the half-steps (within the
//! dot) on which it was high, so a one-half-step pulse is not missed by
//! sampling once. Writes the full frame as CSV to the path given as the
//! first argument and prints a summary: the fetch schedule of one
//! visible line and the (vpos, hpos) at which each control is active.
//! Measurement only; nothing here is a claim.

use std::collections::BTreeMap;
use std::io::Write as _;

use v2c02_dots::{standard_world, Taps};

const CONTROLS: &[&str] = &[
    "bkg_enable_out",
    "in_vblank",
    "set_vbl_flag",
    "vbl_clear_flags",
    "copy_vramaddr_hscroll",
    "copy_vramaddr_vscroll",
    "load_vramaddr_v_hscroll_next",
    "load_vramaddr_v_vscroll_next",
    "vramaddr_v_hpos_eq_31_and_rendering",
    "fine_y_eq_7_and_rendering",
    "vramaddr_v_vpos_29_to_30_transition_and_rendering",
    "clr_vramaddr_vtile",
    "addr_inc",
    "spr_eval_copy_sprite",
    "inc_spr_ptr",
    "sprite_in_range",
    "copy_sprite_to_sec_oam",
    "clear_spr_ptr",
    "spr_addr_load_next_value",
    "spr_addr_clear_low_bump_high_setup",
    "spr_clip_out",
    "end_of_oam_or_sec_oam_overflow",
    "spr0_load_next",
    "pclk0",
    "pclk1",
];

#[derive(Default, Clone)]
struct Dot {
    addr: Option<u16>,
    rd: bool,
    ctl: Vec<u8>,
    steps: u8,
}

fn main() {
    let out_path = std::env::args().nth(1).expect("csv path");
    let mut h = standard_world();
    let taps = Taps::new(&h);
    let nl = h.ppu.engine.netlist().clone();
    let ids: Vec<_> = CONTROLS
        .iter()
        .map(|s| nl.node(s).unwrap_or_else(|| panic!("node {s}")))
        .collect();

    // Align to the frame boundary, the same way capture() does.
    while !(taps.bus(&h, &taps.vpos) == 261 && taps.bus(&h, &taps.hpos) == 340) {
        h.half_step();
    }

    let mut dots: BTreeMap<(u16, u16), Dot> = BTreeMap::new();
    let mut cur = Dot { ctl: vec![0; CONTROLS.len()], ..Default::default() };
    let mut key: Option<(u16, u16)> = None;
    let mut done = false;
    while !done {
        h.half_step();
        let hp = taps.bus(&h, &taps.hpos) as u16;
        let vp = taps.bus(&h, &taps.vpos) as u16;
        let k = (vp, hp);
        if key != Some(k) {
            if let Some(prev) = key {
                // A full frame is every dot from (0,0) through (261,340).
                if prev == (261, 340) && k == (0, 0) && dots.len() > 1000 {
                    done = true;
                }
                dots.insert(prev, cur.clone());
            }
            key = Some(k);
            cur = Dot { ctl: vec![0; CONTROLS.len()], ..Default::default() };
        }
        let p = h.pins();
        if p.ale {
            cur.addr = Some(((p.a_hi as u16) << 8) | p.ad as u16);
        }
        if !p.rd_n {
            cur.rd = true;
        }
        let s = cur.steps.min(7);
        for (i, &id) in ids.iter().enumerate() {
            if h.ppu.engine.is_high(id) {
                cur.ctl[i] |= 1 << s;
            }
        }
        cur.steps += 1;
    }

    // CSV.
    let mut f = std::fs::File::create(&out_path).unwrap();
    write!(f, "vpos,hpos,steps,addr,rd").unwrap();
    for c in CONTROLS {
        write!(f, ",{c}").unwrap();
    }
    writeln!(f).unwrap();
    for ((vp, hp), d) in &dots {
        write!(
            f,
            "{vp},{hp},{},{},{}",
            d.steps,
            d.addr.map(|a| format!("{a:04x}")).unwrap_or_else(|| "-".into()),
            d.rd as u8
        )
        .unwrap();
        for m in &d.ctl {
            write!(f, ",{m:02x}").unwrap();
        }
        writeln!(f).unwrap();
    }
    println!("{} dots written to {out_path}", dots.len());

    // Summary 1: the fetch schedule of visible line 1 (a line with the
    // pipeline warm), dots 0..=16 and 248..=340.
    println!("\n== line 1 fetches (hpos: addr rd) ==");
    for hp in (0u16..=16).chain(248..=340) {
        if let Some(d) = dots.get(&(1, hp)) {
            let a = d.addr.map(|a| format!("{a:04x}")).unwrap_or_else(|| "----".into());
            print!("{hp}:{a}{} ", if d.rd { "r" } else { " " });
            if hp % 8 == 7 || hp == 16 {
                println!();
            }
        }
    }
    println!();

    // Summary 2: where each control is active, as (vpos, hpos) runs.
    println!("== control activity (dot positions; runs collapsed) ==");
    for (i, c) in CONTROLS.iter().enumerate() {
        if c.starts_with("pclk") {
            continue;
        }
        let mut active: Vec<(u16, u16, u8)> = dots
            .iter()
            .filter(|(_, d)| d.ctl[i] != 0)
            .map(|(&(vp, hp), d)| (vp, hp, d.ctl[i]))
            .collect();
        active.sort();
        let n = active.len();
        let mut runs = Vec::new();
        let mut it = active.iter().peekable();
        while let Some(&(vp, hp, m)) = it.next() {
            let (mut vp2, mut hp2) = (vp, hp);
            while let Some(&&(nvp, nhp, nm)) = it.peek() {
                let adj = (nvp == vp2 && nhp == hp2 + 1) || (nvp == vp2 + 1 && nhp == 0 && hp2 == 340);
                if adj && nm == m {
                    vp2 = nvp;
                    hp2 = nhp;
                    it.next();
                } else {
                    break;
                }
            }
            runs.push(format!("({vp},{hp})..({vp2},{hp2})@{m:02x}"));
        }
        let shown: Vec<_> = runs.iter().take(12).cloned().collect();
        println!(
            "{c}: {n} dots, {} runs: {}{}",
            runs.len(),
            shown.join(" "),
            if runs.len() > 12 { " ..." } else { "" }
        );
    }

    // Summary 3: the per-line pattern of the two vramaddr controls on
    // visible lines, which the datapath's increment schedule is from.
    println!("\n== per-line hpos of vramaddr controls, line 1 ==");
    for (i, c) in CONTROLS.iter().enumerate() {
        if !c.contains("vramaddr") && !c.contains("fine_y") {
            continue;
        }
        let hps: Vec<String> = (0u16..=340)
            .filter_map(|hp| dots.get(&(1, hp)).filter(|d| d.ctl[i] != 0).map(|d| format!("{hp}@{:02x}", d.ctl[i])))
            .collect();
        println!("{c}: {}", hps.join(" "));
    }
}
