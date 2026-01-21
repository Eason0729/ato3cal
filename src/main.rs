use std::error::Error;
use std::io;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};

// --- Model Definitions ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PolyModel {
    pub weights: Vec<f64>,
}

impl PolyModel {
    // Features: [1.0, Seats, Ratio, Ratio^2, IsDirect]
    pub fn predict(&self, seats: f64, ratio: f64, is_direct: bool) -> f64 {
        let direct_val = if is_direct { 1.0 } else { 0.0 };
        // If weights don't match expected length, fallback or panic (but they should match)
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

// --- App State ---

enum InputMode {
    Normal,
    Editing,
}

#[derive(PartialEq, Debug)]
enum FocusedField {
    MyCityPoint,
    PlaneSeating,
    FlightType,
}

struct App {
    my_city_point: String,
    plane_seating: String,
    is_direct: bool,
    input_mode: InputMode,
    focused_field: FocusedField,
    model: PolyModel,
}

impl App {
    fn new(model: PolyModel) -> App {
        App {
            my_city_point: String::new(),
            plane_seating: String::new(),
            is_direct: false, // Default to Stopover
            input_mode: InputMode::Normal,
            focused_field: FocusedField::MyCityPoint,
            model,
        }
    }

    // Binary Search Solver to find P2 such that P1 + P2 >= Model(S, Ratio(P1, P2), F)
    // We want the minimal P2.
    // Range of P2: 0 to 20,000 (Reasonable game limits)
    fn solve(&self) -> Option<(f64, f64)> {
        let p1: f64 = self.my_city_point.parse().ok()?;
        let seats: f64 = self.plane_seating.parse().ok()?;
        let is_direct = self.is_direct;
        
        let mut low = 0.0;
        let mut high = 20_000.0;
        let mut ans = -1.0;
        let mut final_req_sum = 0.0;

        // Binary search for precision 0.1
        for _ in 0..100 { 
            let mid = (low + high) / 2.0;
            let p2 = mid;
            
            // Calculate Ratio
            // Avoid division by zero
            let min_p = p1.min(p2).max(1.0); 
            let max_p = p1.max(p2);
            let ratio = max_p / min_p;
            
            let req_sum = self.model.predict(seats, ratio, is_direct);
            
            if p1 + p2 >= req_sum {
                ans = p2;
                final_req_sum = req_sum;
                high = mid; // Try smaller P2
            } else {
                low = mid; // Need bigger P2
            }
        } 
        
        if ans < 0.0 { 
             return None; // Could not find solution in range
        }

        Some((final_req_sum, ans))
    }

    fn next_field(&mut self) {
        self.focused_field = match self.focused_field {
            FocusedField::MyCityPoint => FocusedField::PlaneSeating,
            FocusedField::PlaneSeating => FocusedField::FlightType,
            FocusedField::FlightType => FocusedField::MyCityPoint,
        };
    }

    fn prev_field(&mut self) {
        self.focused_field = match self.focused_field {
            FocusedField::MyCityPoint => FocusedField::FlightType,
            FocusedField::PlaneSeating => FocusedField::MyCityPoint,
            FocusedField::FlightType => FocusedField::PlaneSeating,
        };
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Load Model
    let model_data = include_bytes!("../model.bin");
    let model: PolyModel = bincode::deserialize(model_data)?;

    // Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run App
    let app = App::new(model);
    let res = run_app(&mut terminal, app);

    // Restore Terminal
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
                    KeyCode::Tab | KeyCode::Down => app.next_field(),
                    KeyCode::BackTab | KeyCode::Up => app.prev_field(),
                    KeyCode::Enter => {
                        if app.focused_field == FocusedField::FlightType {
                            app.is_direct = !app.is_direct;
                        } else {
                            app.input_mode = InputMode::Editing;
                        }
                    },
                    KeyCode::Left | KeyCode::Right => {
                         if app.focused_field == FocusedField::FlightType {
                            app.is_direct = !app.is_direct;
                         }
                    },
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter | KeyCode::Esc => app.input_mode = InputMode::Normal,
                    KeyCode::Char(c) => {
                        match app.focused_field {
                            FocusedField::MyCityPoint => {
                                if c.is_ascii_digit() || c == '.' {
                                    app.my_city_point.push(c);
                                }
                            },
                            FocusedField::PlaneSeating => {
                                if c.is_ascii_digit() || c == '.' {
                                    app.plane_seating.push(c);
                                }
                            },
                            _ => {} 
                        }
                    },
                    KeyCode::Backspace => {
                         match app.focused_field {
                            FocusedField::MyCityPoint => { app.my_city_point.pop(); },
                            FocusedField::PlaneSeating => { app.plane_seating.pop(); },
                            _ => {}
                        }
                    }
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
                Constraint::Length(1), // Title
                Constraint::Length(3), // My City Point
                Constraint::Length(3), // Plane Seating
                Constraint::Length(3), // Flight Type
                Constraint::Min(5),    // Result
                Constraint::Length(1), // Footer
            ]
            .as_ref(),
        )
        .split(f.area()); // Updated to use .area()

    let title = Paragraph::new("ATO3 Calculator - ML Ratio Solver")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(title, chunks[0]);

    // Helper to style active fields
    let get_style = |field: FocusedField| {
        if app.focused_field == field {
            match app.input_mode {
                InputMode::Editing => Style::default().fg(Color::Yellow),
                InputMode::Normal => Style::default().fg(Color::Green),
            }
        } else {
            Style::default()
        }
    };

    // My City Point Input
    let my_city_txt = Paragraph::new(app.my_city_point.as_str())
        .style(get_style(FocusedField::MyCityPoint))
        .block(Block::default().borders(Borders::ALL).title("My City Points"));
    f.render_widget(my_city_txt, chunks[1]);

    // Plane Seating Input
    let plane_txt = Paragraph::new(app.plane_seating.as_str())
        .style(get_style(FocusedField::PlaneSeating))
        .block(Block::default().borders(Borders::ALL).title("Plane Max Seating"));
    f.render_widget(plane_txt, chunks[2]);

    // Flight Type Selector
    let type_str = if app.is_direct { "Direct Flight" } else { "Stopover Flight" };
    let type_widget = Paragraph::new(format!(" < {} > ", type_str))
        .style(get_style(FocusedField::FlightType))
        .block(Block::default().borders(Borders::ALL).title("Flight Type (Enter/Arrow to Toggle)"));
    f.render_widget(type_widget, chunks[3]);

    // Result Area
    let result_text = if let Some((req_sum, needed)) = app.solve() {
        format!(
            "Required Total Sum (Model Est.): {:.2}\n\n>>> Other City Needed: {:.2} <<<",
            req_sum,
            needed
        )
    } else {
        String::from("Enter valid numbers to calculate.")
    };
    
    let result_widget = Paragraph::new(result_text)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL).title("Prediction"));
    f.render_widget(result_widget, chunks[4]);

    let footer = Paragraph::new("Press 'q' to quit. 'Enter' to edit. Up/Down/Tab to navigate.")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, chunks[5]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_solver_logic() {
        // Mock Model: Sum = 100 + 0*Seats + 0*Ratio + 0*Ratio^2 + 0*Direct
        // So Required Sum always 100.
        let mock_weights = vec![100.0, 0.0, 0.0, 0.0, 0.0];
        let model = PolyModel { weights: mock_weights };
        
        let mut app = App::new(model);
        app.my_city_point = String::from("60");
        app.plane_seating = String::from("500");
        
        // If Sum is 100, and I have 60, I need 40.
        // Ratio logic shouldn't break this simple case.
        
        let res = app.solve();
        assert!(res.is_some());
        let (sum, needed) = res.unwrap();
        
        assert!((sum - 100.0).abs() < 1.0);
        assert!((needed - 40.0).abs() < 1.0);
    }
    
     #[test]
    fn test_app_solver_complex() {
        // Mock Model: Sum = 100 + 10 * Ratio
        // Weights: [100.0, 0.0, 10.0, 0.0, 0.0]
        let mock_weights = vec![100.0, 0.0, 10.0, 0.0, 0.0];
        let model = PolyModel { weights: mock_weights };
        
        let mut app = App::new(model);
        app.my_city_point = String::from("100"); 
        app.plane_seating = String::from("0");
        
        // P1 = 100. 
        // Try P2 = 100 (Ratio 1.0) -> Sum = 110. P1+P2=200 > 110. OK.
        // Try P2 = 10 (Ratio 10.0) -> Sum = 200. P1+P2=110 < 200. Fail.
        
        // We want Minimal P2.
        // P1 + P2 = 100 + 10 * Ratio
        // P1 + P2 = 100 + 10 * (P1/P2) [assuming P2 < P1]
        // 100 + P2 = 100 + 1000/P2
        // P2 = 1000/P2 -> P2^2 = 1000 -> P2 approx 31.6
        
        let res = app.solve();
        assert!(res.is_some());
        let (_, needed) = res.unwrap();
        
        // Solver is approximate, check if close
        assert!((needed - 31.6).abs() < 1.0);
    }
}
