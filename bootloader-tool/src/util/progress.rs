// Adapted from probe-rs-tools/src/bin/probe-rs/util/flash.rs of probe-rs v0.27.0

use std::cell::RefCell;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use probe_rs::flashing::{FlashLayout, FlashProgress, ProgressEvent};

pub fn flash_progress() -> FlashProgress<'static> {
    let multi_progress = MultiProgress::new();
    let progress_bars = RefCell::new(ProgressBars {
        erase: ProgressBarGroup::new("          Erasing"),
        fill: ProgressBarGroup::new("    Reading flash"),
        program: ProgressBarGroup::new("      Programming"),
    });

    FlashProgress::new(move |event| {
        let mut progress_bars = progress_bars.borrow_mut();

        match event {
            ProgressEvent::Initialized {
                chip_erase,
                phases,
                restore_unwritten,
            } => {
                // Build progress bars.
                if chip_erase {
                    progress_bars
                        .erase
                        .add(multi_progress.add(ProgressBar::new(0)));
                }

                if phases.len() > 1 {
                    progress_bars.erase.append_phase();
                    progress_bars.program.append_phase();
                    progress_bars.fill.append_phase();
                }

                let mut flash_layout = FlashLayout::default();
                for phase_layout in phases {
                    if restore_unwritten {
                        let fill_size = phase_layout.fills().iter().map(|s| s.size()).sum::<u64>();
                        progress_bars
                            .fill
                            .add(multi_progress.add(ProgressBar::new(fill_size)));
                    }

                    if !chip_erase {
                        let sector_size =
                            phase_layout.sectors().iter().map(|s| s.size()).sum::<u64>();
                        progress_bars
                            .erase
                            .add(multi_progress.add(ProgressBar::new(sector_size)));
                    }

                    progress_bars
                        .program
                        .add(multi_progress.add(ProgressBar::new(0)));

                    flash_layout.merge_from(phase_layout);
                }

                // TODO: progress bar for verifying?
            }
            ProgressEvent::StartedProgramming { length } => {
                progress_bars.program.mark_start_now();
                progress_bars.program.set_length(length);
            }
            ProgressEvent::StartedErasing => {
                progress_bars.erase.mark_start_now();
            }
            ProgressEvent::StartedFilling => {
                progress_bars.fill.mark_start_now();
            }
            ProgressEvent::PageProgrammed { size, .. } => {
                progress_bars.program.inc(size as u64);
            }
            ProgressEvent::SectorErased { size, .. } => progress_bars.erase.inc(size),
            ProgressEvent::PageFilled { size, .. } => progress_bars.fill.inc(size),
            ProgressEvent::FailedErasing => {
                progress_bars.erase.abandon();
                progress_bars.program.abandon();
            }
            ProgressEvent::FinishedErasing => progress_bars.erase.finish(),
            ProgressEvent::FailedProgramming => progress_bars.program.abandon(),
            ProgressEvent::FinishedProgramming => progress_bars.program.finish(),
            ProgressEvent::FailedFilling => progress_bars.fill.abandon(),
            ProgressEvent::FinishedFilling => progress_bars.fill.finish(),
            ProgressEvent::DiagnosticMessage { .. } => {}
        }
    })
}

struct ProgressBars {
    erase: ProgressBarGroup,
    fill: ProgressBarGroup,
    program: ProgressBarGroup,
}

pub struct ProgressBarGroup {
    message: String,
    bars: Vec<ProgressBar>,
    selected: usize,
    append_phase: bool,
}

impl ProgressBarGroup {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            bars: vec![],
            selected: 0,
            append_phase: false,
        }
    }

    fn idle() -> ProgressStyle {
        ProgressStyle::with_template("{msg:.green.bold} {spinner} {percent:>3}% [{bar:20}]")
            .unwrap()
            .tick_chars("⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈✔")
            .progress_chars("--")
    }

    fn active() -> ProgressStyle {
        ProgressStyle::with_template("{msg:.green.bold} {spinner} {percent:>3}% [{bar:20}] {bytes:>10} @ {bytes_per_sec:>12} (ETA {eta})")
            .unwrap()
            .tick_chars("⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈✔")
            .progress_chars("##-")
    }

    fn finished() -> ProgressStyle {
        ProgressStyle::with_template("{msg:.green.bold} {spinner} {percent:>3}% [{bar:20}] {bytes:>10} @ {bytes_per_sec:>12} (took {elapsed})")
            .unwrap()
            .tick_chars("⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈✔")
            .progress_chars("##")
    }

    pub fn add(&mut self, bar: ProgressBar) {
        if self.append_phase {
            bar.set_message(format!("{} {}", self.message, self.bars.len() + 1));
        } else {
            bar.set_message(self.message.clone());
        }
        bar.set_style(Self::idle());
        bar.enable_steady_tick(Duration::from_millis(100));
        bar.reset_elapsed();

        self.bars.push(bar);
    }

    pub fn set_length(&mut self, length: u64) {
        if let Some(bar) = self.bars.get(self.selected) {
            bar.set_length(length);
        }
    }

    pub fn inc(&mut self, size: u64) {
        if let Some(bar) = self.bars.get(self.selected) {
            bar.set_style(Self::active());
            bar.inc(size);
        }
    }

    pub fn abandon(&mut self) {
        if let Some(bar) = self.bars.get(self.selected) {
            bar.abandon();
        }
        self.next();
    }

    pub fn finish(&mut self) {
        if let Some(bar) = self.bars.get(self.selected) {
            bar.set_style(Self::finished());
            bar.finish();
        }
        self.next();
    }

    pub fn next(&mut self) {
        self.selected += 1;
    }

    pub fn append_phase(&mut self) {
        self.append_phase = true;
    }

    pub fn mark_start_now(&mut self) {
        if let Some(bar) = self.bars.get(self.selected) {
            bar.reset_elapsed();
            bar.reset_eta();
        }
    }
}
