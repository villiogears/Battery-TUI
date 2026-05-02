use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use std::io;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[derive(Clone, Default)]
struct BatteryInfo {
    name: String,
    status: String,
    capacity: u8,
    energy_now: f64,
    energy_full: f64,
    energy_full_design: f64,
    power_now: f64,
    voltage_now: f64,
    current_now: f64,
    manufacturer: String,
    model_name: String,
    cycle_count: u64,
    health: f64,
}

fn read_file_string(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn read_file_u64(path: &Path) -> Option<u64> {
    read_file_string(path).and_then(|s| s.parse().ok())
}

fn get_batteries() -> Vec<BatteryInfo> {
    let mut batteries = Vec::new();
    let power_supply_dir = Path::new("/sys/class/power_supply");
    
    if let Ok(entries) = fs::read_dir(power_supply_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().into_string().unwrap_or_default();
            if name.starts_with("BAT") {
                let mut info = BatteryInfo {
                    name: name.clone(),
                    ..Default::default()
                };
                
                let dir = entry.path();
                
                info.status = read_file_string(&dir.join("status")).unwrap_or_else(|| "Unknown".to_string());
                info.capacity = read_file_u64(&dir.join("capacity")).unwrap_or(0) as u8;
                info.manufacturer = read_file_string(&dir.join("manufacturer")).unwrap_or_else(|| "Unknown".to_string());
                info.model_name = read_file_string(&dir.join("model_name")).unwrap_or_else(|| "Unknown".to_string());
                info.cycle_count = read_file_u64(&dir.join("cycle_count")).unwrap_or(0);
                
                let energy_now_uwh = read_file_u64(&dir.join("energy_now"));
                let energy_full_uwh = read_file_u64(&dir.join("energy_full"));
                let energy_full_design_uwh = read_file_u64(&dir.join("energy_full_design"));
                let power_now_uw = read_file_u64(&dir.join("power_now"));
                
                let charge_now_uah = read_file_u64(&dir.join("charge_now"));
                let charge_full_uah = read_file_u64(&dir.join("charge_full"));
                let charge_full_design_uah = read_file_u64(&dir.join("charge_full_design"));
                let current_now_ua = read_file_u64(&dir.join("current_now"));
                
                let voltage_now_uv = read_file_u64(&dir.join("voltage_now")).unwrap_or(12_000_000);
                info.voltage_now = voltage_now_uv as f64 / 1_000_000.0;
                
                if let Some(e) = energy_now_uwh {
                    info.energy_now = e as f64 / 1_000_000.0;
                } else if let Some(c) = charge_now_uah {
                    info.energy_now = (c as f64 * voltage_now_uv as f64) / 1_000_000_000_000.0;
                }
                
                if let Some(e) = energy_full_uwh {
                    info.energy_full = e as f64 / 1_000_000.0;
                } else if let Some(c) = charge_full_uah {
                    info.energy_full = (c as f64 * voltage_now_uv as f64) / 1_000_000_000_000.0;
                }
                
                if let Some(e) = energy_full_design_uwh {
                    info.energy_full_design = e as f64 / 1_000_000.0;
                } else if let Some(c) = charge_full_design_uah {
                    info.energy_full_design = (c as f64 * voltage_now_uv as f64) / 1_000_000_000_000.0;
                }
                
                if let Some(p) = power_now_uw {
                    info.power_now = p as f64 / 1_000_000.0;
                    info.current_now = if info.voltage_now > 0.0 { info.power_now / info.voltage_now } else { 0.0 };
                } else if let Some(c) = current_now_ua {
                    info.current_now = c as f64 / 1_000_000.0;
                    info.power_now = info.current_now * info.voltage_now;
                }
                
                if info.energy_full_design > 0.0 {
                    info.health = (info.energy_full / info.energy_full_design) * 100.0;
                }
                
                batteries.push(info);
            }
        }
    }
    
    batteries
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let tick_rate = Duration::from_millis(1000);
    let mut last_tick = Instant::now();

    loop {
        let batteries = get_batteries();
        
        terminal.draw(|f| ui(f, &batteries))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn ui(f: &mut ratatui::Frame, batteries: &[BatteryInfo]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(f.area());

    let title = Paragraph::new(" Battery Information TUI (Press 'q' to quit) ")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    if batteries.is_empty() {
        let msg = Paragraph::new("No batteries found on this system.")
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(msg, chunks[1]);
        return;
    }

    let bat_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            batteries.iter().map(|_| Constraint::Length(15)).collect::<Vec<_>>()
        )
        .split(chunks[1]);

    for (i, bat) in batteries.iter().enumerate() {
        if i >= bat_chunks.len() { break; }
        
        let block = Block::default()
            .title(format!(" {} ", bat.name))
            .borders(Borders::ALL);
            
        let inner_area = block.inner(bat_chunks[i]);
        f.render_widget(block, bat_chunks[i]);
        
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(2), // Capacity Gauge
                    Constraint::Length(2), // Health Gauge
                    Constraint::Min(0),    // Text details
                ]
            )
            .split(inner_area);
            
        let color = match bat.capacity {
            0..=20 => Color::Red,
            21..=50 => Color::Yellow,
            _ => Color::Green,
        };
        
        let capacity_gauge = Gauge::default()
            .block(Block::default().title("Charge Level"))
            .gauge_style(Style::default().fg(color))
            .percent(bat.capacity as u16);
        f.render_widget(capacity_gauge, content_chunks[0]);
        
        let health_color = match bat.health as u8 {
            0..=50 => Color::Red,
            51..=80 => Color::Yellow,
            _ => Color::Green,
        };
        
        let health_gauge = Gauge::default()
            .block(Block::default().title("Battery Health (Lifespan)"))
            .gauge_style(Style::default().fg(health_color))
            .percent(bat.health.clamp(0.0, 100.0) as u16);
        f.render_widget(health_gauge, content_chunks[1]);
        
        let text = vec![
            Line::from(vec![
                Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&bat.status),
            ]),
            Line::from(vec![
                Span::styled("Manufacturer: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&bat.manufacturer),
                Span::styled(" | Model: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&bat.model_name),
            ]),
            Line::from(vec![
                Span::styled(
                    if bat.status == "Charging" {
                        "Power Supply: "
                    } else if bat.status == "Discharging" {
                        "Power Draw: "
                    } else {
                        "Power Rate: "
                    },
                    Style::default().add_modifier(Modifier::BOLD)
                ),
                Span::raw(format!("{:.2} W ({:.2} V, {:.2} A)", bat.power_now, bat.voltage_now, bat.current_now)),
            ]),
            Line::from(vec![
                Span::styled("Energy Now: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{:.2} Wh", bat.energy_now)),
                Span::styled(" / Full: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{:.2} Wh", bat.energy_full)),
                Span::styled(" (Design: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{:.2} Wh)", bat.energy_full_design)),
            ]),
            Line::from(vec![
                Span::styled("Cycle Count: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{}", bat.cycle_count)),
            ]),
        ];
        
        let details = Paragraph::new(text).wrap(Wrap { trim: true });
        f.render_widget(details, content_chunks[2]);
    }
}
