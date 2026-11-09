use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use kitcar::ui::{self, InvalidateHandle};
use tokio::{task::JoinHandle, time::Instant as TokioInstant};

use super::hud::hud_text;

pub struct Marquee {
    wait_ticks: usize,
    tick_count: Arc<AtomicU64>,
    handle: JoinHandle<()>,
}

impl Drop for Marquee {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl Marquee {
    pub fn new(invalidator: InvalidateHandle) -> Self {
        let period = Duration::from_millis(150);
        let wait_duration = Duration::from_secs(3);
        let wait_ticks = usize::try_from(wait_duration.as_millis().div_ceil(period.as_millis()))
            .expect("marquee wait duration fits in usize");

        let tick_count = Arc::new(AtomicU64::new(0));
        let tick_counter = tick_count.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval_at(TokioInstant::now() + period, period);
            loop {
                let _ = interval.tick().await;
                let _ = tick_counter.fetch_add(1, Ordering::Relaxed);
                invalidator.invalidate();
            }
        });

        Self {
            wait_ticks,
            tick_count,
            handle,
        }
    }
}

pub struct MarqueeProps<'a> {
    pub text: &'a str,
    pub width: usize,
}

impl ui::Component for Marquee {
    type Message = std::convert::Infallible;
    type Props<'a> = MarqueeProps<'a>;

    fn render(&self, props: Self::Props<'_>) -> ui::Node<Self::Message> {
        let mut canvas = Vec::new();
        canvas.extend(std::iter::repeat_n(' ', props.width));
        canvas.extend(props.text.chars());
        canvas.extend(std::iter::repeat_n(' ', props.width));
        let scroll_limit = canvas.len().saturating_sub(props.width);

        let w = props.width as f32;

        if scroll_limit == 0 {
            let end = props.width.min(canvas.len());
            let visible: String = canvas[..end].iter().collect();
            return ui::text(visible, hud_text().align_left())
                .w(w)
                .h(5.)
                .key("marquee");
        }

        let cycle_ticks = scroll_limit + self.wait_ticks;
        let phase = usize::try_from(self.tick_count.load(Ordering::Relaxed)).unwrap_or(usize::MAX)
            % cycle_ticks;

        if phase < scroll_limit {
            let offset = phase;
            let end = (offset + props.width).min(canvas.len());
            let visible: String = canvas[offset..end].iter().collect();
            return ui::text(visible, hud_text()).w(w).h(5.).key("marquee");
        }

        ui::text("", hud_text()).w(w).h(5.)
    }
}
