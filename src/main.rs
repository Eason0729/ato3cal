use std::error::Error;
use std::io;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use evalexpr::eval;

// --- Model Definitions ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PolyModel {
    pub weights: Vec<f64>,
}

impl PolyModel {
    pub fn predict(&self, seats: f64, ratio: f64, is_direct: bool) -> f64 {
        let direct_val = if is_direct { 1.0 } else { 0.0 };
        if self.weights.len() < 5 { return 0.0; }
        
        let w = &self.weights;
        let p = w[0] * 1.0 
              + w[1] * seats 
              + w[2] * ratio 
              + w[3] * ratio * ratio 
              + w[4] * direct_val;
        p
    }
}

// --- App Logic ---

enum InputMode {
    Normal,
    Editing,
}

struct App {
    // Inputs
    my_city_input: String,
    
    // State
    input_mode: InputMode,
    model: PolyModel,
    
    // Calculated
    p1_value: Option<f64>, 
    
    // Chart Data
    chart_x_cursor: f64, 
    data_stopover: Vec<(f64, f64)> ,
    data_direct: Vec<(f64, f64)> ,
    y_min: f64,
    y_max: f64,
}

impl App {
    fn new(model: PolyModel) -> App {
        let mut app = App {
            my_city_input: String::new(),
            input_mode: InputMode::Normal,
            model,
            p1_value: None,
            chart_x_cursor: 300.0, 
            data_stopover: vec![],
            data_direct: vec![],
            y_min: 0.0,
            y_max: 2000.0,
        };
        app.update_calculation();
        app
    }

    fn update_calculation(&mut self) {
        match eval(&self.my_city_input) {
            Ok(val) => match val.as_float() {
                Ok(f) => self.p1_value = Some(f),
                Err(_) => self.p1_value = val.as_int().ok().map(|i| i as f64),
            },
            Err(_) => {
                if self.my_city_input.trim().is_empty() {
                    self.p1_value = None;
                }
            }
        }

        let p1 = match self.p1_value {
            Some(v) => v,
            None => {
                self.data_stopover.clear();
                self.data_direct.clear();
                return;
            }
        };

        self.data_stopover.clear();
        self.data_direct.clear();

        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        // Generate points for Seats 0 to 720
        // We generate for the whole range so scrolling is smooth
        for s in (0..=720).step_by(10) {
            let seats = s as f64;
            
            if let Some(p2_stop) = self.solve_p2(p1, seats, false) {
                self.data_stopover.push((seats, p2_stop));
                if p2_stop < min_y { min_y = p2_stop; }
                if p2_stop > max_y { max_y = p2_stop; }
            }
            
            if let Some(p2_dir) = self.solve_p2(p1, seats, true) {
                self.data_direct.push((seats, p2_dir));
                if p2_dir < min_y { min_y = p2_dir; }
                if p2_dir > max_y { max_y = p2_dir; }
            }
        }
        
        if min_y == f64::MAX { min_y = 0.0; max_y = 1000.0; }
        self.y_min = (min_y - 100.0).max(0.0);
        self.y_max = max_y + 100.0;
    }

    fn solve_p2(&self, p1: f64, seats: f64, is_direct: bool) -> Option<f64> {
        let mut low = 0.0;
        let mut high = 50_000.0;
        let mut ans = -1.0;

        for _ in 0..60 { 
            let mid = (low + high) / 2.0;
            let p2 = mid;
            
            let min_p = p1.min(p2).max(1.0); 
            let max_p = p1.max(p2);
            let ratio = max_p / min_p;
            
            let req_sum = self.model.predict(seats, ratio, is_direct);
            
            if p1 + p2 >= req_sum {
                ans = p2;
                high = mid; 
            } else {
                low = mid; 
            }
        }
        
        if ans < 0.0 { None } else { Some(ans) }
    }
    
