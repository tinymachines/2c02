//! The P1 DAC gate: the chip's own leg drives against the transcribed
//! level table, sample for sample. One half-step is one grid sample
//! (the master half-clock is 12 x f_sc), so the prediction is
//! ntsc-source-nes's own signal function at the chip's measured
//! subcarrier alignment, and the observation is which DAC leg
//! conducts. Two constants are fitted once and pinned: the subcarrier
//! phase offset (from the burst legs, which ARE wave 8) and the DAC
//! pipeline delay behind the pal bus.
//!
//! SKIPS by name without the die data. MUTATE=1 perturbs the pinned
//! pipeline delay by one half-step; the comparison must go red.

use ntsc_source_nes::{wave_high, Levels};
use v2c02_dots::{capture, leg_voltage, standard_world, Taps};

fn mutate() -> bool {
    std::env::var("MUTATE").map(|v| v == "1").unwrap_or(false)
}

/// Fitted 2026-09-02 and pinned; the test re-fits and holds the fit to
/// these, so a drift shows up as a failed pin rather than a silently
/// re-aligned comparison.
const PINNED_PHASE: usize = 11;
const PINNED_DELAY: usize = 12;
const FIT_IS_PINNED: bool = true;

#[test]
fn the_dac_legs_speak_the_transcribed_table() {
    if !v2c02_netlist::available() {
        eprintln!("SKIP: extern/visual2c02 not fetched");
        return;
    }
    let mut h = standard_world();
    let cap = capture(&mut h, 4);
    let taps = Taps::new(&h);
    let _ = taps;

    // Structure: at every half-step exactly one level leg conducts,
    // and the emphasis attenuator never does (the program sets no
    // emphasis bits).
    for (i, (_, _, mask, emph)) in cap.trace.iter().enumerate() {
        assert_eq!(mask.count_ones(), 1, "step {i}: leg mask {mask:012b}");
        assert!(!emph, "step {i}: emphasis active with no emphasis bits set");
    }

    // The chip's subcarrier alignment, measured from the burst legs:
    // burst high exactly when wave 8 is high.
    let burst_steps: Vec<(usize, bool)> = cap
        .trace
        .iter()
        .enumerate()
        .filter(|(_, (.., mask, _))| mask & 0b1100 != 0)
        .map(|(i, (.., mask, _))| (i, mask & 0b1000 != 0))
        .collect();
    assert!(burst_steps.len() > 200, "not enough burst samples: {}", burst_steps.len());
    let phase = (0..12)
        .find(|&d| {
            burst_steps
                .iter()
                .all(|&(i, high)| wave_high(8, ((i + d) % 12) as u8) == high)
        })
        .expect("no phase offset makes the burst wave 8");

    // The active-region comparison at each candidate pipeline delay:
    // predicted level (the signal function on the captured dot at the
    // chip's phase) against the conducting leg's table voltage.
    let lv = Levels::transcribed();
    let mismatches = |delay: usize| -> usize {
        let mut bad = 0;
        for (i, (hp, vp, mask, _)) in cap.trace.iter().enumerate() {
            let (hp, vp) = (*hp as usize, *vp as usize);
            if !(8..248).contains(&hp) || vp >= 4 || i < delay {
                continue;
            }
            // The leg at step i reflects the dot the pal bus carried
            // `delay` half-steps earlier.
            let (ehp, evp) = (cap.trace[i - delay].0 as usize, cap.trace[i - delay].1 as usize);
            let (colour, _) = cap.dots.at(evp, ehp + 1);
            let want = lv.signal(colour, 0, (((i - delay) + phase) % 12) as u8);
            let leg = mask.trailing_zeros() as usize;
            if (leg_voltage(leg) - want).abs() > 1e-6 {
                bad += 1;
            }
        }
        bad
    };
    let (best_delay, _) = (0..24).map(|d| (d, mismatches(d))).min_by_key(|&(_, b)| b).unwrap();
    let delay = if mutate() { best_delay + 1 } else { best_delay };
    let bad = mismatches(delay);
    let total = cap
        .trace
        .iter()
        .filter(|(hp, vp, ..)| (8..248).contains(&(*hp as usize)) && (*vp as usize) < 4)
        .count();
    eprintln!(
        "phase offset {phase}, pipeline delay {best_delay}: {bad} mismatches of {total} active samples"
    );
    assert_eq!(bad, 0, "the DAC disagrees with the table at delay {delay}");
    if FIT_IS_PINNED {
        assert_eq!(phase, PINNED_PHASE, "subcarrier alignment moved");
        assert_eq!(best_delay, PINNED_DELAY, "pipeline delay moved");
    } else {
        eprintln!("PIN ME: phase {phase}, delay {best_delay}");
    }
}
