use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressKind {
    Download,
    Checksum,
    Extract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedUnit {
    KB,
    MB,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressUpdate {
    pub increment: f64,
    pub progress: f64,
    pub network: Option<f64>,
    pub unit: Option<SpeedUnit>,
    pub reset: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressEvent {
    pub kind: ProgressKind,
    pub update: ProgressUpdate,
}

const REPORT_THRESHOLD_BYTES: u64 = 64 * 1024;

pub(crate) struct ProgressReporter {
    kind: ProgressKind,
    total_bytes: Option<u64>,
    last_reported_bytes: u64,
    last_reported_progress: f64,
    last_report_time: Instant,
    include_network: bool,
}

impl ProgressReporter {
    pub(crate) fn new(kind: ProgressKind, total_bytes: Option<u64>, include_network: bool) -> Self {
        Self {
            kind,
            total_bytes,
            last_reported_bytes: 0,
            last_reported_progress: 0.0,
            last_report_time: Instant::now(),
            include_network,
        }
    }

    pub(crate) fn emit_reset<F>(&mut self, current_bytes: u64, on_progress: &mut F)
    where
        F: FnMut(ProgressEvent),
    {
        let progress = self
            .total_bytes
            .filter(|total| *total > 0)
            .map(|total| round_percent((current_bytes as f64 / total as f64) * 100.0))
            .unwrap_or(0.0);

        self.last_reported_bytes = current_bytes;
        self.last_reported_progress = progress;
        self.last_report_time = Instant::now();

        on_progress(ProgressEvent {
            kind: self.kind,
            update: ProgressUpdate {
                increment: 0.0,
                progress,
                network: Some(0.0),
                unit: Some(SpeedUnit::KB),
                reset: true,
            },
        });
    }

    pub(crate) fn maybe_emit<F>(&mut self, current_bytes: u64, on_progress: &mut F)
    where
        F: FnMut(ProgressEvent),
    {
        if current_bytes.saturating_sub(self.last_reported_bytes) < REPORT_THRESHOLD_BYTES {
            return;
        }

        let event = self.build_event(current_bytes);
        if event.update.increment > 0.0 {
            on_progress(event);
        }
    }

    pub(crate) fn finish<F>(&mut self, current_bytes: u64, on_progress: &mut F)
    where
        F: FnMut(ProgressEvent),
    {
        if current_bytes == self.last_reported_bytes {
            return;
        }

        let event = self.build_event(current_bytes);
        if event.update.increment > 0.0 {
            on_progress(event);
        }
    }

    fn build_event(&mut self, current_bytes: u64) -> ProgressEvent {
        let now = Instant::now();
        let bytes_since_last = current_bytes.saturating_sub(self.last_reported_bytes);
        let progress = self
            .total_bytes
            .filter(|total| *total > 0)
            .map(|total| round_percent((current_bytes as f64 / total as f64) * 100.0))
            .unwrap_or(0.0);
        let increment = round_percent(progress - self.last_reported_progress).clamp(0.0, 100.0);
        let elapsed_seconds = now.duration_since(self.last_report_time).as_secs_f64();

        self.last_reported_bytes = current_bytes;
        self.last_reported_progress = progress;
        self.last_report_time = now;

        let (network, unit) =
            if self.include_network && elapsed_seconds > 0.0 && bytes_since_last > 0 {
                let kb_per_second = (bytes_since_last as f64 / 1024.0) / elapsed_seconds;
                if kb_per_second >= 1024.0 {
                    (
                        Some(round_percent(kb_per_second / 1024.0)),
                        Some(SpeedUnit::MB),
                    )
                } else {
                    (Some(round_percent(kb_per_second)), Some(SpeedUnit::KB))
                }
            } else {
                (None, None)
            };

        ProgressEvent {
            kind: self.kind,
            update: ProgressUpdate {
                increment,
                progress,
                network,
                unit,
                reset: false,
            },
        }
    }
}

fn round_percent(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