    fn get_values_at_cursor(&self) -> (Option<f64>, Option<f64>) {
        if let Some(p1) = self.p1_value {
            let v1 = self.solve_p2(p1, self.chart_x_cursor, false);
            let v2 = self.solve_p2(p1, self.chart_x_cursor, true);
            (v1, v2)
        } else {
            (None, None)
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let model_data = include_bytes!("../model.bin");
    let model: PolyModel = bincode::deserialize(model_data)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(model);
    let res = run_app(&mut terminal, app);

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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press { continue; }
            
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Enter => app.input_mode = InputMode::Editing,
                    KeyCode::Left => {
                        app.chart_x_cursor = (app.chart_x_cursor - 10.0).max(0.0);
                    },
                    KeyCode::Right => {
                        app.chart_x_cursor = (app.chart_x_cursor + 10.0).min(720.0);
                    },
                    _ => {} 
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter => {
                        app.input_mode = InputMode::Normal;
                        app.update_calculation();
                    },
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                    },
                    KeyCode::Char(c) => {
                        app.my_city_input.push(c);
                    },
                    KeyCode::Backspace => {
                        app.my_city_input.pop();
                    },
                    _ => {} 
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3), // Input
                Constraint::Min(10),   // Chart
                Constraint::Length(5), // Info
            ]
            .as_ref(),
        )
        .split(f.size());

    // --- Input Area ---
    let input_style = match app.input_mode {
        InputMode::Editing => Style::default().fg(Color::Yellow),
        InputMode::Normal => Style::default().fg(Color::Green),
    };
    
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title("My City Points (Math Allowed: e.g. 100+200)");
        
    let input_text = Paragraph::new(app.my_city_input.as_str())
        .style(input_style)
        .block(input_block);
        
    f.render_widget(input_text, chunks[0]);

    // --- Chart Area ---
    if app.p1_value.is_some() {
        // Calculate Visible Window
        // Show 200 seats width
        let window_width = 200.0;
        let mut x_min = app.chart_x_cursor - window_width / 2.0;
        let mut x_max = app.chart_x_cursor + window_width / 2.0;

        // Clamp
        if x_min < 0.0 {
            x_min = 0.0;
            x_max = window_width;
        }
        if x_max > 720.0 {
            x_max = 720.0;
            x_min = 720.0 - window_width;
        }

        // Generate Labels for Window
        let x_labels = vec![
            Span::raw(format!("{:.0}", x_min)),
            Span::raw(format!("{:.0}", (x_min+x_max)/2.0)),
            Span::raw(format!("{:.0}", x_max)),
        ];
        
        let y_min = app.y_min;
        let y_max = app.y_max;
        let y_labels = vec![
            Span::raw(format!("{:.0}", y_min)),
            Span::raw(format!("{:.0}", (y_min+y_max)/2.0)),
            Span::raw(format!("{:.0}", y_max)),
        ];

        // Cursor Line Dataset
        let cursor_data = vec![
            (app.chart_x_cursor, y_min),
            (app.chart_x_cursor, y_max),
        ];

        let datasets = vec![
            Dataset::default()
                .name("Stopover")
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::Red))
                .graph_type(GraphType::Line)
                .data(&app.data_stopover),
            Dataset::default()
                .name("Direct")
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::Cyan))
                .graph_type(GraphType::Line)
                .data(&app.data_direct),
            Dataset::default()
                .name("Selected")
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::Yellow))
                .graph_type(GraphType::Line)
                .data(&cursor_data),
        ];

        let chart = Chart::new(datasets)
            .block(Block::default().title("Other City Needed (Y) vs Plane Seats (X) - [Use Left/Right to Scroll]").borders(Borders::ALL))
            .x_axis(
                Axis::default()
                    .title("Seats")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([x_min, x_max])
                    .labels(x_labels),
            )
            .y_axis(
                Axis::default()
                    .title("Other City Points")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([y_min, y_max])
                    .labels(y_labels),
            );
            
        f.render_widget(chart, chunks[1]);
        
    } else {
        let warning = Paragraph::new("Please enter a valid number or expression (e.g. '100+50') and press Enter.")
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(warning, chunks[1]);
    }

    // --- Info / Cursor Area ---
    let (stop_val, dir_val) = app.get_values_at_cursor();
    
    let info_text = format!(
        "Selected Plane Size: {:.0} Seats\nStopover Needs: {:.2} | Direct Needs: {:.2}",
        app.chart_x_cursor,
        stop_val.unwrap_or(0.0),
        dir_val.unwrap_or(0.0)
    );
    
    let info_block = Paragraph::new(info_text)
        .block(Block::default().borders(Borders::ALL).title("Precise Prediction"))
        .style(Style::default().fg(Color::White).bg(Color::Black));
        
    f.render_widget(info_block, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evalexpr_usage() {
        let res = eval("100+200").unwrap().as_int().unwrap();
        assert_eq!(res, 300);
    }
}