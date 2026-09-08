//made by ai for terminal rendering
use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{Event, EventStream, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame, Terminal,
};
use tokio::sync::watch;

#[derive(Debug, Clone, Default)]
pub struct OrderBookView {
    pub symbol: String,
    pub is_synced: bool,
    pub last_update_id: u64,
    pub bids: Vec<(f64, f64)>, // (Price, Size)
    pub asks: Vec<(f64, f64)>, // (Price, Size)
}

pub struct TerminalUi {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalUi {
    pub fn init() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub async fn run(
        &mut self,
        mut rx: watch::Receiver<OrderBookView>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut render_interval = tokio::time::interval(Duration::from_millis(50));
        let mut event_stream = EventStream::new();

        loop {
            tokio::select! {
                _ = render_interval.tick() => {
                    let view = rx.borrow_and_update().clone();

                    crate::log_debug(&format!(
                        "TUI Render Frame -> Synced: {}, Seq ID: {}, Bids: {}, Asks: {}",
                        view.is_synced,
                        view.last_update_id,
                        view.bids.len(),
                        view.asks.len()
                    ));

                    self.terminal.draw(|f| render_frame(f, &view))?;
                }
                maybe_event = event_stream.next() => {
                    if let Some(Ok(Event::Key(key))) = maybe_event {
                        if key.code == KeyCode::Char('q')
                            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
                        {
                            break;
                        }
                    }
                }
            }
        }

        self.cleanup()?;
        Ok(())
    }

    fn cleanup(&mut self) -> io::Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for TerminalUi {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn render_frame(f: &mut Frame, view: &OrderBookView) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(15)])
        .split(f.area());

    // --- Header Block ---
    let (status_str, status_style) = if view.is_synced {
        ("[SYNCED]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        (
            "[RECONCILING]",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
    };

    let best_bid = view.bids.first().map(|(p, _)| *p).unwrap_or(0.0);
    let best_ask = view.asks.first().map(|(p, _)| *p).unwrap_or(0.0);
    let spread = if best_ask > 0.0 && best_bid > 0.0 {
        best_ask - best_bid
    } else {
        0.0
    };

    let header_text = vec![Line::from(vec![
        Span::raw("Symbol: "),
        Span::styled(&view.symbol, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" | Status: "),
        Span::styled(status_str, status_style),
        Span::raw(" | Seq ID: "),
        Span::styled(view.last_update_id.to_string(), Style::default().fg(Color::White)),
        Span::raw(" | Spread: "),
        Span::styled(format!("{:.2}", spread), Style::default().fg(Color::Yellow)),
        Span::raw(" | Press 'q' to exit"),
    ])];

    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title(" Ordust Terminal "));
    f.render_widget(header, chunks[0]);

    // --- Ladder View Calculations ---
    let mut ask_cum = 0.0;
    let mut ask_data: Vec<(f64, f64, f64)> = view
        .asks
        .iter()
        .map(|&(px, sz)| {
            ask_cum += sz;
            (px, sz, ask_cum)
        })
        .collect();

    let mut bid_cum = 0.0;
    let bid_data: Vec<(f64, f64, f64)> = view
        .bids
        .iter()
        .map(|&(px, sz)| {
            bid_cum += sz;
            (px, sz, bid_cum)
        })
        .collect();

    let max_depth = ask_cum.max(bid_cum).max(1e-6);

    // Highest ask on top, lowest ask right above spread
    ask_data.reverse();

    let mut table_rows: Vec<Row> = Vec::new();
    let bar_max_width = 15; // Max character width for depth bar

    // Helper to build a horizontal depth bar string
    let make_depth_bar = |ratio: f64| -> String {
        let filled_len = ((ratio * bar_max_width as f64).round() as usize).min(bar_max_width);
        "█".repeat(filled_len)
    };

    // 1. Build Ask Rows (Red Horizontal Bars)
    for (price, size, total) in ask_data {
        let ratio = (total / max_depth).clamp(0.0, 1.0);
        let bar_str = make_depth_bar(ratio);

        let cell_px = Cell::from(format!("{:.2}", price))
            .style(Style::default().fg(Color::Rgb(255, 85, 110)).add_modifier(Modifier::BOLD));
        let cell_sz = Cell::from(format!("{:.4}", size))
            .style(Style::default().fg(Color::White));
        let cell_tot = Cell::from(format!("{:.4}", total))
            .style(Style::default().fg(Color::Gray));
        let cell_bar = Cell::from(bar_str)
            .style(Style::default().fg(Color::Rgb(180, 40, 60)));

        table_rows.push(Row::new(vec![cell_px, cell_sz, cell_tot, cell_bar]));
    }

    // 2. Build Spread Row
    let spread_pct = if best_bid > 0.0 {
        (spread / best_bid) * 100.0
    } else {
        0.0
    };

    let spread_row = Row::new(vec![
        Cell::from(format!("Spread {}", view.symbol)).style(Style::default().fg(Color::DarkGray)),
        Cell::from(format!("{:.2}", spread)).style(Style::default().fg(Color::Yellow)),
        Cell::from(format!("{:.3}%", spread_pct)).style(Style::default().fg(Color::Yellow)),
        Cell::from(""),
    ])
    .style(Style::default().bg(Color::Rgb(18, 22, 28)));

    table_rows.push(spread_row);

    // 3. Build Bid Rows (Green Horizontal Bars)
    for (price, size, total) in bid_data {
        let ratio = (total / max_depth).clamp(0.0, 1.0);
        let bar_str = make_depth_bar(ratio);

        let cell_px = Cell::from(format!("{:.2}", price))
            .style(Style::default().fg(Color::Rgb(40, 220, 120)).add_modifier(Modifier::BOLD));
        let cell_sz = Cell::from(format!("{:.4}", size))
            .style(Style::default().fg(Color::White));
        let cell_tot = Cell::from(format!("{:.4}", total))
            .style(Style::default().fg(Color::Gray));
        let cell_bar = Cell::from(bar_str)
            .style(Style::default().fg(Color::Rgb(20, 140, 70)));

        table_rows.push(Row::new(vec![cell_px, cell_sz, cell_tot, cell_bar]));
    }

    // Render Ladder Table
    let header_row = Row::new(vec![
        Cell::from("Price ($)").style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
        Cell::from(format!("Size ({})", view.symbol)).style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
        Cell::from(format!("Total ({})", view.symbol)).style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
        Cell::from("Depth Visualization").style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
    ]);

    let ladder_table = Table::new(
        table_rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ],
    )
    .header(header_row)
    .block(Block::default().borders(Borders::ALL).title(" ORDER BOOK LADDER "));

    f.render_widget(ladder_table, chunks[1]);
}